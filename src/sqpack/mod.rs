//! Backend-independent SqPack resource access.
//!
//! `SqPack::read` resolves game resource paths and extracts standard binary and
//! texture entries. Pure byte parsers are separate from asynchronous I/O. Optional
//! `local` and `web` modules provide backends (behind same-named features).
//! Futures deliberately have no `Send` bound, so browser-local sources can use them.
//!
//! # Resource paths versus container names
//!
//! [`SqPack::read`] accepts game paths such as `exd/item.exh`, normalizes ASCII
//! case, locates the index entry and returns the extracted bytes. It does not
//! interpret the extracted file's format. [`SqPack::open`] is lower level: it
//! opens a physical container name relative to the SqPack root, such as
//! `ffxiv/0a0000.win32.index`, for checked offset reads through [`PackReader`].
//! Neither operation requires a directory listing or registration of files.
//!
//! # Basic use with any backend
//!
//! ```no_run
//! use icexiv_data::sqpack::{Error, PackSource, SqPack};
//!
//! async fn extract_item_header<S: PackSource>(
//!     source: S,
//! ) -> Result<Vec<u8>, Error<S::Error>> {
//!     let sqpack = SqPack::new(source)
//!         .with_max_read_bytes(Some(64 * 1024 * 1024));
//!     sqpack.read("exd/item.exh").await
//! }
//! ```
//!
//! `SqPack::new` performs no I/O. Its byte cap defaults to `None` (no limit).
//! Opting into a cap bounds individual reads and declared extracted sizes, not
//! aggregate memory or concurrent requests. Overflow, range and response-length
//! checks always apply, even without a cap. Parsing/decompression run on the
//! polling thread; async I/O does not automatically offload CPU work.
//!
//! # Built-in backends
//!
//! Enable `local` for `sqpack::local::Local` on native platforms. Its root is
//! **`game/sqpack`**, not `game`; filesystem work uses Tokio's blocking pool.
//! Keep the installation stable while reading it.
//!
//! ```no_run
//! # #[cfg(all(feature = "local", not(target_arch = "wasm32")))]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use icexiv_data::sqpack::{SqPack, local::Local};
//!
//! // Run inside a Tokio runtime.
//! let source = Local::new("/path/to/game/sqpack").await?;
//! let bytes = SqPack::new(source).read("exd/item.exh").await?;
//! # let _ = bytes;
//! # Ok(())
//! # }
//! ```
//!
//! Enable `web` for `sqpack::web::Web` on native or browser WASM. Native HTTP
//! uses Tokio; browser HTTP uses Fetch. Serve unencoded containers under a
//! version-pinned root with HEAD, strong ETags and single-range GET support.
//! Cross-origin browser access also requires appropriate CORS headers; see
//! that backend's module documentation.
//!
//! ```no_run
//! # #[cfg(feature = "web")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use icexiv_data::sqpack::{SqPack, web::Web};
//!
//! let source = Web::new("https://assets.example/game/sqpack/")?;
//! let bytes = SqPack::new(source).read("exd/item.exh").await?;
//! # let _ = bytes;
//! # Ok(())
//! # }
//! ```
//!
//! Custom backends implement [`PackSource`] and [`ReadAt`]. Only confirmed
//! absence is `Ok(None)`; permission, transport and decode failures are errors.
//!
//! # Supported extraction and limitations
//!
//! Supports Win32 little-endian version-1 index/index2 files, type-2 standard
//! entries, type-3 model entries and type-4 texture entries, with raw or raw-DEFLATE blocks. Extract
//! results length-check every table, block span and declared size. Textures are
//! returned as the concatenated TEX file bytes, ready for [`crate::tex`]; the
//! container layer does not decode pixels. Models include a reconstructed logical
//! header and every LOD buffer, ready for [`crate::mdl`]. Other content types
//! return [`Error::UnsupportedContentType`]; flagged or ambiguous index entries
//! return [`Error::UnsupportedIndexEntry`].
//!
//! Some installations split large DAT files into segments that begin in the entry
//! area and carry no container header; such segments are read from offset zero.
//!
//! Lookup probes category/repository chunks in ascending order and prefers index
//! over index2. Without a listing or cache, a miss can require 512 opens. Enable
//! [`SqPack::with_index_cache`] to keep parsed tables between calls. Container
//! SHA-1 checksums are not verified. Per-file consistency checks are not a
//! cross-file release snapshot. [`Error::MissingResource`] is distinct from
//! missing containers, malformed data and backend errors.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::sync::{Arc, Mutex};

use index::Index;

mod format;
pub mod index;
mod model;
mod resource;
mod texture;
pub use texture::TextureGroup;

#[cfg(all(feature = "local", not(target_arch = "wasm32")))]
pub mod local;
#[cfg(feature = "web")]
pub mod web;

// Keep transport mapping safe without requiring callers to wrap strings.
fn is_valid_file_name(name: &str) -> bool {
    name.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    })
}

/// An opened, random-access snapshot of one container file.
///
/// Length must remain stable, and all reads must refer to the same file version.
/// Adapters must return an error if they detect replacement during reading; HTTP
/// adapters should use version validators where available. No `Send`/`Sync` or
/// runtime requirement is imposed.
pub trait ReadAt {
    type Error;

    /// Cached byte length; this method must not perform I/O.
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return exactly `length` bytes, or an error. Do not silently accept a full
    /// HTTP response to a range request or allocate unbounded response bodies.
    /// Callers must check bounds and any desired budget before calling this method.
    /// Backends must not collect more than the requested length. This low-level
    /// trait applies no configurable allocation budget; use [`PackReader`] for
    /// reads governed by [`SqPack`]'s optional cap.
    fn read_at(
        &self,
        offset: u64,
        length: usize,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>>;
}

/// Opens logical files without prescribing a filesystem or network transport.
pub trait PackSource {
    type Error;
    type File: ReadAt<Error = Self::Error>;

    /// `Ok(None)` means confirmed absence, not a permission, transport or decode
    /// failure. Adapters must preserve those failures as `Err`.
    /// Names are relative to the data root, not OS paths, URLs or paths inside
    /// a container. Components use ASCII letters, digits, `.`, `_` and `-`,
    /// separated by `/`; empty, `.` and `..` components are invalid. Backends
    /// must reject invalid names before I/O, including when called directly.
    fn open(&self, file: &str) -> impl Future<Output = Result<Option<Self::File>, Self::Error>>;
}

/// Errors from container access. Backend errors remain intact in `Source`.
#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    InvalidFileName(String),
    InvalidResourcePath(String),
    MissingFile(String),
    MissingResource(String),
    InvalidData {
        file: String,
        reason: &'static str,
    },
    UnsupportedContentType {
        file: String,
        kind: u32,
    },
    UnsupportedIndexEntry {
        file: String,
        reason: &'static str,
    },
    TextureGroupOutOfBounds {
        index: usize,
        count: usize,
    },
    AllocationFailed {
        requested: usize,
    },
    Source(E),
    ReadTooLarge {
        requested: usize,
        limit: usize,
    },
    RangeOverflow,
    OutOfBounds {
        offset: u64,
        length: usize,
        size: u64,
    },
    UnexpectedReadLength {
        expected: usize,
        actual: usize,
    },
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFileName(file) => write!(f, "invalid relative container name: {file}"),
            Self::InvalidResourcePath(path) => {
                write!(f, "invalid or unsupported resource path: {path}")
            }
            Self::MissingFile(file) => write!(f, "container file missing: {file}"),
            Self::MissingResource(path) => write!(f, "resource not found: {path}"),
            Self::InvalidData { file, reason } => {
                write!(f, "invalid SqPack data in {file}: {reason}")
            }
            Self::UnsupportedContentType { file, kind } => {
                write!(f, "unsupported SqPack content type {kind} in {file}")
            }
            Self::UnsupportedIndexEntry { file, reason } => {
                write!(f, "unsupported SqPack index entry in {file}: {reason}")
            }
            Self::TextureGroupOutOfBounds { index, count } => {
                write!(f, "texture group {index} exceeds group count {count}")
            }
            Self::AllocationFailed { requested } => {
                write!(f, "could not allocate {requested} bytes")
            }
            Self::Source(error) => write!(f, "source error: {error}"),
            Self::ReadTooLarge { requested, limit } => {
                write!(f, "read of {requested} bytes exceeds limit {limit}")
            }
            Self::RangeOverflow => f.write_str("read range overflow"),
            Self::OutOfBounds {
                offset,
                length,
                size,
            } => {
                write!(
                    f,
                    "read at {offset} of {length} bytes exceeds file size {size}"
                )
            }
            Self::UnexpectedReadLength { expected, actual } => {
                write!(f, "expected {expected} bytes, received {actual}")
            }
        }
    }
}

impl<E: StdError + 'static> StdError for Error<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            _ => None,
        }
    }
}

/// Backend-independent entry point. Construction performs no I/O.
pub struct SqPack<S> {
    source: S,
    max_read_bytes: Option<usize>,
    index_cache: Mutex<IndexCache>,
}

#[derive(Default)]
struct IndexCache {
    budget: usize,
    used: usize,
    tables: std::collections::BTreeMap<String, Arc<Index<Vec<u8>>>>,
}

impl<S: PackSource> SqPack<S> {
    /// Construct without a per-request byte cap. Performs no I/O.
    pub fn new(source: S) -> Self {
        Self {
            source,
            max_read_bytes: None,
            index_cache: Mutex::new(IndexCache::default()),
        }
    }

    /// Set an optional byte cap for individual reads and extracted resources.
    /// `None` (the default) imposes no cap; `Some(0)` allows only empty reads.
    /// Bounds, arithmetic and response-length checks always apply. This is not
    /// an aggregate-memory limit; set a cap before reading untrusted lengths.
    /// Resource extraction checks the declared output size before allocation.
    pub fn with_max_read_bytes(mut self, max_read_bytes: Option<usize>) -> Self {
        self.max_read_bytes = max_read_bytes;
        self
    }

    /// Keep parsed index tables in memory, bounded by `budget` bytes of table
    /// data. `None` (the default) re-reads and re-parses them on every call, which
    /// is correct but repeats container opens and table reads for bulk lookups.
    /// The cache belongs to this value and holds no DAT data.
    pub fn with_index_cache(mut self, budget: Option<usize>) -> Self {
        self.index_cache = Mutex::new(IndexCache {
            budget: budget.unwrap_or(0),
            ..IndexCache::default()
        });
        self
    }

    fn cached_index(&self, name: &str) -> Option<Arc<Index<Vec<u8>>>> {
        let cache = self.index_cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.tables.get(name).cloned()
    }

    fn store_index(&self, name: String, index: Arc<Index<Vec<u8>>>) {
        let mut cache = self.index_cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.budget == 0 {
            return;
        }
        let size = index.byte_len();
        if size > cache.budget {
            return;
        }
        if cache.used + size > cache.budget {
            cache.tables.clear();
            cache.used = 0;
        }
        if cache.tables.insert(name, index).is_none() {
            cache.used += size;
        }
    }

    /// Open a container, not a resource inside it; use [`Self::read`] for that.
    /// Valid names are resolved by the source; confirmed absence
    /// becomes `MissingFile`, while backend failures remain `Source` errors.
    pub async fn open(&self, file: &str) -> Result<PackReader<S::File>, Error<S::Error>> {
        self.try_open(file)
            .await?
            .ok_or_else(|| Error::MissingFile(file.to_owned()))
    }

    async fn try_open(&self, file: &str) -> Result<Option<PackReader<S::File>>, Error<S::Error>> {
        if !is_valid_file_name(file) {
            return Err(Error::InvalidFileName(file.to_owned()));
        }
        let Some(inner) = self.source.open(file).await.map_err(Error::Source)? else {
            return Ok(None);
        };
        let size = inner.len();
        Ok(Some(PackReader {
            inner,
            size,
            max_read_bytes: self.max_read_bytes,
        }))
    }
}

/// Checked access to an opened file. The source handle is kept private so reads
/// through this API cannot bypass range checks or a configured per-request cap.
pub struct PackReader<F> {
    inner: F,
    size: u64,
    max_read_bytes: Option<usize>,
}

impl<F: ReadAt> PackReader<F> {
    pub fn len(&self) -> u64 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Error<F::Error>> {
        if let Some(limit) = self.max_read_bytes.filter(|&limit| length > limit) {
            return Err(Error::ReadTooLarge {
                requested: length,
                limit,
            });
        }
        let count = u64::try_from(length).map_err(|_| Error::RangeOverflow)?;
        let end = offset.checked_add(count).ok_or(Error::RangeOverflow)?;
        if end > self.size {
            return Err(Error::OutOfBounds {
                offset,
                length,
                size: self.size,
            });
        }
        if length == 0 {
            return Ok(Vec::new());
        }
        let bytes = self
            .inner
            .read_at(offset, length)
            .await
            .map_err(Error::Source)?;
        if bytes.len() != length {
            return Err(Error::UnexpectedReadLength {
                expected: length,
                actual: bytes.len(),
            });
        }
        Ok(bytes)
    }
}

//! Platform-independent access to SqPack container files.
//!
//! This module supplies the I/O boundary, not a binary format implementation.
//! The core performs no I/O, index parsing or decompression. Optional `local`
//! and `web` modules provide asynchronous backends (behind same-named features).
//! Futures deliberately have no `Send` bound, so browser-local sources can use them.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;

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
    MissingFile(String),
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
            Self::MissingFile(file) => write!(f, "container file missing: {file}"),
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
}

impl<S: PackSource> SqPack<S> {
    /// Construct without a per-request byte cap. Performs no I/O.
    pub fn new(source: S) -> Self {
        Self {
            source,
            max_read_bytes: None,
        }
    }

    /// Set an optional per-request byte cap for subsequently opened readers.
    /// `None` (the default) imposes no cap; `Some(0)` allows only empty reads.
    /// Bounds, arithmetic and response-length checks always apply. This is not
    /// an aggregate-memory limit; set a cap before reading untrusted lengths.
    pub fn with_max_read_bytes(mut self, max_read_bytes: Option<usize>) -> Self {
        self.max_read_bytes = max_read_bytes;
        self
    }

    /// Open a container, not a resource inside it. Resource lookup is not yet
    /// implemented. Valid names are resolved by the source; confirmed absence
    /// becomes `MissingFile`, while backend failures remain `Source` errors.
    pub async fn open(&self, file: &str) -> Result<PackReader<S::File>, Error<S::Error>> {
        if !is_valid_file_name(file) {
            return Err(Error::InvalidFileName(file.to_owned()));
        }
        let inner = self
            .source
            .open(file)
            .await
            .map_err(Error::Source)?
            .ok_or_else(|| Error::MissingFile(file.to_owned()))?;
        let size = inner.len();
        Ok(PackReader {
            inner,
            size,
            max_read_bytes: self.max_read_bytes,
        })
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

//! Pure index/index2 parsing and enumeration, without DAT access or path recovery.
//!
//! Only the main hash table is interpreted. Synonym records are preserved as
//! flagged entries; collision-name tables and container SHA-1 are not decoded.

use std::fmt;

use super::{Error, PackReader, PackSource, ReadAt, SqPack, format};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexKind {
    Index,
    Index2,
}

impl IndexKind {
    fn stride(self) -> usize {
        match self {
            Self::Index => 16,
            Self::Index2 => 8,
        }
    }
}

/// Hashes are identifiers, not recoverable resource names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexHash {
    Index { directory: u32, filename: u32 },
    Index2(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub dat: u8,
    pub offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexEntry {
    pub hash: IndexHash,
    /// Original packed field, including the synonym flag.
    pub packed: u32,
}

impl IndexEntry {
    pub fn is_synonym(self) -> bool {
        self.packed & 1 != 0
    }

    /// Flagged entries do not directly identify a DAT location.
    pub fn location(self) -> Option<Location> {
        (!self.is_synonym()).then_some(Location {
            dat: ((self.packed >> 1) & 7) as u8,
            offset: u64::from(self.packed & !15) * 8,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub reason: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason)
    }
}

impl std::error::Error for ParseError {}

impl From<&'static str> for ParseError {
    fn from(reason: &'static str) -> Self {
        Self { reason }
    }
}

/// Validated main hash table. Borrowed parsing allocates nothing; asynchronous
/// loading owns only the table bytes. Entries retain file order and duplicates.
#[derive(Debug)]
pub struct Index<B = Vec<u8>> {
    table: B,
    kind: IndexKind,
}

impl<'a> Index<&'a [u8]> {
    /// Parse a complete container snapshot. The kind is explicit because both
    /// formats share the common header. Does not verify checksums or auxiliary
    /// tables; validates the headers and main table's bounds and record lengths.
    pub fn parse(bytes: &'a [u8], kind: IndexKind) -> Result<Self, ParseError> {
        let size = u64::try_from(bytes.len()).map_err(|_| "container too large")?;
        let start = format::sqpack_header(bytes, size, 2)?;
        let start = usize::try_from(start).map_err(|_| "header offset too large")?;
        let header = format::index_header(&bytes[start..], start as u64, size, kind.stride())?;
        let offset = usize::try_from(header.offset).map_err(|_| "table offset too large")?;
        // index_header has validated this span against the complete snapshot.
        Self::from_table(&bytes[offset..offset + header.length], kind)
    }
}

impl<B: AsRef<[u8]>> Index<B> {
    /// Parse just the main hash table, not a container. The caller is responsible
    /// for locating the table; only record alignment is validated here.
    pub fn from_table(table: B, kind: IndexKind) -> Result<Self, ParseError> {
        if !table.as_ref().len().is_multiple_of(kind.stride()) {
            return Err("partial index record".into());
        }
        Ok(Self { table, kind })
    }

    pub fn kind(&self) -> IndexKind {
        self.kind
    }

    pub fn len(&self) -> usize {
        self.table.as_ref().len() / self.kind.stride()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> impl ExactSizeIterator<Item = IndexEntry> + DoubleEndedIterator + '_ {
        self.table
            .as_ref()
            .chunks_exact(self.kind.stride())
            .map(|record| {
                // Fixed-size records were validated at construction.
                let (key, packed) = format::index_record(record, self.kind == IndexKind::Index2)
                    .expect("validated fixed-size index record");
                let hash = match self.kind {
                    IndexKind::Index => IndexHash::Index {
                        directory: (key >> 32) as u32,
                        filename: key as u32,
                    },
                    IndexKind::Index2 => IndexHash::Index2(key as u32),
                };
                IndexEntry { hash, packed }
            })
    }

    pub(super) fn find_entry(&self, hash: IndexHash) -> Result<Option<Location>, &'static str> {
        let mut found = None;
        for entry in self.entries().filter(|entry| entry.hash == hash) {
            if found.is_some() {
                return Err("ambiguous duplicate hash; collision resolution not supported");
            }
            found = Some(
                entry
                    .location()
                    .ok_or("flagged/synonym index entries not supported")?,
            );
        }
        Ok(found)
    }
}

impl<S: PackSource> SqPack<S> {
    /// Load the main table of a known index container, without opening DAT files.
    /// The configured read cap applies to the table and header reads. This does
    /// not discover other containers or resolve synonym names.
    pub async fn read_index(&self, file: &str, kind: IndexKind) -> Result<Index, Error<S::Error>> {
        let reader = self.open(file).await?;
        load(&reader, file, kind).await
    }
}

pub(super) async fn load<F: ReadAt>(
    reader: &PackReader<F>,
    file: &str,
    kind: IndexKind,
) -> Result<Index, Error<F::Error>> {
    let invalid = |reason| Error::InvalidData {
        file: file.to_owned(),
        reason,
    };
    let prefix = reader.read_at(0, 24).await?;
    let start = format::sqpack_header(&prefix, reader.len(), 2).map_err(invalid)?;
    let header = reader.read_at(start, 16).await?;
    let header =
        format::index_header(&header, start, reader.len(), kind.stride()).map_err(invalid)?;
    let table = reader.read_at(header.offset, header.length).await?;
    Index::from_table(table, kind).map_err(|e| invalid(e.reason))
}

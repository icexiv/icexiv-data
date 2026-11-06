//! Pure index/index2 parsing and enumeration, without DAT access or path recovery.
//!
//! Index2 collision names are interpreted; primary-index synonym tables and
//! container SHA-1 are not decoded.

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
/// loading owns the main and index2 auxiliary table bytes. Entries retain file
/// order and duplicates. Auxiliary tables default to a 16 MiB byte budget.
/// Lookup uses a lower-bound search when keys are ordered; unordered tables
/// retain linear lookup. Index2 containers also retain collision-name bytes.
#[derive(Debug)]
pub struct Index<B = Vec<u8>> {
    table: B,
    kind: IndexKind,
    sorted: bool,
    collisions: Option<B>,
}

/// Borrowed 256-byte index2 auxiliary record, including unknown and trailing bytes.
#[derive(Clone, Copy, Debug)]
pub struct CollisionRecord<'a> {
    raw: &'a [u8],
}

impl<'a> CollisionRecord<'a> {
    pub fn raw(self) -> &'a [u8] {
        self.raw
    }
    fn word(self, offset: usize) -> u32 {
        u32::from_le_bytes(
            self.raw[offset..offset + 4]
                .try_into()
                .expect("fixed record"),
        )
    }
    pub fn hash(self) -> u32 {
        self.word(0)
    }
    pub fn unknown(self) -> u32 {
        self.word(4)
    }
    pub fn packed(self) -> u32 {
        self.word(8)
    }
    pub fn sequence(self) -> u32 {
        self.word(12)
    }
    pub fn path(self) -> &'a [u8] {
        let field = &self.raw[16..];
        &field[..field.iter().position(|&b| b == 0).expect("validated NUL")]
    }
    pub fn is_sentinel(self) -> bool {
        self.hash() == u32::MAX
            && self.packed() == 0
            && self.sequence() == u32::MAX
            && self.raw[16] == 0
    }
}

const DEFAULT_COLLISION_BUDGET: usize = 16 << 20;

fn collision_span(
    header: &[u8],
    start: u64,
    size: u64,
    main: &format::IndexHeader,
    budget: usize,
) -> Result<(u64, usize), &'static str> {
    let word = |at: usize| -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            header
                .get(at..at + 4)
                .ok_or("truncated collision header")?
                .try_into()
                .map_err(|_| "truncated collision header")?,
        ))
    };
    let header_size = u64::from(word(0)?);
    if header_size < 92 {
        return Err("truncated collision header");
    }
    let offset = u64::from(word(84)?);
    let length = usize::try_from(word(88)?).map_err(|_| "collision table too large")?;
    if length > budget {
        return Err("collision table budget exceeded");
    }
    if !length.is_multiple_of(256) {
        return Err("partial collision record");
    }
    if length != 0 {
        let end = format::check_span(offset, length as u64, size)?;
        let header_end = format::check_span(start, header_size, size)?;
        if offset < header_end {
            return Err("collision table overlaps headers");
        }
        let main_end = main.offset + main.length as u64;
        if main.length != 0 && offset < main_end && main.offset < end {
            return Err("collision table overlaps main table");
        }
    }
    Ok((offset, length))
}

fn validate_collisions(bytes: &[u8]) -> Result<(), &'static str> {
    if !bytes.len().is_multiple_of(256) {
        return Err("partial collision record");
    }
    for (i, raw) in bytes.chunks_exact(256).enumerate() {
        let row = CollisionRecord { raw };
        if !raw[16..].contains(&0) {
            return Err("unterminated collision path");
        }
        if row.is_sentinel() {
            if (i + 1) * 256 != bytes.len() {
                return Err("collision sentinel is not final");
            }
        } else {
            if row.path().is_empty() {
                return Err("empty collision path");
            }
            if row.packed() & 1 != 0 {
                return Err("flagged collision location");
            }
            if format::hash(row.path()) != row.hash() {
                return Err("collision path hash mismatch");
            }
        }
    }
    Ok(())
}

fn record_key(record: &[u8], kind: IndexKind) -> u64 {
    let low = u32::from_le_bytes([record[0], record[1], record[2], record[3]]);
    if kind == IndexKind::Index2 {
        return u64::from(low);
    }
    let high = u32::from_le_bytes([record[4], record[5], record[6], record[7]]);
    u64::from(low) | (u64::from(high) << 32)
}

impl<'a> Index<&'a [u8]> {
    /// Parse a complete container snapshot. The kind is explicit because both
    /// formats share the common header. Validates main and index2 auxiliary
    /// spans, NUL-terminated collision names and their hashes. Does not verify
    /// checksums or primary-index synonym tables.
    pub fn parse(bytes: &'a [u8], kind: IndexKind) -> Result<Self, ParseError> {
        Self::parse_with_collision_budget(bytes, kind, DEFAULT_COLLISION_BUDGET)
    }

    /// Like `parse`, with an explicit maximum auxiliary table byte length.
    pub fn parse_with_collision_budget(
        bytes: &'a [u8],
        kind: IndexKind,
        budget: usize,
    ) -> Result<Self, ParseError> {
        let size = u64::try_from(bytes.len()).map_err(|_| "container too large")?;
        let start = format::sqpack_header(bytes, size, 2)?;
        let start = usize::try_from(start).map_err(|_| "header offset too large")?;
        let header = format::index_header(&bytes[start..], start as u64, size, kind.stride())?;
        let offset = usize::try_from(header.offset).map_err(|_| "table offset too large")?;
        // index_header has validated this span against the complete snapshot.
        let mut index = Self::from_table(&bytes[offset..offset + header.length], kind)?;
        if kind == IndexKind::Index2 {
            let (at, len) = collision_span(&bytes[start..], start as u64, size, &header, budget)?;
            let names = if len == 0 {
                &bytes[0..0]
            } else {
                let at = usize::try_from(at).map_err(|_| "collision offset too large")?;
                &bytes[at..at + len]
            };
            validate_collisions(names)?;
            index.collisions = if len == 0 { None } else { Some(names) };
        }
        Ok(index)
    }
}

impl<B: AsRef<[u8]>> Index<B> {
    /// Parse just the main hash table, not a container. The caller is responsible
    /// for locating the table; only record alignment is validated here.
    pub fn from_table(table: B, kind: IndexKind) -> Result<Self, ParseError> {
        if !table.as_ref().len().is_multiple_of(kind.stride()) {
            return Err("partial index record".into());
        }
        let sorted = table
            .as_ref()
            .chunks_exact(kind.stride())
            .map(|r| record_key(r, kind))
            .is_sorted();
        Ok(Self {
            table,
            kind,
            sorted,
            collisions: None,
        })
    }

    pub fn kind(&self) -> IndexKind {
        self.kind
    }

    /// Total bytes held by the main and auxiliary tables (used for cache budgets).
    pub fn byte_len(&self) -> usize {
        self.table.as_ref().len() + self.collisions.as_ref().map_or(0, |b| b.as_ref().len())
    }

    pub fn len(&self) -> usize {
        self.table.as_ref().len() / self.kind.stride()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// All auxiliary records, including a terminal sentinel and raw trailing bytes.
    /// `None` means no auxiliary bytes were supplied (`from_table`, primary index,
    /// or a zero-length auxiliary span).
    pub fn collision_records(&self) -> Option<impl ExactSizeIterator<Item = CollisionRecord<'_>>> {
        self.collisions.as_ref().map(|b| {
            b.as_ref()
                .chunks_exact(256)
                .map(|raw| CollisionRecord { raw })
        })
    }

    /// Look up a full resource path (ASCII case folded). Primary-index synonym
    /// records still report unsupported; index2 flagged entries use full names.
    pub fn find_path(&self, path: &str) -> Result<Option<Location>, ParseError> {
        let path = path.to_ascii_lowercase();
        let hash = match self.kind {
            IndexKind::Index2 => IndexHash::Index2(format::hash(path.as_bytes())),
            IndexKind::Index => {
                let (directory, filename) = path
                    .rsplit_once('/')
                    .ok_or("resource path lacks directory")?;
                IndexHash::Index {
                    directory: format::hash(directory.as_bytes()),
                    filename: format::hash(filename.as_bytes()),
                }
            }
        };
        self.find_named(hash, path.as_bytes()).map_err(Into::into)
    }

    pub(super) fn find_named(
        &self,
        hash: IndexHash,
        path: &[u8],
    ) -> Result<Option<Location>, &'static str> {
        match self.find_entry(hash) {
            Err("flagged/synonym index entries not supported")
                if self.kind == IndexKind::Index2 =>
            {
                if self
                    .entries()
                    .filter(|entry| entry.hash == hash)
                    .take(2)
                    .count()
                    > 1
                {
                    return Err("ambiguous duplicate hash; collision resolution not supported");
                }
                let rows = self
                    .collision_records()
                    .ok_or("collision name table unavailable")?;
                let mut found = None;
                for row in rows.filter(|row| {
                    !row.is_sentinel()
                        && IndexHash::Index2(row.hash()) == hash
                        && row.path() == path
                }) {
                    if found.is_some() {
                        return Err("ambiguous duplicate collision path");
                    }
                    found = IndexEntry {
                        hash,
                        packed: row.packed(),
                    }
                    .location();
                }
                Ok(found)
            }
            result => result,
        }
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
        if self.sorted {
            let key = match (self.kind, hash) {
                (
                    IndexKind::Index,
                    IndexHash::Index {
                        directory,
                        filename,
                    },
                ) => (u64::from(directory) << 32) | u64::from(filename),
                (IndexKind::Index2, IndexHash::Index2(key)) => u64::from(key),
                _ => return Ok(None),
            };
            let bytes = self.table.as_ref();
            let stride = self.kind.stride();
            let at = |i: usize| &bytes[i * stride..(i + 1) * stride];
            // Lower bound finds the first duplicate, preserving the linear
            // path's error precedence. The table is never reordered.
            let (mut low, mut high) = (0, self.len());
            while low < high {
                let middle = low + (high - low) / 2;
                if record_key(at(middle), self.kind) < key {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            if low == self.len() || record_key(at(low), self.kind) != key {
                return Ok(None);
            }
            let (_, packed) = format::index_record(at(low), self.kind == IndexKind::Index2)?;
            let location = IndexEntry { hash, packed }
                .location()
                .ok_or("flagged/synonym index entries not supported")?;
            if low + 1 < self.len() && record_key(at(low + 1), self.kind) == key {
                return Err("ambiguous duplicate hash; collision resolution not supported");
            }
            return Ok(Some(location));
        }
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

#[cfg(test)]
mod lookup_tests {
    use super::*;
    #[test]
    fn sorted_lookup_matches_linear_semantics_without_reordering() {
        for kind in [IndexKind::Index, IndexKind::Index2] {
            for keys in [
                vec![],
                vec![0],
                vec![0, 1, 1, 3, u32::MAX as u64],
                vec![3, 1, 1, 0],
                vec![0, 1, (1u64 << 32) + 1, u64::MAX],
            ] {
                for flagged in [None, Some(1usize), Some(2)] {
                    let mut table = Vec::new();
                    for (i, key) in keys.iter().enumerate() {
                        if kind == IndexKind::Index {
                            table.extend_from_slice(&key.to_le_bytes());
                        } else {
                            table.extend_from_slice(&(*key as u32).to_le_bytes());
                        }
                        table.extend_from_slice(
                            &(if flagged == Some(i) {
                                1u32
                            } else {
                                (i as u32 + 1) * 16
                            })
                            .to_le_bytes(),
                        );
                        if kind == IndexKind::Index {
                            table.extend_from_slice(&[0; 4]);
                        }
                    }
                    let fast = Index::from_table(table.clone(), kind).unwrap();
                    let mut linear = Index::from_table(table.clone(), kind).unwrap();
                    linear.sorted = false;
                    assert_eq!(fast.table, table);
                    for key in [0, 1, 2, 3, 4, u32::MAX as u64, (1u64 << 32) + 1, u64::MAX] {
                        let hash = if kind == IndexKind::Index {
                            IndexHash::Index {
                                directory: (key >> 32) as u32,
                                filename: key as u32,
                            }
                        } else {
                            IndexHash::Index2(key as u32)
                        };
                        assert_eq!(fast.find_entry(hash), linear.find_entry(hash));
                    }
                }
            }
        }
    }
}

impl<S: PackSource> SqPack<S> {
    /// Load a known index container's main and index2 collision tables, without
    /// opening DAT files. The configured read cap applies to each read; the
    /// auxiliary table has an additional 16 MiB cap. No container discovery or
    /// primary-index synonym resolution is performed.
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
    let raw_header = reader
        .read_at(start, if kind == IndexKind::Index2 { 92 } else { 16 })
        .await?;
    let header =
        format::index_header(&raw_header, start, reader.len(), kind.stride()).map_err(invalid)?;
    let auxiliary = if kind == IndexKind::Index2 {
        let (at, length) = collision_span(
            &raw_header,
            start,
            reader.len(),
            &header,
            DEFAULT_COLLISION_BUDGET,
        )
        .map_err(invalid)?;
        let bytes = if length == 0 {
            Vec::new()
        } else {
            reader.read_at(at, length).await?
        };
        validate_collisions(&bytes).map_err(invalid)?;
        if length == 0 { None } else { Some(bytes) }
    } else {
        None
    };
    let table = reader.read_at(header.offset, header.length).await?;
    let mut index = Index::from_table(table, kind).map_err(|e| invalid(e.reason))?;
    index.collisions = auxiliary;
    Ok(index)
}

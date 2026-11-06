//! Type-4 group metadata and complete selected-group bytes.
use super::format::{self, EntryHeader, LodBlock};

/// An independently decoded type-4 data group with its unchanged file prefix.
///
/// Groups are container units: a group is not generally a TEX mip. The full
/// entry's declared length is retained even when other groups are incomplete.
/// Successful extraction validates all group tables and physical ranges, but
/// only the selected group's block headers and DEFLATE streams.
#[derive(Clone, Debug)]
pub struct TextureGroup {
    pub(crate) prefix: Vec<u8>,
    pub(crate) data: Vec<u8>,
    pub(crate) index: usize,
    pub(crate) count: usize,
    pub(crate) declared_file_size: usize,
}

impl TextureGroup {
    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn group_count(&self) -> usize {
        self.count
    }
    pub fn declared_file_size(&self) -> usize {
        self.declared_file_size
    }

    /// Bind this group as a complete mip of the one-group-per-mip volume profile.
    ///
    /// Requires an 80-byte TEX header, volume layout, a group for every original
    /// mip, contiguous dimension-derived logical ranges (or the explicit repeated
    /// offset-80 volume profile), and an exact full logical size declaration.
    /// The selected mip must contain every slice; a short group is an error.
    /// This neither constructs a TEX file nor validates other groups' pixels.
    pub fn volume_mip(&self) -> std::result::Result<crate::tex::Mip<'_>, crate::tex::Error> {
        use crate::tex::{Error, Header, Layout};
        if self.prefix.len() != 80 {
            return Err(Error::InvalidData(
                "volume group requires an 80-byte TEX prefix",
            ));
        }
        let header = match Header::parse(&self.prefix) {
            Ok(header) => header,
            Err(error) => Header::parse_packed_volume(&self.prefix).map_err(|_| error)?,
        };
        if header.layout() != Layout::Volume || header.mip_count() != self.count {
            return Err(Error::InvalidData(
                "not the one-group-per-mip volume profile",
            ));
        }
        let mut previous = 80;
        for index in 0..header.mip_count() {
            let range = header.mip_range(index)?;
            if range.start != previous {
                return Err(Error::InvalidData("volume mip ranges are not contiguous"));
            }
            previous = range.end;
        }
        if header.encoded_len() != self.declared_file_size {
            return Err(Error::InvalidData(
                "volume logical size disagrees with entry declaration",
            ));
        }
        header.mip(self.index, &self.data)
    }
}

type Result<T> = std::result::Result<T, &'static str>;

/// Number of u16 block spans required by the group table, bounded by the entry
/// header before the caller reads or allocates the span table.
pub(super) fn span_count(table: &[u8], entry: &EntryHeader) -> Result<usize> {
    if table.len() != entry.table_size || table.is_empty() || !table.len().is_multiple_of(20) {
        return Err("invalid texture group table");
    }
    let mut count = 0usize;
    for record in table.as_chunks::<20>().0 {
        let group = format::lod_block(record)?;
        if group.block_count == 0 || group.block_offset as usize != count {
            return Err("texture first block index or count mismatch");
        }
        count = count
            .checked_add(group.block_count as usize)
            .ok_or("texture block count overflow")?;
    }
    let size = count
        .checked_mul(2)
        .and_then(|n| n.checked_add(entry.table_size))
        .and_then(|n| n.checked_add(24))
        .ok_or("texture span table overflow")?;
    if size > entry.header_size {
        return Err("texture span table exceeds entry header");
    }
    Ok(count)
}

pub(super) struct Table<'a> {
    table: &'a [u8],
    spans: &'a [u8],
    pub prefix_size: usize,
    pub undeclared_size: usize,
}

impl<'a> Table<'a> {
    pub fn parse(
        table: &'a [u8],
        spans: &'a [u8],
        entry: &EntryHeader,
        file_size: u64,
    ) -> Result<Self> {
        if spans.len() != span_count(table, entry)? * 2 {
            return Err("truncated texture span table");
        }
        let prefix_size = format::lod_block(&table[..20])?.compressed_offset as usize;
        if prefix_size > entry.output_size {
            return Err("texture prefix exceeds logical size");
        }
        let mut result = Self {
            table,
            spans,
            prefix_size,
            undeclared_size: 0,
        };
        let mut previous_end = entry
            .body_offset
            .checked_add(prefix_size as u64)
            .ok_or("texture prefix overflow")?;
        format::check_span(entry.body_offset, prefix_size as u64, file_size)?;
        let mut declared = prefix_size;
        for i in 0..result.len() {
            let group = result.group(i)?;
            let start = entry
                .body_offset
                .checked_add(u64::from(group.compressed_offset))
                .ok_or("texture group offset overflow")?;
            if start < previous_end {
                return Err("texture groups overlap or are out of order");
            }
            previous_end = format::check_span(start, u64::from(group.compressed_size), file_size)?;
            let mut sum = 0u64;
            for span in result.spans(&group) {
                if span < 16 {
                    return Err("texture block span shorter than header");
                }
                sum = sum
                    .checked_add(u64::from(span))
                    .ok_or("texture group span overflow")?;
            }
            if sum != u64::from(group.compressed_size) {
                return Err("texture group stored span mismatch");
            }
            declared = declared
                .checked_add(group.decompressed_size as usize)
                .ok_or("texture group output overflow")?;
            if declared > entry.output_size {
                return Err("texture groups exceed logical size");
            }
        }
        result.undeclared_size = entry.output_size - declared;
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.table.len() / 20
    }
    pub fn group(&self, index: usize) -> Result<LodBlock> {
        let start = index
            .checked_mul(20)
            .ok_or("texture group index overflow")?;
        format::lod_block(
            self.table
                .get(
                    start
                        ..start
                            .checked_add(20)
                            .ok_or("texture group index overflow")?,
                )
                .ok_or("texture group index out of bounds")?,
        )
    }
    pub fn spans(&self, group: &LodBlock) -> impl Iterator<Item = u16> + '_ {
        let start = group.block_offset as usize * 2;
        let end = start + group.block_count as usize * 2;
        self.spans[start..end]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
    }
}

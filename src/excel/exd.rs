//! Borrowed EXD pages, typed fields and compound (parent/subrow) IDs.

use super::{ColumnType, Error, Exh, Result, Variant, read, slice, table_end};
use binrw::{BinRead, Endian};

#[derive(BinRead)]
#[br(big, magic = b"EXDF")]
struct HeaderRaw {
    version: u16,
    _unknown: u16,
    index_size: u32,
    _reserved: [u8; 20],
}

#[derive(BinRead)]
#[br(big)]
struct IndexRaw {
    id: u32,
    offset: u32,
}

#[derive(BinRead)]
#[br(big)]
struct RowHeaderRaw {
    data_size: u32,
    subrows: u16,
}

struct RowIndex {
    id: u32,
    offset: usize,
    end: usize,
    subrows: u16,
}

/// A page borrows its immutable bytes and EXH. Only validated row metadata is
/// allocated; row payloads and string values are not copied.
pub struct Exd<'data, 'header> {
    bytes: &'data [u8],
    header: &'header Exh,
    index: Vec<RowIndex>,
    row_count: usize,
}

impl<'data, 'header> Exd<'data, 'header> {
    pub fn parse(bytes: &'data [u8], header: &'header Exh) -> Result<Self> {
        let raw: HeaderRaw = read(bytes, Endian::Big)?;
        if raw.version != 2 {
            return Err(Error::UnsupportedVersion {
                format: "EXD",
                version: raw.version,
            });
        }
        let index_size = usize::try_from(raw.index_size)
            .map_err(|_| Error::InvalidData("index size overflow"))?;
        if !index_size.is_multiple_of(8) {
            return Err(Error::InvalidData("partial row index record"));
        }
        let data_start = table_end(bytes, 32, index_size / 8, 8)?;
        let mut index = Vec::new();
        index
            .try_reserve_exact(index_size / 8)
            .map_err(|_| Error::AllocationFailed)?;
        let mut row_count = 0usize;
        for record in bytes[32..data_start].as_chunks::<8>().0 {
            let record: IndexRaw = read(record, Endian::Big)?;
            let offset = usize::try_from(record.offset)
                .map_err(|_| Error::InvalidData("row offset overflow"))?;
            if offset < data_start {
                return Err(Error::InvalidData("row overlaps page header/index"));
            }
            let row: RowHeaderRaw = read(slice(bytes, offset, 6)?, Endian::Big)?;
            let payload_start = offset
                .checked_add(6)
                .ok_or(Error::InvalidData("row offset overflow"))?;
            let payload_size = usize::try_from(row.data_size)
                .map_err(|_| Error::InvalidData("row size overflow"))?;
            let payload = slice(bytes, payload_start, payload_size)?;
            let fixed = usize::from(header.fixed_row_size());
            match header.variant() {
                Variant::Default => {
                    if row.subrows != 1 {
                        return Err(Error::InvalidData("ordinary row must have count one"));
                    }
                    slice(payload, 0, fixed)?;
                }
                Variant::Subrows => {
                    let stride = fixed
                        .checked_add(2)
                        .ok_or(Error::InvalidData("subrow stride overflow"))?;
                    let fixed_end = table_end(payload, 0, usize::from(row.subrows), stride)?;
                    // ID values need not equal physical positions or be contiguous.
                    let mut seen = [0u64; 1024];
                    for subrow in payload[..fixed_end].chunks_exact(stride) {
                        let id: u16 = read(subrow, Endian::Big)?;
                        let word = usize::from(id / 64);
                        let bit = 1u64 << (id % 64);
                        if seen[word] & bit != 0 {
                            return Err(Error::InvalidData("duplicate subrow ID"));
                        }
                        seen[word] |= bit;
                    }
                }
            }
            let end = payload_start
                .checked_add(payload_size)
                .ok_or(Error::InvalidData("row end overflow"))?;
            row_count = row_count
                .checked_add(usize::from(row.subrows))
                .ok_or(Error::InvalidData("row count overflow"))?;
            index.push(RowIndex {
                id: record.id,
                offset,
                end,
                subrows: row.subrows,
            });
        }
        // Reject aliases/overlaps without assuming the input index is sorted.
        index.sort_unstable_by_key(|row| row.offset);
        if index.windows(2).any(|pair| pair[0].end > pair[1].offset) {
            return Err(Error::InvalidData("overlapping row records"));
        }
        index.sort_unstable_by_key(|row| row.id);
        if index.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(Error::InvalidData("duplicate parent row ID"));
        }
        Ok(Self {
            bytes,
            header,
            index,
            row_count,
        })
    }

    /// Number of values exposed by `rows()`, including every subrow.
    pub fn len(&self) -> usize {
        self.row_count
    }
    pub fn is_empty(&self) -> bool {
        self.row_count == 0
    }
    pub fn parent_row_count(&self) -> usize {
        self.index.len()
    }

    /// Iterate in ascending parent ID order, retaining physical subrow order.
    pub fn rows(&self) -> impl Iterator<Item = Row<'data, 'header>> + '_ {
        self.index
            .iter()
            .flat_map(move |row| (0..usize::from(row.subrows)).map(move |i| self.make_row(row, i)))
    }

    /// Lookup an ordinary row. A subrow sheet requires `subrow` instead.
    pub fn row(&self, id: u32) -> Result<Option<Row<'data, 'header>>> {
        if self.header.variant() != Variant::Default {
            return Err(Error::VariantMismatch);
        }
        Ok(self.find(id).map(|row| self.make_row(row, 0)))
    }

    /// Lookup by the stored compound key, not the subrow's physical position.
    pub fn subrow(&self, id: u32, subrow_id: u16) -> Result<Option<Row<'data, 'header>>> {
        if self.header.variant() != Variant::Subrows {
            return Err(Error::VariantMismatch);
        }
        Ok(self.find(id).and_then(|row| {
            (0..usize::from(row.subrows))
                .map(|i| self.make_row(row, i))
                .find(|row| row.subrow_id == Some(subrow_id))
        }))
    }

    fn find(&self, id: u32) -> Option<&RowIndex> {
        self.index
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|i| &self.index[i])
    }

    fn make_row(&self, row: &RowIndex, position: usize) -> Row<'data, 'header> {
        // All arithmetic/slices below are bounded by parse's checked fixed area;
        // callers pass only positions in 0..subrows. Borrowed bytes are immutable.
        let fixed = usize::from(self.header.fixed_row_size());
        let payload_start = row.offset + 6;
        let (start, pool_start, subrow_id) = match self.header.variant() {
            Variant::Default => (payload_start, payload_start + fixed, None),
            Variant::Subrows => {
                let stride = fixed + 2;
                let key = payload_start + position * stride;
                let id = u16::from_be_bytes([self.bytes[key], self.bytes[key + 1]]);
                (
                    key + 2,
                    payload_start + usize::from(row.subrows) * stride,
                    Some(id),
                )
            }
        };
        let end = start + fixed;
        Row {
            id: row.id,
            subrow_id,
            header: self.header,
            fixed: &self.bytes[start..end],
            tail: &self.bytes[end..row.end],
            minimum_string_offset: pool_start - end,
        }
    }
}

/// One ordinary row or subrow; strings borrow the containing parent row.
#[derive(Clone, Copy, Debug)]
pub struct Row<'data, 'header> {
    id: u32,
    subrow_id: Option<u16>,
    header: &'header Exh,
    fixed: &'data [u8],
    tail: &'data [u8],
    minimum_string_offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value<'data> {
    /// Raw bytes without NUL; no UTF-8 conversion or SeString interpretation.
    String(&'data [u8]),
    Bool(bool),
    Int8(i8),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Float32(f32),
    Int64(i64),
    UInt64(u64),
}

impl<'data> Row<'data, '_> {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn subrow_id(&self) -> Option<u16> {
        self.subrow_id
    }
    pub fn fields(&self) -> impl ExactSizeIterator<Item = Result<Value<'data>>> + '_ {
        (0..self.header.columns().len()).map(|i| self.field(i))
    }

    /// Decode a numeric column index from EXH, not a schema field name.
    pub fn field(&self, index: usize) -> Result<Value<'data>> {
        let column = self
            .header
            .columns()
            .get(index)
            .ok_or(Error::ColumnOutOfBounds {
                index,
                count: self.header.columns().len(),
            })?;
        let bytes = slice(
            self.fixed,
            usize::from(column.offset),
            column.data_type.width(),
        )?;
        Ok(match column.data_type {
            ColumnType::String => {
                let offset = usize::try_from(read::<u32>(bytes, Endian::Big)?)
                    .map_err(|_| Error::InvalidData("string offset overflow"))?;
                if offset < self.minimum_string_offset {
                    return Err(Error::InvalidData("string points into subrow fixed data"));
                }
                let tail = self
                    .tail
                    .get(offset..)
                    .ok_or(Error::InvalidData("string offset exceeds parent row"))?;
                let end = tail
                    .iter()
                    .position(|&byte| byte == 0)
                    .ok_or(Error::InvalidData("unterminated row string"))?;
                Value::String(&tail[..end])
            }
            ColumnType::Bool => Value::Bool(read::<u8>(bytes, Endian::Big)? != 0),
            ColumnType::PackedBool(bit) => {
                Value::Bool(read::<u8>(bytes, Endian::Big)? & (1 << bit) != 0)
            }
            ColumnType::Int8 => Value::Int8(read(bytes, Endian::Big)?),
            ColumnType::UInt8 => Value::UInt8(read(bytes, Endian::Big)?),
            ColumnType::Int16 => Value::Int16(read(bytes, Endian::Big)?),
            ColumnType::UInt16 => Value::UInt16(read(bytes, Endian::Big)?),
            ColumnType::Int32 => Value::Int32(read(bytes, Endian::Big)?),
            ColumnType::UInt32 => Value::UInt32(read(bytes, Endian::Big)?),
            ColumnType::Float32 => Value::Float32(read(bytes, Endian::Big)?),
            ColumnType::Int64 => Value::Int64(read(bytes, Endian::Big)?),
            ColumnType::UInt64 => Value::UInt64(read(bytes, Endian::Big)?),
        })
    }
}

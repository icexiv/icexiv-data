//! Byte-only SqPack parsing. Documentary sources and limitations are recorded
//! in docs/sqpack-provenance.md. No filesystem, HTTP or async dependencies here.

use binrw::{BinRead, Endian};
use flate2::{Decompress, FlushDecompress, Status};
use std::io::Cursor;

type Result<T> = std::result::Result<T, &'static str>;

// Only fixed-size layouts are read here. Validate ranges, counts and budgets
// before obtaining variable-size tables; never let a file count allocate a Vec.
fn read<T>(bytes: &[u8]) -> Result<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    T::read_options(&mut Cursor::new(bytes), Endian::Little, ())
        .map_err(|_| "invalid or truncated binary structure")
}

#[derive(BinRead)]
#[br(little, magic = b"SqPack\0\0")]
struct SqPackHeaderRaw {
    #[br(pad_after = 3)]
    platform: u8,
    size: u32,
    version: u32,
    kind: u32,
}

#[derive(BinRead)]
#[br(little)]
struct IndexHeaderRaw {
    size: u32,
    version: u32,
    table_offset: u32,
    table_size: u32,
}

#[derive(BinRead)]
#[br(little)]
struct IndexEntryRaw {
    hash: u64,
    packed: u32,
    _padding: u32,
}

#[derive(BinRead)]
#[br(little)]
struct Index2EntryRaw {
    hash: u32,
    packed: u32,
}

#[derive(BinRead)]
#[br(little)]
struct EntryHeaderRaw {
    size: u32,
    kind: u32,
    output_size: u32,
    #[br(pad_before = 8)]
    block_count: u32,
}

#[derive(BinRead)]
#[br(little)]
struct BlockRecordRaw {
    offset: u32,
    stored_size: u16,
    output_size: u16,
}

#[derive(BinRead)]
#[br(little)]
struct BlockHeaderRaw {
    size: u32,
    #[br(pad_before = 4)]
    compressed_size: u32,
    output_size: u32,
}

pub(super) fn check_span(offset: u64, length: u64, size: u64) -> Result<u64> {
    let end = offset.checked_add(length).ok_or("file range overflow")?;
    if end > size {
        return Err("file range out of bounds");
    }
    Ok(end)
}

/// Validate the shared header prefix; only Win32 little-endian version 1.
pub(super) fn sqpack_header(bytes: &[u8], size: u64, kind: u32) -> Result<u64> {
    let header: SqPackHeaderRaw = read(bytes)?;
    if header.platform != 0 {
        return Err("unsupported SqPack platform");
    }
    if header.version != 1 {
        return Err("unsupported SqPack version");
    }
    if header.kind != kind {
        return Err("unexpected SqPack container type");
    }
    let length = u64::from(header.size);
    if length < 24 {
        return Err("SqPack header too short");
    }
    check_span(0, length, size)?;
    Ok(length)
}

pub(super) struct IndexHeader {
    pub offset: u64,
    pub length: usize,
}

pub(super) fn index_header(
    bytes: &[u8],
    start: u64,
    file_size: u64,
    stride: usize,
) -> Result<IndexHeader> {
    let header: IndexHeaderRaw = read(bytes)?;
    let length = u64::from(header.size);
    if length < 16 {
        return Err("index header too short");
    }
    if header.version != 1 {
        return Err("unsupported index version");
    }
    let header_end = check_span(start, length, file_size)?;
    let offset = u64::from(header.table_offset);
    let table_length = u64::from(header.table_size);
    if offset < header_end {
        return Err("index table overlaps headers");
    }
    check_span(offset, table_length, file_size)?;
    let length = usize::try_from(table_length).map_err(|_| "index table too large")?;
    if !length.is_multiple_of(stride) {
        return Err("partial index record");
    }
    Ok(IndexHeader { offset, length })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Location {
    pub dat: u8,
    pub offset: u64,
}

pub(super) enum LookupError {
    Malformed(&'static str),
    Unsupported(&'static str),
}

pub(super) fn find_entry(
    table: &[u8],
    hash: u64,
    index2: bool,
) -> std::result::Result<Option<Location>, LookupError> {
    let stride = if index2 { 8 } else { 16 };
    if !table.len().is_multiple_of(stride) {
        return Err(LookupError::Malformed("partial index record"));
    }
    let mut found = None;
    for entry in table.chunks_exact(stride) {
        let (key, packed) = if index2 {
            let entry: Index2EntryRaw = read(entry).map_err(LookupError::Malformed)?;
            (u64::from(entry.hash), entry.packed)
        } else {
            let entry: IndexEntryRaw = read(entry).map_err(LookupError::Malformed)?;
            (entry.hash, entry.packed)
        };
        if key != hash {
            continue;
        }
        if found.is_some() {
            return Err(LookupError::Unsupported(
                "ambiguous duplicate hash; collision resolution not supported",
            ));
        }
        if packed & 1 != 0 {
            return Err(LookupError::Unsupported(
                "flagged/synonym index entries not supported",
            ));
        }
        found = Some(Location {
            dat: ((packed >> 1) & 7) as u8,
            offset: u64::from(packed & !15) * 8,
        });
    }
    Ok(found)
}

/// Reflected CRC-32/JAMCRC (init all ones, no final xor).
pub(super) fn hash(bytes: &[u8]) -> u32 {
    !crc32fast::hash(bytes)
}

/// The data header's length prefix determines the first possible entry offset.
pub(super) fn data_start(bytes: &[u8], start: u64, file_size: u64) -> Result<u64> {
    let length = u64::from(read::<u32>(bytes)?);
    if length < 32 {
        return Err("data header too short");
    }
    check_span(start, length, file_size)
}

pub(super) struct EntryHeader {
    pub kind: u32,
    pub output_size: usize,
    pub body_offset: u64,
    pub table_size: usize,
}

pub(super) fn entry_header(bytes: &[u8], start: u64, file_size: u64) -> Result<EntryHeader> {
    let header: EntryHeaderRaw = read(bytes)?;
    let length = u64::from(header.size);
    if length < 128 {
        return Err("data entry header too short");
    }
    let body_offset = check_span(start, length, file_size)?;
    let kind = header.kind;
    let output_size = usize::try_from(header.output_size).map_err(|_| "resource too large")?;
    let count = u64::from(header.block_count);
    let table_size = if kind == 2 {
        let table_size = count.checked_mul(8).ok_or("block table size overflow")?;
        if table_size > length - 24 {
            return Err("block table exceeds entry header");
        }
        usize::try_from(table_size).map_err(|_| "block table too large")?
    } else {
        0
    };
    Ok(EntryHeader {
        kind,
        output_size,
        body_offset,
        table_size,
    })
}

pub(super) struct Block {
    pub offset: u64,
    pub stored_size: usize,
    pub output_size: usize,
}

pub(super) fn block(bytes: &[u8], base: u64, file_size: u64) -> Result<Block> {
    let record: BlockRecordRaw = read(bytes)?;
    let offset = base
        .checked_add(u64::from(record.offset))
        .ok_or("block offset overflow")?;
    let stored_size = usize::from(record.stored_size);
    let output_size = usize::from(record.output_size);
    if stored_size < 16 {
        return Err("stored block shorter than its header");
    }
    check_span(offset, stored_size as u64, file_size)?;
    Ok(Block {
        offset,
        stored_size,
        output_size,
    })
}

pub(super) fn validate_blocks(table: &[u8], entry: &EntryHeader, file_size: u64) -> Result<()> {
    if table.len() != entry.table_size || !table.len().is_multiple_of(8) {
        return Err("truncated block table");
    }
    let mut total = 0usize;
    for record in table.as_chunks::<8>().0 {
        let block = block(record, entry.body_offset, file_size)?;
        total = total
            .checked_add(block.output_size)
            .ok_or("decoded size overflow")?;
    }
    if total != entry.output_size {
        return Err("block sizes disagree with resource size");
    }
    Ok(())
}

pub(super) fn decode_block(bytes: &[u8], expected: usize) -> Result<Vec<u8>> {
    let header: BlockHeaderRaw = read(bytes)?;
    let header_size = usize::try_from(header.size).map_err(|_| "block header too large")?;
    if header_size < 16 || header_size > bytes.len() {
        return Err("invalid block header size");
    }
    let compressed_size = header.compressed_size;
    let decoded_size = usize::try_from(header.output_size).map_err(|_| "block output too large")?;
    if decoded_size != expected || expected > usize::from(u16::MAX) {
        return Err("block output size mismatch");
    }
    let stored = if compressed_size == 32000 {
        expected
    } else {
        usize::try_from(compressed_size).map_err(|_| "compressed block too large")?
    };
    let end = header_size
        .checked_add(stored)
        .ok_or("block payload overflow")?;
    let input = bytes
        .get(header_size..end)
        .ok_or("truncated block payload")?;
    if compressed_size == 32000 {
        return Ok(input.to_vec());
    }

    // One extra output byte detects expansion beyond the advertised block size.
    // The table's u16 size bounds this temporary allocation to 64 KiB.
    let mut output = vec![0; expected + 1];
    let mut decoder = Decompress::new(false);
    let status = decoder
        .decompress(input, &mut output, FlushDecompress::Finish)
        .map_err(|_| "invalid raw DEFLATE stream")?;
    if status != Status::StreamEnd
        || decoder.total_in() != input.len() as u64
        || decoder.total_out() != expected as u64
    {
        return Err("DEFLATE stream length or decoded size mismatch");
    }
    output.truncate(expected);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jamcrc_matches_documented_and_independently_computed_vectors() {
        assert_eq!(hash(b""), u32::MAX);
        assert_eq!(hash(b"exd"), 0xe39b_7999);
        assert_eq!(hash(b"item.exh"), 0xdebb_8979);
        assert_eq!(hash(b"exd/item.exh"), 0xb118_d1a9);
        assert_eq!(hash(b"123456789"), 0x340b_c6d9);
    }

    fn assert_fixed_width<T>(bytes: &[u8])
    where
        T: for<'a> BinRead<Args<'a> = ()>,
    {
        for length in 0..bytes.len() {
            assert!(
                read::<T>(&bytes[..length]).is_err(),
                "prefix length {length}"
            );
        }
        let mut cursor = Cursor::new(bytes);
        assert!(T::read_options(&mut cursor, Endian::Little, ()).is_ok());
        assert_eq!(cursor.position(), bytes.len() as u64);
    }

    #[test]
    fn declarative_layouts_consume_the_exact_fixed_prefix_and_reject_short_reads() {
        let mut header = [0; 24];
        header[..8].copy_from_slice(b"SqPack\0\0");
        assert_fixed_width::<SqPackHeaderRaw>(&header);
        assert_fixed_width::<IndexHeaderRaw>(&[0; 16]);
        assert_fixed_width::<IndexEntryRaw>(&[0; 16]);
        assert_fixed_width::<Index2EntryRaw>(&[0; 8]);
        assert_fixed_width::<EntryHeaderRaw>(&[0; 24]);
        assert_fixed_width::<BlockRecordRaw>(&[0; 8]);
        assert_fixed_width::<BlockHeaderRaw>(&[0; 16]);
    }

    #[test]
    fn declarative_magic_validation_rejects_an_incorrect_signature() {
        assert!(read::<SqPackHeaderRaw>(&[0; 24]).is_err());
    }

    fn compressed_block() -> Vec<u8> {
        // Raw DEFLATE generated independently using Python zlib, wbits=-15.
        let payload = [203, 72, 205, 201, 201, 87, 8, 46, 12, 72, 76, 206, 6, 0];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(&payload);
        bytes
    }

    #[test]
    fn independently_encoded_raw_deflate_decodes_exactly() {
        assert_eq!(
            decode_block(&compressed_block(), 12).unwrap(),
            b"hello SqPack"
        );
    }

    #[test]
    fn deflate_rejects_overexpansion_truncation_and_trailing_compressed_bytes() {
        let mut bytes = compressed_block();
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        assert!(decode_block(&bytes, 1).is_err());
        let mut bytes = compressed_block();
        bytes.pop();
        assert!(decode_block(&bytes, 12).is_err());
        let mut bytes = compressed_block();
        bytes[8..12].copy_from_slice(&15u32.to_le_bytes());
        bytes.push(0);
        assert!(decode_block(&bytes, 12).is_err());
    }

    #[test]
    fn fixed_headers_reject_truncation_without_panics() {
        for length in 0..24 {
            let bytes = vec![0; length];
            assert!(sqpack_header(&bytes, 2048, 2).is_err());
            assert!(entry_header(&bytes, 2048, 4096).is_err());
            assert!(decode_block(&bytes, 1).is_err());
        }
        assert!(read::<u32>(&[]).is_err());
        assert!(check_span(u64::MAX, 1, u64::MAX).is_err());
    }
}

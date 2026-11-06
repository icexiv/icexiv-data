//! Newly authored synthetic containers, not extracted game files or copied tests.

use icexiv_data::sqpack::index::{Index, IndexHash, IndexKind, Location};
use icexiv_data::sqpack::{Error, PackSource, ReadAt, SqPack};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

const PATH: &str = "exd/item.exh";
const HASH: u64 = 0xe39b_7999_debb_8979;
const HASH2: u64 = 0xb118_d1a9;
const STEM: &str = "ffxiv/0a0000.win32";

fn block_on<T>(future: impl Future<Output = T>) -> T {
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

type Reads = Rc<RefCell<Vec<(String, u64, usize)>>>;

#[derive(Default)]
struct Memory {
    files: HashMap<String, Rc<Vec<u8>>>,
    fail: HashMap<String, &'static str>,
    opens: Rc<RefCell<Vec<String>>>,
    reads: Reads,
    fail_read_at: Rc<RefCell<Option<u64>>>,
}

struct MemoryFile {
    name: String,
    bytes: Rc<Vec<u8>>,
    reads: Reads,
    fail_read_at: Rc<RefCell<Option<u64>>>,
}

impl Memory {
    fn insert(&mut self, name: String, bytes: Vec<u8>) {
        self.files.insert(name, Rc::new(bytes));
    }
    fn mutate(&mut self, name: &str) -> &mut Vec<u8> {
        Rc::make_mut(self.files.get_mut(name).unwrap())
    }
}

impl PackSource for Memory {
    type Error = &'static str;
    type File = MemoryFile;
    async fn open(&self, name: &str) -> Result<Option<MemoryFile>, Self::Error> {
        self.opens.borrow_mut().push(name.to_owned());
        if let Some(&error) = self.fail.get(name) {
            return Err(error);
        }
        Ok(self.files.get(name).map(|bytes| MemoryFile {
            name: name.to_owned(),
            bytes: bytes.clone(),
            reads: self.reads.clone(),
            fail_read_at: self.fail_read_at.clone(),
        }))
    }
}

impl ReadAt for MemoryFile {
    type Error = &'static str;
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Self::Error> {
        if *self.fail_read_at.borrow() == Some(offset) {
            return Err("injected read failure");
        }
        self.reads
            .borrow_mut()
            .push((self.name.clone(), offset, length));
        let offset = usize::try_from(offset).map_err(|_| "offset overflow")?;
        let end = offset.checked_add(length).ok_or("range overflow")?;
        Ok(self.bytes.get(offset..end).ok_or("out of bounds")?.to_vec())
    }
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn container(kind: u32) -> Vec<u8> {
    let mut bytes = vec![0; 0x800];
    bytes[..8].copy_from_slice(b"SqPack\0\0");
    put32(&mut bytes, 12, 0x400);
    put32(&mut bytes, 16, 1);
    put32(&mut bytes, 20, kind);
    put32(&mut bytes, 0x400, 0x400);
    bytes
}

fn index(index2: bool, records: &[(u64, u32)]) -> Vec<u8> {
    let mut bytes = container(2);
    let stride = if index2 { 8 } else { 16 };
    put32(&mut bytes, 0x404, 1);
    put32(&mut bytes, 0x408, 0x800);
    put32(&mut bytes, 0x40c, (records.len() * stride) as u32);
    bytes.resize(0x800 + records.len() * stride, 0);
    for (i, &(hash, location)) in records.iter().enumerate() {
        let start = 0x800 + i * stride;
        put32(&mut bytes, start, hash as u32);
        if !index2 {
            put32(&mut bytes, start + 4, (hash >> 32) as u32);
        }
        put32(&mut bytes, start + if index2 { 4 } else { 8 }, location);
    }
    bytes
}

fn collision_index(paths: &[(&str, u32)], sentinel_unknown: u32) -> Vec<u8> {
    let hash = !crc32fast::hash(paths[0].0.as_bytes());
    let mut bytes = index(true, &[(u64::from(hash), 1)]);
    let offset = bytes.len();
    put32(&mut bytes, 0x454, offset as u32);
    put32(&mut bytes, 0x458, ((paths.len() + 1) * 256) as u32);
    for (sequence, &(path, packed)) in paths.iter().enumerate() {
        let start = bytes.len();
        bytes.resize(start + 256, 0xa5); // NUL trailing bytes are not padding.
        put32(&mut bytes, start, hash);
        put32(&mut bytes, start + 4, 0x12345678);
        put32(&mut bytes, start + 8, packed);
        put32(&mut bytes, start + 12, sequence as u32);
        bytes[start + 16..start + 16 + path.len()].copy_from_slice(path.as_bytes());
        bytes[start + 16 + path.len()] = 0;
    }
    let start = bytes.len();
    bytes.resize(start + 256, 0x5a);
    put32(&mut bytes, start, u32::MAX);
    put32(&mut bytes, start + 4, sentinel_unknown);
    put32(&mut bytes, start + 8, 0);
    put32(&mut bytes, start + 12, u32::MAX);
    bytes[start + 16] = 0;
    bytes
}

#[test]
fn index2_names_preserve_wire_bytes_and_reject_ambiguous_paths() {
    // Real observed pair; no game bytes embedded. Their independent JAMCRC is
    // fixed by the read-only inventory, not selected by this synthetic writer.
    let a = "bgcommon/world/common/vfx_for_bg/texture/bomb101zy.atex";
    let b = "bgcommon/world/evt/shared/for_pln/sgpl_a_evt_vfx01_bomber_v.sgb";
    assert_eq!(!crc32fast::hash(a.as_bytes()), 0x118cb727);
    assert_eq!(!crc32fast::hash(b.as_bytes()), 0x118cb727);
    for unknown in [0, u32::MAX] {
        let bytes = collision_index(&[(a, 16), (b, 34)], unknown);
        let parsed = Index::parse(&bytes, IndexKind::Index2).unwrap();
        assert_eq!(
            parsed.find_path(a).unwrap(),
            Some(Location {
                dat: 0,
                offset: 128
            })
        );
        assert_eq!(
            parsed.find_path(&b.to_ascii_uppercase()).unwrap(),
            Some(Location {
                dat: 1,
                offset: 256
            })
        );
        assert_eq!(parsed.find_path("exd/absent.exh").unwrap(), None);
        let rows: Vec<_> = parsed.collision_records().unwrap().collect();
        assert_eq!(rows[0].unknown(), 0x12345678);
        assert_eq!(rows[1].sequence(), 1);
        assert_eq!(rows[0].raw()[255], 0xa5);
        assert!(rows[2].is_sentinel());
        assert_eq!(rows[2].unknown(), unknown);
        assert_eq!(rows[2].raw()[255], 0x5a);
        assert_eq!(parsed.byte_len(), 8 + 768);
        assert_eq!(
            Index::parse_with_collision_budget(&bytes, IndexKind::Index2, 767)
                .unwrap_err()
                .reason,
            "collision table budget exceeded"
        );
        let table = Index::from_table(&bytes[0x800..0x808], IndexKind::Index2).unwrap();
        assert_eq!(
            table.find_path(a).unwrap_err().reason,
            "collision name table unavailable"
        );
    }
    let bytes = collision_index(&[(a, 16), (a, 32)], 0);
    assert_eq!(
        Index::parse(&bytes, IndexKind::Index2)
            .unwrap()
            .find_path(a)
            .unwrap_err()
            .reason,
        "ambiguous duplicate collision path"
    );
    let bytes = collision_index(&[(b, 32)], 0);
    assert_eq!(
        Index::parse(&bytes, IndexKind::Index2)
            .unwrap()
            .find_path(a)
            .unwrap(),
        None
    );
    let mut bytes = collision_index(&[(a, 16)], 0);
    let duplicate = bytes[0x800..0x808].to_vec();
    bytes.splice(0x808..0x808, duplicate);
    put32(&mut bytes, 0x40c, 16);
    put32(&mut bytes, 0x454, 0x810);
    assert_eq!(
        Index::parse(&bytes, IndexKind::Index2)
            .unwrap()
            .find_path(a)
            .unwrap_err()
            .reason,
        "ambiguous duplicate hash; collision resolution not supported"
    );
}

#[test]
fn index2_auxiliary_spans_and_records_are_bounded() {
    let original = collision_index(&[(PATH, 16)], 0);
    for (at, value, reason) in [
        (0x454, 0x400, "collision table overlaps headers"),
        (0x454, 0x800, "collision table overlaps main table"),
        (0x454, u32::MAX, "file range out of bounds"),
        (0x458, 513, "partial collision record"),
        (0x808, 0, "collision path hash mismatch"),
        (0x810, 1, "flagged collision location"),
    ] {
        let mut bytes = original.clone();
        put32(&mut bytes, at, value);
        assert_eq!(
            Index::parse(&bytes, IndexKind::Index2).unwrap_err().reason,
            reason
        );
    }
    let mut bytes = original.clone();
    bytes[0x818..0x908].fill(b'x');
    assert_eq!(
        Index::parse(&bytes, IndexKind::Index2).unwrap_err().reason,
        "unterminated collision path"
    );
    let mut bytes = original.clone();
    let sentinel = bytes[0x908..0xa08].to_vec();
    bytes[0x808..0x908].copy_from_slice(&sentinel);
    assert_eq!(
        Index::parse(&bytes, IndexKind::Index2).unwrap_err().reason,
        "collision sentinel is not final"
    );
    for length in 0..original.len() {
        assert!(Index::parse(&original[..length], IndexKind::Index2).is_err());
    }
}

#[test]
fn index2_only_backend_reads_collision_and_propagates_auxiliary_io_errors() {
    let mut memory = Memory::default();
    let bytes = collision_index(&[(PATH, 0x100)], u32::MAX);
    memory.insert(format!("{STEM}.index2"), bytes);
    memory.insert(
        format!("{STEM}.dat0"),
        dat(&[(b"collision payload", false)]),
    );
    let opens = memory.opens.clone();
    let fail_at = memory.fail_read_at.clone();
    let pack = SqPack::new(memory);
    assert_eq!(block_on(pack.read(PATH)).unwrap(), b"collision payload");
    assert!(opens.borrow().iter().any(|s| s.ends_with(".index2")));
    *fail_at.borrow_mut() = Some(0x808);
    assert_eq!(
        block_on(pack.read(PATH)).unwrap_err(),
        Error::Source("injected read failure")
    );
}

#[test]
fn index2_auxiliary_read_budget_is_checked_before_io() {
    let mut memory = Memory::default();
    memory.insert(
        format!("{STEM}.index2"),
        collision_index(&[(PATH, 0x100)], 0),
    );
    let reads = memory.reads.clone();
    let pack = SqPack::new(memory).with_max_read_bytes(Some(511));
    assert_eq!(
        block_on(pack.read(PATH)).unwrap_err(),
        Error::ReadTooLarge {
            requested: 512,
            limit: 511
        }
    );
    assert!(!reads.borrow().iter().any(|(_, offset, _)| *offset == 0x808));
}

#[test]
fn index2_cache_budget_includes_collision_bytes() {
    for (budget, expected_reads) in [(8, 2), (520, 1)] {
        let mut memory = Memory::default();
        memory.insert(
            format!("{STEM}.index2"),
            collision_index(&[(PATH, 0x100)], 0),
        );
        memory.insert(format!("{STEM}.dat0"), dat(&[(b"cached", false)]));
        let reads = memory.reads.clone();
        let pack = SqPack::new(memory).with_index_cache(Some(budget));
        assert_eq!(block_on(pack.read(PATH)).unwrap(), b"cached");
        assert_eq!(block_on(pack.read(PATH)).unwrap(), b"cached");
        assert_eq!(
            reads
                .borrow()
                .iter()
                .filter(|(name, at, _)| name.ends_with(".index2") && *at == 0x808)
                .count(),
            expected_reads
        );
    }
}

fn dat(blocks: &[(&[u8], bool)]) -> Vec<u8> {
    let mut bytes = container(1);
    bytes.resize(0x880, 0);
    put32(&mut bytes, 0x800, 128);
    put32(&mut bytes, 0x804, 2);
    put32(
        &mut bytes,
        0x808,
        blocks.iter().map(|(b, _)| b.len() as u32).sum(),
    );
    put32(&mut bytes, 0x814, blocks.len() as u32);
    for (i, &(payload, compressed)) in blocks.iter().enumerate() {
        let encoded = if compressed {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(payload).unwrap();
            encoder.finish().unwrap()
        } else {
            payload.to_vec()
        };
        let start = bytes.len();
        let stored_size = (16 + encoded.len()).div_ceil(128) * 128;
        bytes.resize(start + stored_size, 0);
        put32(&mut bytes, 0x818 + i * 8, (start - 0x880) as u32);
        put16(&mut bytes, 0x81c + i * 8, stored_size as u16);
        put16(&mut bytes, 0x81e + i * 8, payload.len() as u16);
        put32(&mut bytes, start, 16);
        put32(
            &mut bytes,
            start + 8,
            if compressed {
                encoded.len() as u32
            } else {
                32000
            },
        );
        put32(&mut bytes, start + 12, payload.len() as u32);
        bytes[start + 16..start + 16 + encoded.len()].copy_from_slice(&encoded);
    }
    bytes
}

fn fixture(stem: &str, index2: bool, dat_id: u8, blocks: &[(&[u8], bool)]) -> Memory {
    let mut memory = Memory::default();
    let packed = (0x800 / 8) | (u32::from(dat_id) << 1);
    memory.insert(
        format!("{stem}.{}", if index2 { "index2" } else { "index" }),
        index(index2, &[(if index2 { HASH2 } else { HASH }, packed)]),
    );
    memory.insert(format!("{stem}.dat{dat_id}"), dat(blocks));
    memory
}

fn model_fixture() -> (Memory, Vec<Vec<u8>>) {
    let payloads: Vec<Vec<u8>> = (0..11).map(|i| vec![i as u8 + 1; 17 + i * 3]).collect();
    let mut data = container(1);
    data.resize(0x900, 0);
    put32(&mut data, 0x800, 256);
    put32(&mut data, 0x804, 3);
    put32(
        &mut data,
        0x808,
        68 + payloads.iter().map(|v| v.len() as u32).sum::<u32>(),
    );
    put32(&mut data, 0x814, 0x0100_0006);
    put16(&mut data, 0x8c8, 3);
    put16(&mut data, 0x8ca, 2);
    data[0x8cc..0x8cf].copy_from_slice(&[3, 1, 1]);
    // Physical order intentionally differs from logical segment order.
    for (i, payload) in payloads.iter().enumerate() {
        let compressed = i % 2 != 0;
        let encoded = if compressed {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(payload).unwrap();
            encoder.finish().unwrap()
        } else {
            payload.clone()
        };
        let start = data.len();
        let span = (encoded.len() + 16).div_ceil(128) * 128;
        put32(&mut data, 0x818 + i * 4, 128);
        put32(&mut data, 0x844 + i * 4, span as u32);
        put32(&mut data, 0x870 + i * 4, (start - 0x900) as u32);
        put16(&mut data, 0x89c + i * 2, i as u16);
        put16(&mut data, 0x8b2 + i * 2, 1);
        put16(&mut data, 0x8d0 + i * 2, span as u16);
        data.resize(start + span, 0);
        put32(&mut data, start, 16);
        put32(
            &mut data,
            start + 8,
            if compressed {
                encoded.len() as u32
            } else {
                32000
            },
        );
        put32(&mut data, start + 12, payload.len() as u32);
        data[start + 16..start + 16 + encoded.len()].copy_from_slice(&encoded);
    }
    let mut memory = fixture(STEM, false, 0, &[]);
    memory.insert(format!("{STEM}.dat0"), data);
    (memory, payloads)
}

#[test]
fn model_reconstruction_keeps_all_segments_and_writes_exact_logical_offsets() {
    let (memory, payloads) = model_fixture();
    let output = block_on(SqPack::new(memory).read(PATH)).unwrap();
    assert_eq!(&output[..4], &0x0100_0006u32.to_le_bytes());
    assert_eq!(&output[4..8], &(payloads[0].len() as u32).to_le_bytes());
    assert_eq!(&output[8..12], &(payloads[1].len() as u32).to_le_bytes());
    assert_eq!(&output[12..16], &[3, 0, 2, 0]);
    assert_eq!(&output[64..68], &[3, 1, 1, 0]);
    let mut cursor = 68;
    for i in [0, 1, 2, 5, 8, 3, 6, 9, 4, 7, 10] {
        assert_eq!(&output[cursor..cursor + payloads[i].len()], &payloads[i]);
        if (2..5).contains(&i) || i >= 8 {
            let field = if i < 5 {
                16 + (i - 2) * 4
            } else {
                28 + (i - 8) * 4
            };
            assert_eq!(&output[field..field + 4], &(cursor as u32).to_le_bytes());
        }
        cursor += payloads[i].len();
    }
    assert_eq!(output.len(), cursor);
}

#[test]
fn explicit_entry_locations_preserve_extraction_and_error_checks() {
    let memory = fixture(STEM, false, 0, &[(b"located", true)]);
    let sqpack = SqPack::new(memory);
    assert_eq!(
        block_on(sqpack.read_entry(&format!("{STEM}.dat0"), 0x800)).unwrap(),
        b"located"
    );
    assert!(block_on(sqpack.read_entry(&format!("{STEM}.dat0"), u64::MAX)).is_err());
    assert!(block_on(sqpack.read_entry(&format!("{STEM}.dat0"), 0x400)).is_err());
}

#[test]
fn model_rejects_overlaps_bad_spans_bad_sizes_and_truncated_metadata() {
    for (offset, value, width) in [
        (0x800, 128, 4),   // specialized metadata does not fit
        (0x808, 69, 4),    // output budget smaller than decoded data
        (0x818, 256, 4),   // wrong rounded segment capacity
        (0x844, 256, 4),   // stored size disagrees with spans
        (0x874, 0, 4),     // overlapping physical segments
        (0x89e, 0, 2),     // duplicate block indices
        (0x89c, 65535, 2), // span table range overflow
        (0x8cc, 4, 1),     // impossible LOD count
        (0x8d0, 1, 2),     // block span shorter than prefix
        (0x90c, 65536, 4), // invalid individual block expansion
    ] {
        let (mut memory, _) = model_fixture();
        let data = memory.mutate(&format!("{STEM}.dat0"));
        match width {
            4 => put32(data, offset, value),
            2 => put16(data, offset, value as u16),
            _ => data[offset] = value as u8,
        }
        assert!(
            block_on(SqPack::new(memory).read(PATH)).is_err(),
            "offset {offset:x}"
        );
    }
    let (memory, _) = model_fixture();
    assert!(matches!(
        block_on(
            SqPack::new(memory)
                .with_max_read_bytes(Some(100))
                .read(PATH)
        ),
        Err(Error::ReadTooLarge { .. })
    ));
}

#[test]
fn indexes_enumerate_in_order_including_duplicates_and_synonyms() {
    for kind in [IndexKind::Index, IndexKind::Index2] {
        let index2 = kind == IndexKind::Index2;
        let hash = if index2 { HASH2 } else { HASH };
        let packed = 0x100 | (7 << 1);
        let bytes = index(index2, &[(hash, packed), (hash, packed), (42, 1)]);
        let parsed = Index::parse(&bytes, kind).unwrap();
        assert_eq!(parsed.kind(), kind);
        assert_eq!(parsed.len(), 3);
        assert!(!parsed.is_empty());
        let entries: Vec<_> = parsed.entries().collect();
        assert_eq!(entries[0], entries[1]);
        assert_eq!(entries[0].packed, packed);
        assert_eq!(
            entries[0].location(),
            Some(Location {
                dat: 7,
                offset: 0x800
            })
        );
        assert!(!entries[0].is_synonym());
        assert_eq!(
            entries[0].hash,
            if index2 {
                IndexHash::Index2(HASH2 as u32)
            } else {
                IndexHash::Index {
                    directory: (HASH >> 32) as u32,
                    filename: HASH as u32,
                }
            }
        );
        assert!(entries[2].is_synonym());
        assert_eq!(entries[2].location(), None);
        assert_eq!(parsed.entries().next_back(), Some(entries[2]));
        assert_eq!(parsed.entries().len(), 3);
        let owned = Index::from_table(bytes[0x800..].to_vec(), kind).unwrap();
        assert_eq!(owned.entries().collect::<Vec<_>>(), entries);
        assert!(Index::parse(&index(index2, &[]), kind).unwrap().is_empty());
    }
}

#[test]
fn pure_index_parser_rejects_truncation_invalid_headers_and_table_spans() {
    for kind in [IndexKind::Index, IndexKind::Index2] {
        let index2 = kind == IndexKind::Index2;
        let bytes = index(index2, &[(42, 0x100)]);
        for end in 0..bytes.len() {
            assert!(Index::parse(&bytes[..end], kind).is_err(), "prefix {end}");
        }
        for (offset, value) in [
            (0, 0),
            (8, 1),
            (12, 23),
            (16, 2),
            (20, 1),
            (0x400, 15),
            (0x404, 2),
            (0x408, 0x400),
            (0x408, u32::MAX),
            (0x40c, 1),
            (0x40c, u32::MAX),
        ] {
            let mut bad = bytes.clone();
            put32(&mut bad, offset, value);
            assert!(
                Index::parse(&bad, kind).is_err(),
                "offset {offset}, value {value}"
            );
        }
        assert!(Index::from_table(&[0u8; 1][..], kind).is_err());
    }
}

#[test]
fn loading_an_index_never_opens_dat_and_preserves_backend_errors() {
    for kind in [IndexKind::Index, IndexKind::Index2] {
        let name = format!(
            "{STEM}.{}",
            if kind == IndexKind::Index {
                "index"
            } else {
                "index2"
            }
        );
        let mut memory = Memory::default();
        memory.insert(name.clone(), index(kind == IndexKind::Index2, &[(42, 1)]));
        let opens = memory.opens.clone();
        let sqpack = SqPack::new(memory);
        let parsed = block_on(sqpack.read_index(&name, kind)).unwrap();
        assert!(parsed.entries().next().unwrap().is_synonym());
        assert_eq!(&*opens.borrow(), &[name]);
    }
    let mut memory = Memory::default();
    memory
        .fail
        .insert("error.index".to_owned(), "permission denied");
    let sqpack = SqPack::new(memory);
    assert!(matches!(
        block_on(sqpack.read_index("missing.index", IndexKind::Index)),
        Err(Error::MissingFile(_))
    ));
    assert!(matches!(
        block_on(sqpack.read_index("error.index", IndexKind::Index)),
        Err(Error::Source("permission denied"))
    ));
}

#[test]
fn index_loading_checks_the_read_budget_before_reading_the_table() {
    let mut memory = Memory::default();
    memory.insert("test.index".to_owned(), index(false, &[(1, 0), (2, 0)]));
    let reads = memory.reads.clone();
    let sqpack = SqPack::new(memory).with_max_read_bytes(Some(24));
    assert!(matches!(
        block_on(sqpack.read_index("test.index", IndexKind::Index)),
        Err(Error::ReadTooLarge {
            requested: 32,
            limit: 24
        })
    ));
    assert!(reads.borrow().iter().all(|(_, offset, _)| *offset < 0x800));
}

#[test]
fn resource_path_extracts_multiple_raw_and_deflated_blocks_from_index() {
    let memory = fixture(
        STEM,
        false,
        3,
        &[(b"EXHF", false), (b"\0synthetic", true), (b" table", false)],
    );
    let opens = memory.opens.clone();
    let sqpack = SqPack::new(memory);
    assert_eq!(
        block_on(sqpack.read("EXD/Item.Exh")).unwrap(),
        b"EXHF\0synthetic table"
    );
    assert_eq!(
        *opens.borrow(),
        [format!("{STEM}.index"), format!("{STEM}.dat3")]
    );
}

#[test]
fn index2_is_used_when_index_is_confirmed_absent() {
    let memory = fixture(STEM, true, 0, &[(b"EXHF\0synthetic item table", true)]);
    let opens = memory.opens.clone();
    assert_eq!(
        block_on(SqPack::new(memory).read(PATH)).unwrap(),
        b"EXHF\0synthetic item table"
    );
    assert_eq!(
        *opens.borrow(),
        [
            format!("{STEM}.index"),
            format!("{STEM}.index2"),
            format!("{STEM}.dat0")
        ]
    );
}

#[test]
fn later_chunks_are_found_without_listing_or_registration() {
    let memory = fixture("ffxiv/0a0002.win32", false, 0, &[(b"later chunk", true)]);
    let opens = memory.opens.clone();
    assert_eq!(
        block_on(SqPack::new(memory).read(PATH)).unwrap(),
        b"later chunk"
    );
    assert!(
        opens
            .borrow()
            .contains(&"ffxiv/0a0001.win32.index2".to_owned())
    );
}

#[test]
fn missing_resource_requires_confirmed_absence_not_a_transport_failure() {
    let memory = Memory::default();
    let opens = memory.opens.clone();
    assert!(
        matches!(block_on(SqPack::new(memory).read(PATH)), Err(Error::MissingResource(path)) if path == PATH)
    );
    assert_eq!(opens.borrow().len(), 512);
    let mut memory = Memory::default();
    memory
        .fail
        .insert(format!("{STEM}.index"), "permission or network failure");
    let opens = memory.opens.clone();
    assert_eq!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::Source("permission or network failure"))
    );
    assert_eq!(opens.borrow().len(), 1);
}

#[test]
fn an_index_without_the_hash_is_a_lookup_miss_not_an_empty_file() {
    let mut memory = Memory::default();
    memory.insert(format!("{STEM}.index"), index(false, &[(HASH ^ 1, 0x100)]));
    let reads = memory.reads.clone();
    assert!(matches!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::MissingResource(_))
    ));
    assert!(
        reads
            .borrow()
            .iter()
            .all(|(name, _, _)| !name.contains(".dat"))
    );
}

#[test]
fn a_present_primary_index_is_preferred_over_index2() {
    let mut memory = fixture(STEM, false, 0, &[(b"primary index", false)]);
    memory.insert(format!("{STEM}.index2"), vec![0; 24]);
    let opens = memory.opens.clone();
    assert_eq!(
        block_on(SqPack::new(memory).read(PATH)).unwrap(),
        b"primary index"
    );
    assert!(!opens.borrow().iter().any(|name| name.ends_with(".index2")));
}

#[test]
fn missing_dat_is_not_misreported_as_missing_resource() {
    let mut memory = fixture(STEM, false, 0, &[(b"data", false)]);
    memory.files.remove(&format!("{STEM}.dat0"));
    assert!(
        matches!(block_on(SqPack::new(memory).read(PATH)), Err(Error::MissingFile(file)) if file == format!("{STEM}.dat0"))
    );
}

#[test]
fn malformed_index_does_not_fall_back_to_another_index() {
    let mut memory = fixture(STEM, true, 0, &[(b"data", false)]);
    memory.insert(format!("{STEM}.index"), vec![0; 24]);
    let opens = memory.opens.clone();
    assert!(matches!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::InvalidData { .. })
    ));
    assert_eq!(opens.borrow().len(), 1);
}

#[test]
fn index_ranges_record_widths_and_platform_are_validated() {
    for (offset, value) in [
        (0x408, u32::MAX),
        (0x40c, 15),
        (0x400, u32::MAX),
        (12, 0),
        (16, 99),
    ] {
        let mut memory = fixture(STEM, false, 0, &[(b"data", false)]);
        put32(memory.mutate(&format!("{STEM}.index")), offset, value);
        assert!(
            matches!(
                block_on(SqPack::new(memory).read(PATH)),
                Err(Error::InvalidData { .. })
            ),
            "offset {offset}"
        );
    }
    let mut memory = fixture(STEM, false, 0, &[(b"data", false)]);
    memory.mutate(&format!("{STEM}.index"))[8] = 1;
    assert!(matches!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::InvalidData { .. })
    ));
}

#[test]
fn ambiguous_hashes_and_synonyms_do_not_return_guessed_data() {
    for index2 in [false, true] {
        let key = if index2 { HASH2 } else { HASH };
        for records in [vec![(key, 0x100), (key, 0x100)], vec![(key, 0x101)]] {
            let mut memory = Memory::default();
            memory.insert(
                format!("{STEM}.{}", if index2 { "index2" } else { "index" }),
                index(index2, &records),
            );
            assert!(matches!(
                block_on(SqPack::new(memory).read(PATH)),
                Err(Error::UnsupportedIndexEntry { .. })
            ));
        }
    }
}

#[test]
fn specialized_entries_are_explicitly_unsupported_not_missing() {
    for kind in [1, 99] {
        let mut memory = fixture(STEM, false, 0, &[(b"data", false)]);
        put32(memory.mutate(&format!("{STEM}.dat0")), 0x804, kind);
        assert!(
            matches!(block_on(SqPack::new(memory).read(PATH)), Err(Error::UnsupportedContentType { kind: actual, .. }) if actual == kind)
        );
    }
}

#[test]
fn output_cap_is_checked_before_block_reads_or_output_allocation() {
    let payload = vec![b'x'; 2048];
    let memory = fixture(STEM, false, 0, &[(&payload, true)]);
    let reads = memory.reads.clone();
    let sqpack = SqPack::new(memory).with_max_read_bytes(Some(1024));
    assert_eq!(
        block_on(sqpack.read(PATH)),
        Err(Error::ReadTooLarge {
            requested: 2048,
            limit: 1024
        })
    );
    assert!(
        !reads
            .borrow()
            .iter()
            .any(|(name, offset, _)| name.ends_with(".dat0") && *offset >= 0x818)
    );
    let memory = fixture(STEM, false, 0, &[(&payload, true)]);
    assert_eq!(block_on(SqPack::new(memory).read(PATH)).unwrap(), payload);
}

#[test]
fn corrupt_sizes_offsets_and_deflate_are_rejected() {
    for (offset, value) in [
        (0x800, 24),
        (0x808, u32::MAX),
        (0x814, u32::MAX),
        (0x818, u32::MAX),
        (0x880, 0),
        (0x888, u32::MAX),
        (0x88c, u32::MAX),
    ] {
        let mut memory = fixture(STEM, false, 0, &[(b"hello SqPack", true)]);
        put32(memory.mutate(&format!("{STEM}.dat0")), offset, value);
        assert!(
            matches!(
                block_on(SqPack::new(memory).read(PATH)),
                Err(Error::InvalidData { .. })
            ),
            "offset {offset}"
        );
    }
    let mut memory = fixture(STEM, false, 0, &[(b"hello SqPack", true)]);
    memory.mutate(&format!("{STEM}.dat0"))[0x890] = 0xff;
    assert!(matches!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::InvalidData { .. })
    ));
}

#[test]
fn a_block_cannot_expand_beyond_its_claimed_length() {
    let payload = vec![b'x'; 4096];
    let mut memory = fixture(STEM, false, 0, &[(&payload, true)]);
    let bytes = memory.mutate(&format!("{STEM}.dat0"));
    // Make all advertised sizes agree on one byte, while the DEFLATE stream
    // actually produces 4096. The decoder may allocate only two output bytes.
    put32(bytes, 0x808, 1);
    put16(bytes, 0x81e, 1);
    put32(bytes, 0x88c, 1);
    assert!(matches!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::InvalidData { .. })
    ));
}

#[test]
fn empty_binary_entry_and_invalid_paths_have_distinct_results() {
    assert_eq!(
        block_on(SqPack::new(fixture(STEM, false, 0, &[])).read(PATH)).unwrap(),
        b""
    );
    let memory = Memory::default();
    let opens = memory.opens.clone();
    let sqpack = SqPack::new(memory);
    for path in ["../exd/item.exh", "item.exh", "unknown/item.exh"] {
        assert!(matches!(
            block_on(sqpack.read(path)),
            Err(Error::InvalidResourcePath(_))
        ));
    }
    assert!(opens.borrow().is_empty());
}

fn deflate(payload: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(payload).unwrap();
    encoder.finish().unwrap()
}

/// Synthetic type-4 entry: 24-byte header, level table, span table, body.
fn texture_dat(header: &[u8], blocks: &[(&[u8], bool)]) -> Vec<u8> {
    let encoded: Vec<Vec<u8>> = blocks
        .iter()
        .map(|&(payload, compressed)| {
            if compressed {
                deflate(payload)
            } else {
                payload.to_vec()
            }
        })
        .collect();
    let spans: Vec<usize> = encoded
        .iter()
        .map(|bytes| (16 + bytes.len()).div_ceil(16) * 16)
        .collect();
    let mut bytes = container(1);
    bytes.resize(0x880, 0);
    let output = header.len() + blocks.iter().map(|(b, _)| b.len()).sum::<usize>();
    put32(&mut bytes, 0x800, 128);
    put32(&mut bytes, 0x804, 4);
    put32(&mut bytes, 0x808, output as u32);
    put32(&mut bytes, 0x814, 1);
    put32(&mut bytes, 0x818, header.len() as u32);
    put32(&mut bytes, 0x81c, spans.iter().sum::<usize>() as u32);
    put32(
        &mut bytes,
        0x820,
        blocks.iter().map(|(b, _)| b.len() as u32).sum::<u32>(),
    );
    put32(&mut bytes, 0x828, blocks.len() as u32);
    bytes.extend_from_slice(header);
    for (i, (&(payload, compressed), span)) in blocks.iter().zip(&spans).enumerate() {
        let start = bytes.len();
        bytes.resize(start + span, 0);
        put32(&mut bytes, start, 16);
        put32(
            &mut bytes,
            start + 8,
            if compressed {
                encoded[i].len() as u32
            } else {
                32000
            },
        );
        put32(&mut bytes, start + 12, payload.len() as u32);
        bytes[start + 16..start + 16 + encoded[i].len()].copy_from_slice(&encoded[i]);
        put16(&mut bytes, 0x82c + i * 2, *span as u16);
    }
    bytes
}

fn texture_fixture(header: &[u8], blocks: &[(&[u8], bool)]) -> Memory {
    let mut memory = Memory::default();
    memory.insert(format!("{STEM}.index"), index(false, &[(HASH, 0x800 / 8)]));
    memory.insert(format!("{STEM}.dat0"), texture_dat(header, blocks));
    memory
}

fn selective_volume_fixture(short: bool, compressed: bool) -> Memory {
    let mut header = [0; 80];
    put32(&mut header, 0, 0x0100_0000);
    put32(&mut header, 4, 0x1450);
    put16(&mut header, 8, 2);
    put16(&mut header, 10, 2);
    put16(&mut header, 12, 4);
    header[14] = 3;
    for (i, offset) in [80, 144, 152].into_iter().enumerate() {
        put32(&mut header, 28 + i * 4, offset);
    }
    let mut dat = container(1);
    dat.resize(0x880, 0);
    put32(&mut dat, 0x800, 128);
    put32(&mut dat, 0x804, 4);
    put32(&mut dat, 0x808, 156);
    put32(&mut dat, 0x814, 3);
    dat.extend(header);
    for (i, size) in [64, if short { 4 } else { 8 }, 4].into_iter().enumerate() {
        let payload = [1, 2, 3, 4].repeat(size / 4);
        let encoded = if compressed {
            deflate(&payload)
        } else {
            payload
        };
        let span = (16 + encoded.len()).div_ceil(16) * 16;
        let start = dat.len();
        let row = 0x818 + i * 20;
        put32(&mut dat, row, (start - 0x880) as u32);
        put32(&mut dat, row + 4, span as u32);
        put32(&mut dat, row + 8, size as u32);
        put32(&mut dat, row + 12, i as u32);
        put32(&mut dat, row + 16, 1);
        put16(&mut dat, 0x854 + i * 2, span as u16);
        dat.resize(start + span, 0);
        put32(&mut dat, start, 16);
        put32(
            &mut dat,
            start + 8,
            if compressed {
                encoded.len() as u32
            } else {
                32000
            },
        );
        put32(&mut dat, start + 12, size as u32);
        dat[start + 16..start + 16 + encoded.len()].copy_from_slice(&encoded);
    }
    let mut memory = Memory::default();
    memory.insert(format!("{STEM}.index"), index(false, &[(HASH, 0x800 / 8)]));
    memory.insert(format!("{STEM}.dat0"), dat);
    memory
}

#[test]
fn complete_selected_volume_mips_decode_without_fabricating_the_full_file() {
    for compressed in [false, true] {
        let memory = selective_volume_fixture(true, compressed);
        let reads = memory.reads.clone();
        let dat = &memory.files[&format!("{STEM}.dat0")];
        let last_selected = 0x880
            + u64::from(u32::from_le_bytes(dat[0x818..0x81c].try_into().unwrap()))
            + u64::from(u32::from_le_bytes(dat[0x81c..0x820].try_into().unwrap()));
        let sqpack = SqPack::new(memory).with_max_read_bytes(Some(144));
        let group = block_on(sqpack.read_texture_group(PATH, 0)).unwrap();
        assert_eq!(group.index(), 0);
        assert_eq!(group.group_count(), 3);
        assert_eq!(group.declared_file_size(), 156);
        assert_eq!(group.prefix()[14], 3);
        let mip = group.volume_mip().unwrap();
        assert_eq!((mip.width(), mip.height(), mip.depth()), (2, 2, 4));
        assert_eq!((mip.source_index(), mip.source_mip_count()), (0, 3));
        assert_eq!(mip.decode_rgba8(64).unwrap(), [3, 2, 1, 4].repeat(16));
        assert!(mip.decode_rgba8(63).is_err());
        // Prefix/table reads and the selected blocks are the complete I/O scope.
        assert!(
            reads
                .borrow()
                .iter()
                .filter(|(name, _, _)| name.ends_with(".dat0"))
                .all(|(_, offset, length)| offset + *length as u64 <= last_selected)
        );
        assert!(matches!(
            block_on(sqpack.read(PATH)),
            Err(Error::ReadTooLarge {
                requested: 156,
                limit: 144
            })
        ));
        let short = block_on(sqpack.read_texture_group(PATH, 1)).unwrap();
        assert_eq!(
            short.volume_mip().unwrap_err(),
            icexiv_data::tex::Error::MipSizeMismatch {
                index: 1,
                expected: 8,
                actual: 4
            }
        );
        let tail = block_on(sqpack.read_texture_group(PATH, 2)).unwrap();
        assert_eq!(
            tail.volume_mip().unwrap().decode_rgba8(4).unwrap(),
            [3, 2, 1, 4]
        );
        assert!(matches!(
            block_on(sqpack.read_texture_group(PATH, 3)),
            Err(Error::TextureGroupOutOfBounds { index: 3, count: 3 })
        ));
    }
    let sqpack = SqPack::new(selective_volume_fixture(true, false));
    assert!(matches!(
        block_on(sqpack.read(PATH)),
        Err(Error::InvalidData {
            reason: "decoded size disagrees with entry size",
            ..
        })
    ));
    let sqpack = SqPack::new(selective_volume_fixture(true, false)).with_max_read_bytes(Some(143));
    assert!(matches!(
        block_on(sqpack.read_texture_group_entry(&format!("{STEM}.dat0"), 0x800, 0)),
        Err(Error::ReadTooLarge {
            requested: 144,
            limit: 143
        })
    ));
}

#[test]
fn selected_group_checks_all_metadata_but_only_its_own_streams() {
    let mut memory = selective_volume_fixture(false, true);
    // Damage the second group's DEFLATE stream, leaving all table spans intact.
    let bytes = memory.mutate(&format!("{STEM}.dat0"));
    let second = 0x880 + u32::from_le_bytes(bytes[0x82c..0x830].try_into().unwrap()) as usize;
    bytes[second + 16] = 0xff;
    let sqpack = SqPack::new(memory);
    assert!(
        block_on(sqpack.read_texture_group(PATH, 0))
            .unwrap()
            .volume_mip()
            .is_ok()
    );
    assert!(matches!(
        block_on(sqpack.read_texture_group(PATH, 1)),
        Err(Error::InvalidData { .. })
    ));
    for (offset, value) in [
        (0x82c, 80),
        (0x838, 0),
        (0x830, 1),
        (0x834, u32::MAX),
        (0x83c, 0),
    ] {
        let mut memory = selective_volume_fixture(false, true);
        put32(memory.mutate(&format!("{STEM}.dat0")), offset, value);
        assert!(matches!(
            block_on(SqPack::new(memory).read_texture_group(PATH, 0)),
            Err(Error::InvalidData { .. })
        ));
    }
    let mut memory = selective_volume_fixture(false, true);
    memory.mutate(&format!("{STEM}.dat0")).pop();
    assert!(matches!(
        block_on(SqPack::new(memory).read_texture_group(PATH, 0)),
        Err(Error::InvalidData { .. })
    ));
    for offset in [0x818, 0x854, 0x880, 0x8d0] {
        let memory = selective_volume_fixture(false, true);
        *memory.fail_read_at.borrow_mut() = Some(offset);
        assert!(matches!(
            block_on(SqPack::new(memory).read_texture_group(PATH, 0)),
            Err(Error::Source("injected read failure"))
        ));
    }
}

#[test]
fn selected_group_can_contain_multiple_blocks_without_claiming_a_mip() {
    let memory = texture_fixture(&[0x42; 80], &[(&[7; 64], true), (&[9; 32], false)]);
    let group = block_on(SqPack::new(memory).read_texture_group(PATH, 0)).unwrap();
    assert_eq!(group.prefix(), &[0x42; 80]);
    assert_eq!(group.data(), [&[7; 64][..], &[9; 32]].concat());
    assert!(group.volume_mip().is_err());
}

#[test]
fn selected_group_with_unspecified_length_respects_other_declared_groups() {
    let name = format!("{STEM}.dat0");
    for declared_size in [156, 155] {
        let mut memory = selective_volume_fixture(false, true);
        let bytes = memory.mutate(&name);
        put32(bytes, 0x820, 0); // First group's length is defined by its blocks.
        put32(bytes, 0x808, declared_size);
        let result = block_on(SqPack::new(memory).read_texture_group_entry(&name, 0x800, 0));
        if declared_size == 156 {
            assert_eq!(result.unwrap().data(), [1, 2, 3, 4].repeat(16));
        } else {
            assert!(matches!(
                result,
                Err(Error::InvalidData {
                    reason: "texture group block exceeds declared output",
                    ..
                })
            ));
        }
    }
}

#[test]
fn volume_binding_checks_original_header_and_logical_identity() {
    let name = format!("{STEM}.dat0");
    let mut memory = selective_volume_fixture(false, false);
    for offset in [28, 32, 36] {
        put32(memory.mutate(&name), 0x880 + offset, 80);
    }
    let sqpack = SqPack::new(memory);
    let group = block_on(sqpack.read_texture_group(PATH, 1)).unwrap();
    assert_eq!(group.prefix()[28..40], 80u32.to_le_bytes().repeat(3));
    let mip = group.volume_mip().unwrap();
    assert_eq!((mip.source_index(), mip.source_mip_count()), (1, 3));
    assert_eq!(mip.decode_rgba8(8).unwrap(), [3, 2, 1, 4].repeat(2));
    for (offset, value) in [
        (0x808, 157),         // Container logical length disagrees with TEX dimensions.
        (0x880 + 14, 2),      // Original mip count disagrees with group count.
        (0x880 + 36, 153),    // Gap between logical mip ranges.
        (0x880, 0x0080_0000), // Dimensions do not describe a volume.
    ] {
        let mut memory = selective_volume_fixture(false, false);
        put32(memory.mutate(&name), offset, value);
        let group = block_on(SqPack::new(memory).read_texture_group(PATH, 0)).unwrap();
        assert!(group.volume_mip().is_err());
    }
}

#[test]
fn texture_unaccounted_output_is_not_fabricated_as_zeroes() {
    let mut memory = texture_fixture(&[0; 80], &[(&[1; 16], false)]);
    let dat = memory.mutate(&format!("{STEM}.dat0"));
    put32(dat, 0x808, 112);
    assert!(matches!(
        block_on(SqPack::new(memory).read(PATH)),
        Err(Error::InvalidData {
            reason: "decoded size disagrees with entry size",
            ..
        })
    ));
}
#[test]
fn volume_group_size_gaps_remain_errors_with_valid_tex_headers_and_all_six_groups() {
    // Shapes and lengths independently observed in the two authorized samples;
    // payloads below are ours, not redistributed game data. A matching group
    // length is not evidence that the complete TEX mip slot has been supplied.
    for (depth, observed) in [
        (24u16, [24576usize, 1536, 64, 16, 8, 4]),
        (32u16, [32768usize, 2048, 128, 16, 8, 4]),
    ] {
        let mut tex = vec![0; 80];
        put32(&mut tex, 0, 0x0100_0000);
        put32(&mut tex, 4, 0x1450);
        put16(&mut tex, 8, 8);
        put16(&mut tex, 10, 32);
        put16(&mut tex, 12, depth);
        tex[14] = 6;
        let mut complete = [0usize; 6];
        let mut end = 80;
        for (mip, size) in complete.iter_mut().enumerate() {
            put32(&mut tex, 28 + mip * 4, end as u32);
            *size = (8usize >> mip).max(1)
                * (32usize >> mip).max(1)
                * (usize::from(depth) >> mip).max(1)
                * 4;
            end += *size;
        }
        assert_eq!(end, if depth == 24 { 28172 } else { 37540 });
        for sizes in [observed, complete] {
            let mut dat = container(1);
            dat.resize(0x900, 0);
            put32(&mut dat, 0x800, 256);
            put32(&mut dat, 0x804, 4);
            put32(&mut dat, 0x808, end as u32);
            put32(&mut dat, 0x814, 6);
            dat.extend_from_slice(&tex);
            for (mip, size) in sizes.into_iter().enumerate() {
                let start = dat.len();
                let span = (16 + size).div_ceil(16) * 16;
                let row = 0x818 + mip * 20;
                put32(&mut dat, row, (start - 0x900) as u32);
                put32(&mut dat, row + 4, span as u32);
                put32(&mut dat, row + 8, size as u32);
                put32(&mut dat, row + 12, mip as u32);
                put32(&mut dat, row + 16, 1);
                put16(&mut dat, 0x890 + mip * 2, span as u16);
                dat.resize(start + span, 0);
                put32(&mut dat, start, 16);
                put32(&mut dat, start + 8, 32000);
                put32(&mut dat, start + 12, size as u32);
                dat[start + 16..start + 16 + size].fill(0x35 + mip as u8);
            }
            let mut memory = Memory::default();
            memory.insert(format!("{STEM}.dat0"), dat);
            let result = block_on(SqPack::new(memory).read_entry(&format!("{STEM}.dat0"), 0x800));
            if sizes == observed {
                assert!(matches!(
                    result,
                    Err(Error::InvalidData {
                        reason: "decoded size disagrees with entry size",
                        ..
                    })
                ));
            } else {
                let bytes = result.unwrap();
                assert_eq!(bytes.len(), end);
                let texture = icexiv_data::tex::Tex::parse(&bytes).unwrap();
                assert_eq!(texture.depth(), depth);
                for (mip, size) in complete.into_iter().enumerate() {
                    assert_eq!(
                        texture.mip(mip).unwrap().data(),
                        vec![0x35 + mip as u8; size]
                    );
                }
            }
        }
    }
}
#[test]
fn texture_entries_reassemble_the_prefix_and_every_data_block() {
    let header = [0x42u8; 80];
    let first = vec![7u8; 64];
    let second = vec![9u8; 32];
    let memory = texture_fixture(&header, &[(&first, true), (&second, false)]);
    let bytes = block_on(SqPack::new(memory).read(PATH)).unwrap();
    assert_eq!(&bytes[..80], &header);
    assert_eq!(&bytes[80..], [first, second].concat());
}

#[test]
fn dat_segments_without_a_container_header_are_readable() {
    let header = [0x42u8; 80];
    let payload = vec![3u8; 48];
    let full = texture_dat(&header, &[(&payload, false)]);
    let mut memory = Memory::default();
    memory.insert(format!("{STEM}.index"), index(false, &[(HASH, 0)]));
    // Segment starting directly at the entry, as some installations split dats.
    memory.insert(format!("{STEM}.dat0"), full[0x800..].to_vec());
    let bytes = block_on(SqPack::new(memory).read(PATH)).unwrap();
    assert_eq!(&bytes[..80], &header);
    assert_eq!(&bytes[80..], payload);
}

#[test]
fn texture_groups_use_first_span_indices_not_mip_numbers() {
    for mutation in 0..4 {
        let mut memory = texture_fixture(&[0x42; 80], &[(&[7; 64], false), (&[9; 32], false)]);
        let bytes = memory.mutate(&format!("{STEM}.dat0"));
        let first_span = u16::from_le_bytes([bytes[0x82c], bytes[0x82d]]);
        let second_span = u16::from_le_bytes([bytes[0x82e], bytes[0x82f]]);
        put32(bytes, 0x814, 2);
        put32(bytes, 0x81c, u32::from(first_span));
        put32(bytes, 0x820, 64);
        put32(bytes, 0x828, 1);
        put32(bytes, 0x82c, 80 + u32::from(first_span));
        put32(bytes, 0x830, u32::from(second_span));
        put32(bytes, 0x834, 32);
        put32(bytes, 0x838, 1);
        put32(bytes, 0x83c, 1);
        put16(bytes, 0x840, first_span);
        put16(bytes, 0x842, second_span);
        match mutation {
            1 => put32(bytes, 0x838, 0),
            2 => put32(bytes, 0x830, 1),
            3 => put16(bytes, 0x842, 16),
            _ => (),
        }
        let result = block_on(SqPack::new(memory).read(PATH));
        if mutation == 0 {
            assert_eq!(
                result.unwrap(),
                [&[0x42; 80][..], &[7; 64], &[9; 32]].concat()
            );
        } else {
            assert!(matches!(result, Err(Error::InvalidData { .. })));
        }
    }
}

#[test]
fn texture_entries_reject_inconsistent_tables_and_spans() {
    let header = [0x42u8; 80];
    let first = vec![7u8; 64];
    let second = vec![9u8; 32];
    for (offset, value, wide) in [
        (0x814, 0, false),
        (0x814, u32::MAX, false),
        (0x808, 1, false),
        (0x820, 1, false),
        (0x81c, 1, false),
        (0x824, 1, false),
        (0x828, 0, false),
        (0x82c, 8, true),
    ] {
        let mut memory = texture_fixture(&header, &[(&first, true), (&second, true)]);
        let bytes = memory.mutate(&format!("{STEM}.dat0"));
        if wide {
            put16(bytes, offset, value as u16);
        } else {
            put32(bytes, offset, value);
        }
        assert!(
            matches!(
                block_on(SqPack::new(memory).read(PATH)),
                Err(Error::InvalidData { .. })
            ),
            "offset {offset:#x} value {value}"
        );
    }
}

#[test]
fn texture_rejects_cumulative_output_before_reading_the_overflowing_payload() {
    let header = [0x42; 80];
    let payload = [7; 64];
    let mut memory = texture_fixture(&header, &[(&payload, false), (&payload, false)]);
    put32(memory.mutate(&format!("{STEM}.dat0")), 0x808, 160);
    let reads = memory.reads.clone();
    assert!(matches!(
        block_on(
            SqPack::new(memory)
                .with_max_read_bytes(Some(160))
                .read(PATH)
        ),
        Err(Error::InvalidData {
            reason: "texture block exceeds remaining output size",
            ..
        })
    ));
    let second_block = 0x880 + 80 + 80;
    assert!(
        !reads
            .borrow()
            .iter()
            .any(|(name, offset, length)| name.ends_with(".dat0")
                && *offset == second_block
                && *length > 16)
    );
}

#[test]
fn texture_table_reads_preserve_source_errors() {
    for offset in [0x818, 0x82c] {
        let memory = texture_fixture(&[0; 80], &[(&[1; 16], false)]);
        *memory.fail_read_at.borrow_mut() = Some(offset);
        assert_eq!(
            block_on(SqPack::new(memory).read(PATH)),
            Err(Error::Source("injected read failure"))
        );
    }
}

#[test]
fn texture_span_table_preserves_read_budget_errors() {
    let memory = texture_fixture(&[], &[(&[1][..], false); 17]);
    assert_eq!(
        block_on(SqPack::new(memory).with_max_read_bytes(Some(32)).read(PATH)),
        Err(Error::ReadTooLarge {
            requested: 34,
            limit: 32
        })
    );
}

#[test]
fn an_enabled_index_cache_stops_rereading_tables() {
    let memory = fixture(STEM, false, 0, &[(b"cached", true)]);
    let reads = memory.reads.clone();
    let sqpack = SqPack::new(memory).with_index_cache(Some(4096));
    assert_eq!(block_on(sqpack.read(PATH)).unwrap(), b"cached");
    let after_first = reads.borrow().len();
    assert_eq!(block_on(sqpack.read(PATH)).unwrap(), b"cached");
    let second = reads.borrow().len() - after_first;
    assert!(
        second < after_first,
        "second read {second} vs first {after_first}"
    );
    assert!(
        !reads.borrow()[after_first..]
            .iter()
            .any(|(name, _, _)| name.ends_with(".index"))
    );
}

//! Newly authored synthetic containers, not extracted game files or copied tests.

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
}

struct MemoryFile {
    name: String,
    bytes: Rc<Vec<u8>>,
    reads: Reads,
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
        }))
    }
}

impl ReadAt for MemoryFile {
    type Error = &'static str;
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Self::Error> {
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
    for kind in [1, 3, 4, 99] {
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

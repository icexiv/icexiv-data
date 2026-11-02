//! Synthetic contract tests, not fixtures or assertions about the SqPack format.

use icexiv_data::sqpack::{Error, PackSource, ReadAt, SqPack};
use std::cell::Cell;
use std::future::{Future, poll_fn};
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

// Test-only executor: no runtime dependency, unsafe code, or production executor.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Permission,
    Transport,
    VersionChanged,
}

#[derive(Clone)]
struct MemoryFile {
    bytes: Rc<Vec<u8>>,
    size: u64,
    reads: Rc<Cell<usize>>,
    returned_length: Option<usize>,
    failure: Option<Failure>,
}

impl ReadAt for MemoryFile {
    type Error = Failure;

    fn len(&self) -> u64 {
        self.size
    }

    async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Failure> {
        // Keep an Rc across suspension to exercise genuinely non-Send futures.
        let bytes = Rc::clone(&self.bytes);
        let mut yielded = false;
        poll_fn(|cx| {
            if yielded {
                Poll::Ready(())
            } else {
                yielded = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
        .await;
        self.reads.set(self.reads.get() + 1);
        if let Some(error) = self.failure {
            return Err(error);
        }
        if let Some(length) = self.returned_length {
            return Ok(vec![0; length]);
        }
        let start = usize::try_from(offset).unwrap();
        Ok(bytes[start..start + length].to_vec())
    }
}

struct MemorySource {
    file: Option<MemoryFile>,
    opens: Rc<Cell<usize>>,
    failure: Option<Failure>,
}

impl PackSource for MemorySource {
    type Error = Failure;
    type File = MemoryFile;

    async fn open(&self, _: &str) -> Result<Option<Self::File>, Failure> {
        self.opens.set(self.opens.get() + 1);
        match self.failure {
            Some(error) => Err(error),
            None => Ok(self.file.clone()),
        }
    }
}

fn name() -> &'static str {
    "synthetic/container.dat"
}

fn source() -> MemorySource {
    MemorySource {
        file: Some(MemoryFile {
            bytes: Rc::new(vec![10, 20, 30, 40]),
            size: 4,
            reads: Rc::new(Cell::new(0)),
            returned_length: None,
            failure: None,
        }),
        opens: Rc::new(Cell::new(0)),
        failure: None,
    }
}

fn pack(source: MemorySource) -> SqPack<MemorySource> {
    SqPack::new(source).with_max_read_bytes(Some(4))
}

#[test]
fn logical_names_reject_unsafe_or_ambiguous_components() {
    let source = source();
    let opens = Rc::clone(&source.opens);
    let valid = "expansion-1/A_0.index2";
    let invalid_names = [
        "", "/a", "a/", "a//b", ".", "..", "a/../b", "a/./b", "a\\b", "C:/a", "a?b", "a%2fb",
        "a\nb", "中文",
    ];
    let sqpack = SqPack::new(source);
    for invalid in invalid_names {
        assert!(
            matches!(block_on(sqpack.open(invalid)), Err(Error::InvalidFileName(file)) if file == invalid)
        );
    }
    assert_eq!(opens.get(), 0);
    assert!(block_on(sqpack.open(valid)).is_ok());
    assert_eq!(opens.get(), 1);
}

#[test]
fn construction_does_no_io_and_any_valid_name_is_sent_to_the_source() {
    let source = source();
    let opens = Rc::clone(&source.opens);
    let sqpack = SqPack::new(source);
    assert_eq!(opens.get(), 0);
    // There is no registration or discovery step before opening a valid name.
    assert!(block_on(sqpack.open("unregistered/file.dat")).is_ok());
    assert_eq!(opens.get(), 1);
}

#[test]
fn default_has_no_read_cap_but_still_checks_response_length() {
    let requested = 8 * 1024 * 1024 + 1;
    let mut source = source();
    let file = source.file.as_mut().unwrap();
    file.size = u64::try_from(requested).unwrap();
    // No large fixture or allocation: a backend call returns an empty response.
    file.returned_length = Some(0);
    let reads = Rc::clone(&file.reads);
    let sqpack = SqPack::new(source);
    let reader = block_on(sqpack.open(name())).unwrap();
    assert_eq!(
        block_on(reader.read_at(0, requested)),
        Err(Error::UnexpectedReadLength {
            expected: requested,
            actual: 0
        })
    );
    assert_eq!(reads.get(), 1);
}

#[test]
fn none_removes_a_cap_without_disabling_range_checks() {
    let source = source();
    let reads = Rc::clone(&source.file.as_ref().unwrap().reads);
    let sqpack = SqPack::new(source)
        .with_max_read_bytes(Some(0))
        .with_max_read_bytes(None);
    let reader = block_on(sqpack.open(name())).unwrap();
    assert_eq!(block_on(reader.read_at(0, 4)), Ok(vec![10, 20, 30, 40]));
    assert_eq!(
        block_on(reader.read_at(4, 1)),
        Err(Error::OutOfBounds {
            offset: 4,
            length: 1,
            size: 4
        })
    );
    assert_eq!(
        block_on(reader.read_at(u64::MAX, 1)),
        Err(Error::RangeOverflow)
    );
    assert_eq!(reads.get(), 1);
}

#[test]
fn zero_cap_allows_only_empty_reads_even_for_a_nonempty_file() {
    let source = source();
    let reads = Rc::clone(&source.file.as_ref().unwrap().reads);
    let sqpack = SqPack::new(source).with_max_read_bytes(Some(0));
    let reader = block_on(sqpack.open(name())).unwrap();
    assert_eq!(block_on(reader.read_at(4, 0)), Ok(vec![]));
    assert_eq!(
        block_on(reader.read_at(0, 1)),
        Err(Error::ReadTooLarge {
            requested: 1,
            limit: 0
        })
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn non_send_source_reads_exact_ranges_and_empty_reads_avoid_io() {
    let source = source();
    let reads = Rc::clone(&source.file.as_ref().unwrap().reads);
    let sqpack = pack(source);
    let reader = block_on(sqpack.open(name())).unwrap();
    assert_eq!(reader.len(), 4);
    assert!(!reader.is_empty());
    assert_eq!(block_on(reader.read_at(1, 2)), Ok(vec![20, 30]));
    assert_eq!(block_on(reader.read_at(4, 0)), Ok(vec![]));
    assert_eq!(reads.get(), 1);
}

#[test]
fn missing_files_are_confirmed_by_the_source_and_distinct_from_failures() {
    let mut source = source();
    let opens = Rc::clone(&source.opens);
    source.file = None;
    let sqpack = pack(source);
    let missing = "other.dat";
    assert!(
        matches!(block_on(sqpack.open(missing)), Err(Error::MissingFile(file)) if file == missing)
    );
    assert_eq!(opens.get(), 1);
    assert!(
        matches!(block_on(sqpack.open(name())), Err(Error::MissingFile(file)) if file == name())
    );
    assert_eq!(opens.get(), 2);

    for failure in [Failure::Permission, Failure::Transport] {
        let mut source = self::source();
        source.failure = Some(failure);
        assert!(
            matches!(block_on(pack(source).open(name())), Err(Error::Source(error)) if error == failure)
        );
    }
}

#[test]
fn invalid_ranges_and_budget_excess_are_rejected_before_io() {
    let source = source();
    let reads = Rc::clone(&source.file.as_ref().unwrap().reads);
    let reader = block_on(pack(source).open(name())).unwrap();
    assert_eq!(
        block_on(reader.read_at(0, 5)),
        Err(Error::ReadTooLarge {
            requested: 5,
            limit: 4
        })
    );
    assert_eq!(
        block_on(reader.read_at(3, 2)),
        Err(Error::OutOfBounds {
            offset: 3,
            length: 2,
            size: 4
        })
    );
    assert_eq!(
        block_on(reader.read_at(5, 0)),
        Err(Error::OutOfBounds {
            offset: 5,
            length: 0,
            size: 4
        })
    );
    assert_eq!(
        block_on(reader.read_at(u64::MAX, 1)),
        Err(Error::RangeOverflow)
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn short_and_oversized_backend_responses_are_not_accepted() {
    for actual in [1, 3] {
        let mut source = source();
        source.file.as_mut().unwrap().returned_length = Some(actual);
        let reader = block_on(pack(source).open(name())).unwrap();
        assert_eq!(
            block_on(reader.read_at(0, 2)),
            Err(Error::UnexpectedReadLength {
                expected: 2,
                actual
            })
        );
    }
}

#[test]
fn read_failures_preserve_backend_errors() {
    let mut source = source();
    source.file.as_mut().unwrap().failure = Some(Failure::VersionChanged);
    let reader = block_on(pack(source).open(name())).unwrap();
    assert_eq!(
        block_on(reader.read_at(0, 1)),
        Err(Error::Source(Failure::VersionChanged))
    );
}

#[test]
fn empty_files_and_zero_budget_allow_only_empty_reads() {
    let mut source = source();
    source.file.as_mut().unwrap().size = 0;
    let sqpack = SqPack::new(source).with_max_read_bytes(Some(0));
    let reader = block_on(sqpack.open(name())).unwrap();
    assert!(reader.is_empty());
    assert_eq!(block_on(reader.read_at(0, 0)), Ok(vec![]));
    assert_eq!(
        block_on(reader.read_at(0, 1)),
        Err(Error::ReadTooLarge {
            requested: 1,
            limit: 0
        })
    );
}

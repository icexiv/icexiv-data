//! Local container access using Tokio's blocking pool, not blocking executor threads.
//!
//! Requires a Tokio runtime and a trusted, stable data directory. Canonicalization
//! rejects resolved paths outside the root but is not a sandbox against concurrent
//! path changes. Length/mtime checks are best-effort, not filesystem snapshots.
//! Cancelling an awaiting future does not stop already-started blocking work.
//! This backend checks ranges and file consistency, but has no allocation-budget
//! setting. Use `SqPack`/`PackReader` to opt into a per-request cap; direct
//! `ReadAt` callers are responsible for choosing a bounded request length.

use super::{PackSource, ReadAt, is_valid_file_name};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Local {
    root: PathBuf,
}

impl Local {
    /// Resolve the SqPack root off-thread. No directory listing is performed.
    pub async fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        blocking(move || {
            let root = root.canonicalize()?;
            if !root.metadata()?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "root must be a directory",
                ));
            }
            Ok(Self { root })
        })
        .await
    }
}

impl PackSource for Local {
    type Error = io::Error;
    type File = LocalFile;

    async fn open(&self, name: &str) -> io::Result<Option<LocalFile>> {
        if !is_valid_file_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid relative container name",
            ));
        }
        let root = self.root.clone();
        let name = name.to_owned();
        blocking(move || {
            let path = match root.join(name).canonicalize() {
                Ok(path) => path,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            };
            if !path.starts_with(&root) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "container escapes data root",
                ));
            }
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            };
            let metadata = file.metadata()?;
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "container must be a regular file",
                ));
            }
            Ok(Some(LocalFile {
                size: metadata.len(),
                modified: metadata.modified()?,
                file: Arc::new(file),
            }))
        })
        .await
    }
}

/// An opened container with no application-managed shared cursor or locks.
#[derive(Clone, Debug)]
pub struct LocalFile {
    file: Arc<File>,
    size: u64,
    modified: SystemTime,
}

impl LocalFile {
    fn check_unchanged(&self) -> io::Result<()> {
        let metadata = self.file.metadata()?;
        if metadata.len() != self.size || metadata.modified()? != self.modified {
            return Err(io::Error::other(
                "container changed; reopen against a stable installation",
            ));
        }
        Ok(())
    }
}

impl ReadAt for LocalFile {
    type Error = io::Error;

    fn len(&self) -> u64 {
        self.size
    }

    async fn read_at(&self, offset: u64, length: usize) -> io::Result<Vec<u8>> {
        let count = u64::try_from(length)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "length overflow"))?;
        if offset.checked_add(count).is_none_or(|end| end > self.size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "read exceeds file range",
            ));
        }
        let this = self.clone();
        blocking(move || {
            this.check_unchanged()?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length).map_err(io::Error::other)?;
            bytes.resize(length, 0);
            read_exact_at(&this.file, &mut bytes, offset)?;
            this.check_unchanged()?;
            Ok(bytes)
        })
        .await
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    let runtime = tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
    runtime
        .spawn_blocking(work)
        .await
        .map_err(io::Error::other)?
}

#[cfg(unix)]
fn read_exact_at(file: &File, bytes: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(bytes, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut bytes: &mut [u8], mut offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    // This API may affect the OS cursor; every call specifies its own offset,
    // and this backend never relies on the shared cursor.
    while !bytes.is_empty() {
        match file.seek_read(bytes, offset) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(count) => {
                offset = offset.checked_add(count as u64).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "offset overflow")
                })?;
                bytes = &mut bytes[count..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn read_exact_at(_file: &File, _bytes: &mut [u8], _offset: u64) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "local reads require Unix or Windows positioned I/O",
    ))
}

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "icexiv-local-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            std::fs::write(root.join("synthetic.dat"), (0..=255).collect::<Vec<u8>>()).unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("synthetic.dat"));
            let _ = std::fs::remove_file(self.0.join("escape.dat"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
    }

    async fn fixture_file(fixture: &Fixture) -> LocalFile {
        Local::new(&fixture.0)
            .await
            .unwrap()
            .open("synthetic.dat")
            .await
            .unwrap()
            .unwrap()
    }

    #[test]
    fn shared_handle_supports_independent_concurrent_ranges() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LocalFile>();
        let fixture = Fixture::new();
        let runtime = runtime();
        let file = runtime.block_on(fixture_file(&fixture));
        std::thread::scope(|scope| {
            for offset in [128, 0, 64, 192, 32, 128] {
                let file = &file;
                let runtime = &runtime;
                scope.spawn(move || {
                    let bytes = runtime.handle().block_on(file.read_at(offset, 32)).unwrap();
                    assert_eq!(
                        bytes,
                        (offset as u8..offset as u8 + 32).collect::<Vec<u8>>()
                    );
                });
            }
        });
    }

    #[test]
    fn ranges_and_mutation_are_checked() {
        let fixture = Fixture::new();
        runtime().block_on(async {
            let file = fixture_file(&fixture).await;
            assert_eq!(file.read_at(256, 0).await.unwrap(), b"");
            for (offset, length) in [(256, 1), (u64::MAX, 1), (0, 257)] {
                assert_eq!(
                    file.read_at(offset, length).await.unwrap_err().kind(),
                    io::ErrorKind::InvalidInput
                );
            }
            let mut bytes = [0; 2];
            assert_eq!(
                read_exact_at(&file.file, &mut bytes, 255)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::UnexpectedEof
            );
            std::fs::OpenOptions::new()
                .write(true)
                .open(fixture.0.join("synthetic.dat"))
                .unwrap()
                .set_len(2)
                .unwrap();
            assert!(
                file.read_at(0, 1)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("changed")
            );
        });
    }

    #[test]
    fn read_budget_is_configured_only_at_sqpack() {
        use crate::sqpack::{Error, SqPack};

        let fixture = Fixture::new();
        runtime().block_on(async {
            let source = Local::new(&fixture.0).await.unwrap();
            let name = "synthetic.dat";
            let limited = SqPack::new(source.clone()).with_max_read_bytes(Some(4));
            let reader = limited.open(name).await.unwrap();
            assert!(matches!(
                reader.read_at(0, 5).await,
                Err(Error::ReadTooLarge {
                    requested: 5,
                    limit: 4
                })
            ));
            assert_eq!(reader.read_at(0, 4).await.unwrap(), [0, 1, 2, 3]);

            // Reuse the exact same backend with another policy, no backend
            // setting to update or accidentally leave at the old limit.
            let normal = SqPack::new(source.clone());
            assert_eq!(
                normal
                    .open(name)
                    .await
                    .unwrap()
                    .read_at(0, 5)
                    .await
                    .unwrap(),
                [0, 1, 2, 3, 4]
            );
            let larger = SqPack::new(source.clone()).with_max_read_bytes(Some(8));
            assert_eq!(
                larger
                    .open(name)
                    .await
                    .unwrap()
                    .read_at(0, 8)
                    .await
                    .unwrap(),
                [0, 1, 2, 3, 4, 5, 6, 7]
            );
            assert_eq!(
                source
                    .open(name)
                    .await
                    .unwrap()
                    .unwrap()
                    .read_at(0, 5)
                    .await
                    .unwrap(),
                [0, 1, 2, 3, 4]
            );
        });
    }

    #[test]
    fn missing_is_not_a_directory_or_other_io_error() {
        let fixture = Fixture::new();
        runtime().block_on(async {
            let source = Local::new(&fixture.0).await.unwrap();
            assert!(source.open("missing.dat").await.unwrap().is_none());
            assert_eq!(
                Local::new(fixture.0.join("synthetic.dat"))
                    .await
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
        });
    }

    #[test]
    fn direct_opens_reject_invalid_names_before_filesystem_access() {
        // No runtime and no real root: validation must precede blocking work.
        use std::future::Future;
        use std::task::{Context, Poll, Waker};

        let source = Local {
            root: PathBuf::from("unused"),
        };
        for name in [
            "",
            "/absolute.dat",
            "../escape.dat",
            "a/../b",
            "a\\b",
            "a?b",
            "a%2fb",
        ] {
            let mut future = std::pin::pin!(source.open(name));
            let mut cx = Context::from_waker(Waker::noop());
            assert!(
                matches!(future.as_mut().poll(&mut cx), Poll::Ready(Err(error)) if error.kind() == io::ErrorKind::InvalidInput)
            );
        }
    }

    #[test]
    fn blocking_work_runs_off_the_executor_thread() {
        let caller = std::thread::current().id();
        let worker = runtime()
            .block_on(blocking(|| Ok(std::thread::current().id())))
            .unwrap();
        assert_ne!(caller, worker);
    }

    #[cfg(unix)]
    #[test]
    fn positioned_reads_do_not_touch_the_os_cursor() {
        use std::io::{Seek, SeekFrom};
        let fixture = Fixture::new();
        runtime().block_on(async {
            let file = fixture_file(&fixture).await;
            let mut cursor = file.file.try_clone().unwrap();
            cursor.seek(SeekFrom::Start(7)).unwrap();
            assert_eq!(file.read_at(128, 4).await.unwrap(), [128, 129, 130, 131]);
            assert_eq!(cursor.stream_position().unwrap(), 7);
        });
    }

    #[cfg(unix)]
    #[test]
    fn resolved_symlink_escape_is_rejected() {
        let fixture = Fixture::new();
        std::os::unix::fs::symlink(
            std::env::current_exe().unwrap(),
            fixture.0.join("escape.dat"),
        )
        .unwrap();
        runtime().block_on(async {
            let source = Local::new(&fixture.0).await.unwrap();
            assert_eq!(
                source.open("escape.dat").await.unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
        });
    }
}

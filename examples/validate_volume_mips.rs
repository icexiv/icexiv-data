//! Args: pinned-capture-directory new-output-directory.
//! Replays captured type-4 entries through the public backend-independent API.
use icexiv_data::{
    sqpack::{PackSource, ReadAt, SqPack},
    tex,
};
use std::{
    future::Future,
    io::Write,
    path::Path,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

struct Capture(Arc<Vec<u8>>);
impl PackSource for Capture {
    type Error = std::io::Error;
    type File = Capture;
    async fn open(&self, name: &str) -> Result<Option<Self::File>, Self::Error> {
        Ok((name == "capture.dat0").then(|| Capture(self.0.clone())))
    }
}
impl ReadAt for Capture {
    type Error = std::io::Error;
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Self::Error> {
        let offset = usize::try_from(offset).map_err(std::io::Error::other)?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| std::io::Error::other("range overflow"))?;
        Ok(self
            .0
            .get(offset..end)
            .ok_or_else(|| std::io::Error::other("range outside capture"))?
            .to_vec())
    }
}
fn block_on<T>(future: impl Future<Output = T>) -> T {
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}
fn save(path: &Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(data)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("expected captures new-output-directory".into());
    }
    let output = Path::new(&args[1]);
    std::fs::create_dir(output)?;
    let mut files = std::fs::read_dir(&args[0])?.collect::<Result<Vec<_>, _>>()?;
    if files.len() > 4096 {
        return Err("capture count budget".into());
    }
    files.sort_by_key(|f| f.file_name());
    let mut rows = String::from("name\tmip\tstatus\tdetail\n");
    let mut whole = String::from("name\tstatus\tdetail\n");
    for file in files {
        let path = file.path();
        if path.extension().is_none_or(|v| v != "entry") {
            continue;
        }
        if file.metadata()?.len() > 4 << 20 {
            return Err("capture byte budget".into());
        }
        let bytes = std::fs::read(&path)?;
        let count =
            u32::from_le_bytes(bytes.get(20..24).ok_or("entry prefix")?.try_into()?) as usize;
        if !(1..=13).contains(&count) {
            return Err("group count outside verification profile".into());
        }
        let name = path
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or("capture name")?;
        let sqpack = SqPack::new(Capture(Arc::new(bytes))).with_max_read_bytes(Some(4 << 20));
        match block_on(sqpack.read_entry("capture.dat0", 0)) {
            Ok(bytes) => whole += &format!("{name}\tok\t{}\n", bytes.len()),
            Err(error) => whole += &format!("{name}\terror\t{error}\n"),
        }
        for index in 0..count {
            let group = block_on(sqpack.read_texture_group_entry("capture.dat0", 0, index))?;
            if index == 0 {
                save(&output.join(format!("{name}.header")), group.prefix())?;
            }
            match group.volume_mip() {
                Ok(mip) => {
                    let rgba = mip.decode_rgba8(4 << 20)?;
                    save(&output.join(format!("{name}-m{index}.rgba")), &rgba)?;
                    save(&output.join(format!("{name}-m{index}.encoded")), mip.data())?;
                    rows += &format!(
                        "{name}\t{index}\tok\t{}x{}x{};{};source_mips={};source_index={}\n",
                        mip.width(),
                        mip.height(),
                        mip.depth(),
                        rgba.len(),
                        mip.source_mip_count(),
                        mip.source_index()
                    );
                }
                Err(error @ tex::Error::MipSizeMismatch { .. }) => {
                    rows += &format!("{name}\t{index}\terror\t{error:?}\n");
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    save(&output.join("results.tsv"), rows.as_bytes())?;
    save(&output.join("whole.tsv"), whole.as_bytes())?;
    Ok(())
}

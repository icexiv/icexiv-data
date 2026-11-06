//! Read an index-enumerated DAT entry without guessing its hashed resource path.
//! Args: game/sqpack relative-dat-name absolute-entry-offset new-output-file

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            use icexiv_data::sqpack::{SqPack, local::Local};
            use std::io::Write;
            let args: Vec<_> = std::env::args().skip(1).collect();
            if args.len() != 4 {
                return Err("expected sqpack-root dat-name offset new-output-file".into());
            }
            let sqpack = SqPack::new(Local::new(&args[0]).await?)
                .with_max_read_bytes(Some(64 * 1024 * 1024));
            let bytes = sqpack.read_entry(&args[1], args[2].parse()?).await?;
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&args[3])?
                .write_all(&bytes)?;
            println!("{} bytes from {} at {}", bytes.len(), args[1], args[2]);
            Ok(())
        })
}
#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("extract_entry requires a native platform");
}

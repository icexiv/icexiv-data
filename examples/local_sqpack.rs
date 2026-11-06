//! Extract a game resource using the asynchronous Local backend.
//! Does not parse the extracted resource's own file format.
//!
//! # Usage
//!
//! ```sh
//! cargo run --features local --example local_sqpack -- /path/to/game/sqpack exd/item.exh
//! ```
//!
//! Arguments are the SqPack root (`game/sqpack`, not `game`), a resource path,
//! and an optional output file (created exclusively; never overwritten).
//! Prints the extracted byte length and a hex preview of the first 64 bytes.
//! Supports the entry types implemented by SqPack.
//! Reads and output default to a 64 MiB cap; `--max-bytes N` overrides it.
//! Requires a native platform;
//! the example creates its own Tokio runtime. Keep the installation stable while
//! reading it.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The local_sqpack example requires a native platform.");
}

#[cfg(not(target_arch = "wasm32"))]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use icexiv_data::sqpack::{SqPack, local::Local};
    use std::path::Path;

    const USAGE: &str = "Usage: cargo run --features local --example local_sqpack -- [--max-bytes N] <game/sqpack> <resource-path> [output-file]\n\
        Extracts standard binary entries, e.g. exd/item.exh, and previews 64 bytes.\n\
        Default read/output cap: 64 MiB; --max-bytes overrides it. Do not patch the installation while reading.";
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    let limit = if args.first().is_some_and(|a| a == "--max-bytes") {
        let limit = args
            .get(1)
            .and_then(|n| n.to_str())
            .ok_or(USAGE)?
            .parse::<usize>()?;
        args.drain(..2);
        limit
    } else {
        64 * 1024 * 1024
    };
    if !(2..=3).contains(&args.len()) {
        return Err(USAGE.into());
    }
    let path = args[1].to_str().ok_or("resource path must be ASCII")?;
    let source = Local::new(Path::new(&args[0])).await?;
    let sqpack = SqPack::new(source).with_max_read_bytes(Some(limit));
    let bytes = sqpack.read(path).await?;
    if let Some(output) = args.get(2) {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?
            .write_all(&bytes)?;
    }
    println!("Resource: {path} ({} bytes extracted)", bytes.len());
    println!("Preview (resource format not parsed):");
    for (line, chunk) in bytes[..bytes.len().min(64)].chunks(16).enumerate() {
        print!("{:08x}:", line * 16);
        for byte in chunk {
            print!(" {byte:02x}");
        }
        println!();
    }
    Ok(())
}

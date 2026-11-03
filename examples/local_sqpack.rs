//! Extract a standard binary game resource using the asynchronous Local backend.
//! Does not parse the extracted resource's own file format.

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

    const USAGE: &str = "Usage: cargo run --features local --example local_sqpack -- <game/sqpack> <resource-path>\n\
        Extracts standard binary entries, e.g. exd/item.exh, and previews 64 bytes.\n\
        Read/output cap: 64 MiB. Do not patch the installation while reading.";
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    if args.len() != 2 {
        return Err(USAGE.into());
    }
    let path = args[1].to_str().ok_or("resource path must be ASCII")?;
    let source = Local::new(Path::new(&args[0])).await?;
    let sqpack = SqPack::new(source).with_max_read_bytes(Some(64 * 1024 * 1024));
    let bytes = sqpack.read(path).await?;
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

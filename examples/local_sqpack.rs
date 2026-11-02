//! Inspect raw container ranges using the built-in asynchronous Local backend.
//! Index lookup and resource decompression are not implemented yet.

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

    const USAGE: &str = "Usage: cargo run --features local --example local_sqpack -- <game/sqpack> <relative-container> [offset=0] [length=64]\n\
        Reads raw bytes only; index lookup and decompression are not implemented.\n\
        Offset and length are decimal. Maximum read: 1 MiB. Do not patch the installation while reading.";
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    if !(2..=4).contains(&args.len()) {
        return Err(USAGE.into());
    }
    let file = args[1].to_str().ok_or("container name must be ASCII")?;
    let offset = args
        .get(2)
        .map(|s| {
            s.to_str()
                .ok_or("offset must be decimal")?
                .parse::<u64>()
                .map_err(|_| "invalid offset")
        })
        .transpose()?
        .unwrap_or(0);
    let length = args
        .get(3)
        .map(|s| {
            s.to_str()
                .ok_or("length must be decimal")?
                .parse::<usize>()
                .map_err(|_| "invalid length")
        })
        .transpose()?
        .unwrap_or(64);

    let source = Local::new(Path::new(&args[0])).await?;
    let sqpack = SqPack::new(source).with_max_read_bytes(Some(1024 * 1024));
    let reader = sqpack.open(file).await?;
    let bytes = reader.read_at(offset, length).await?;
    println!("Container: {file} ({} bytes)", reader.len());
    println!(
        "Raw range: offset {offset}, length {} (not decoded)",
        bytes.len()
    );
    for (line, chunk) in bytes.chunks(16).enumerate() {
        print!("{:016x}:", offset + (line as u64) * 16);
        for byte in chunk {
            print!(" {byte:02x}");
        }
        println!();
    }
    Ok(())
}

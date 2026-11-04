//! Extract one texture resource and decode mip 0 to a PPM image.
//!
//! # Usage
//!
//! ```sh
//! cargo run --features local --example extract_texture -- \
//!   /path/to/game/sqpack ui/icon/062000/062101_hr1.tex out.ppm
//! ```
//!
//! The argument is the SqPack root (`game/sqpack`, not `game`). The output is a
//! binary P6 PPM with alpha composited over black, which keeps the command
//! dependency-free; use the TEX API directly for RGBA output. Reads and decoded
//! output are capped at 64 MiB. Requires a native platform.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The extract_texture example requires a native platform.");
}

#[cfg(not(target_arch = "wasm32"))]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use icexiv_data::sqpack::{SqPack, local::Local};
    use icexiv_data::tex::Tex;
    use std::io::Write;
    use std::path::Path;

    const USAGE: &str = "Usage: cargo run --features local --example extract_texture -- \
        <game/sqpack> <resource-path> <output.ppm>\n\
        Extracts a texture entry, decodes mip 0 and writes a P6 PPM composited over black.\n\
        Read/output cap: 64 MiB. Do not patch the installation while reading.";
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    if args.len() != 3 {
        return Err(USAGE.into());
    }
    let path = args[1].to_str().ok_or("resource path must be ASCII")?;
    let source = Local::new(Path::new(&args[0])).await?;
    let sqpack = SqPack::new(source).with_max_read_bytes(Some(64 * 1024 * 1024));
    let bytes = sqpack.read(path).await?;
    let mip = Tex::parse(&bytes)?.mip(0)?;
    let rgba = mip.decode_rgba8(64 * 1024 * 1024)?;
    let (width, height) = (usize::from(mip.width()), usize::from(mip.height()));
    let mut out = Vec::with_capacity(width * height * 3);
    for pixel in rgba.chunks_exact(4) {
        for channel in &pixel[..3] {
            out.push((u16::from(*channel) * u16::from(pixel[3]) / 255) as u8);
        }
    }
    let mut file = std::fs::File::create(&args[2])?;
    write!(file, "P6\n{width} {height}\n255\n")?;
    file.write_all(&out)?;
    println!(
        "{path}: {:?} {width}x{height}, {} bytes extracted",
        mip.format(),
        bytes.len()
    );
    Ok(())
}

//! Decode every TEX mip/image to caller-selected new output directory, CPU only.
//! Args: new-output-directory [--packed-volume] extracted.tex ...
//! Raw RGBA output permits exact independent comparison, not only a thumbnail.
use icexiv_data::tex::Tex;
use std::io::{BufWriter, Write};
struct Hash(u64);
impl Hash {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn data(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).peekable();
    let output = std::path::PathBuf::from(args.next().ok_or("expected new output directory")?);
    std::fs::create_dir(&output)?;
    let packed = args.next_if(|arg| arg == "--packed-volume").is_some();
    let mut samples = 0;
    for (sample, path) in args.enumerate() {
        if std::fs::metadata(&path)?.len() > 64 * 1024 * 1024 {
            return Err("input exceeds 64 MiB".into());
        }
        let bytes = std::fs::read(path)?;
        let tex = if packed {
            Tex::parse_packed_volume(&bytes)?
        } else {
            Tex::parse(&bytes)?
        };
        println!(
            "tex\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}",
            bytes.len(),
            tex.layout(),
            tex.format() as u32,
            tex.width(),
            tex.height(),
            tex.depth(),
            tex.raw_array_size(),
            tex.mip_count()
        );
        for (m, mip) in tex.mips().enumerate() {
            let rgba = mip.decode_rgba8(128 * 1024 * 1024)?;
            let mut hash = Hash::new();
            hash.data(&rgba);
            std::fs::write(output.join(format!("{sample}-{m}.rgba8")), &rgba)?;
            let floating = if tex.format().float_channels().is_some() {
                Some(mip.decode_rgba32f(128 * 1024 * 1024)?)
            } else {
                None
            };
            let mut fh = Hash::new();
            if let Some(ref values) = floating {
                let mut writer = BufWriter::new(std::fs::File::create_new(
                    output.join(format!("{sample}-{m}.rgba32f")),
                )?);
                for value in values {
                    let raw = value.to_le_bytes();
                    fh.data(&raw);
                    writer.write_all(&raw)?;
                }
                writer.flush()?;
            }
            println!(
                "mip\t{m}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}\t{}",
                mip.width(),
                mip.height(),
                mip.depth(),
                mip.layer_count(),
                mip.face_count(),
                mip.image_count(),
                mip.data().len(),
                rgba.len(),
                hash.0,
                floating
                    .as_ref()
                    .map_or_else(|| "absent".into(), |_| format!("{:016x}", fh.0))
            );
            let stride = usize::from(mip.width()) * usize::from(mip.height()) * 4;
            for (image, surface) in mip.images().enumerate() {
                let depth = usize::from(mip.depth());
                let face = (image / depth) % mip.face_count();
                let layer = image / (depth * mip.face_count());
                if mip.image(layer, face, image % depth)?.data() != surface.data()
                    || mip.image_by_index(image)?.data() != surface.data()
                {
                    return Err("coordinate and sequential image access disagree".into());
                }
                let decoded = surface.decode_rgba8(128 * 1024 * 1024)?;
                if decoded != rgba[image * stride..(image + 1) * stride] {
                    return Err("surface and complete-mip decode disagree".into());
                }
                if let Some(ref complete) = floating {
                    let decoded = surface.decode_rgba32f(128 * 1024 * 1024)?;
                    if decoded.len() != stride
                        || !decoded
                            .iter()
                            .zip(&complete[image * stride..(image + 1) * stride])
                            .all(|(a, b)| a.to_bits() == b.to_bits())
                    {
                        return Err("surface and complete-mip float bits disagree".into());
                    }
                }
                let mut raw = Hash::new();
                raw.data(surface.data());
                println!("image\t{image}\t{}\t{:016x}", surface.data().len(), raw.0);
            }
        }
        samples += 1;
    }
    if samples == 0 {
        return Err("expected at least one TEX file".into());
    }
    Ok(())
}

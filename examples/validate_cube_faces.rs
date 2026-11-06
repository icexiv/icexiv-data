//! Export named cube faces as encoded bytes; no graphics API or pixel transform.
use icexiv_data::tex::{CubeFace, Layout, Tex};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let out = std::path::PathBuf::from(args.next().ok_or("output directory")?);
    std::fs::create_dir(&out)?;
    for (file, path) in args.enumerate() {
        if std::fs::metadata(&path)?.len() > 64 << 20 {
            return Err("TEX size".into());
        }
        let bytes = std::fs::read(path)?;
        let tex = Tex::parse(&bytes)?;
        if tex.layout() != Layout::Cube {
            return Err("cube required".into());
        }
        for mip in 0..tex.mip_count() {
            let m = tex.mip(mip)?;
            for face in CubeFace::ALL {
                let data = m.cube_face(face)?.data();
                std::fs::write(out.join(format!("{file}-{mip}-{}.bin", face.index())), data)?;
                println!(
                    "face\t{file}\t{mip}\t{face:?}\t{}\t{}",
                    face.index(),
                    data.len()
                );
            }
        }
    }
    Ok(())
}

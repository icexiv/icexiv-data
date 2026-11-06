//! Args: legacy.stm dawntrail.stm jobs.bin new-output.bin.
//! Each job is u16 MTRL length, two u8 stains, then the complete MTRL bytes.
//! Each result is u8 profile, 2048 base bytes, 2048 output bytes, 32 u32 write
//! masks and a u32 missing-template row mask. All integers are little-endian.
use icexiv_data::{
    mtrl::{DyeOverrideProfile, Mtrl, dye},
    stm::Stm,
};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("expected legacy.stm dawntrail.stm jobs new-output".into());
    }
    for path in &args[..3] {
        if std::fs::metadata(path)?.len() > 16 << 20 {
            return Err("input file budget".into());
        }
    }
    let legacy = std::fs::read(&args[0])?;
    let modern = std::fs::read(&args[1])?;
    let stms = [Stm::parse(&legacy)?, Stm::parse(&modern)?];
    let jobs = std::fs::read(&args[2])?;
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[3])?;
    let mut out = std::io::BufWriter::new(file);
    let mut remaining = jobs.as_slice();
    let mut count = 0;
    while !remaining.is_empty() {
        let header = remaining.get(..4).ok_or("partial job header")?;
        let size = usize::from(u16::from_le_bytes([header[0], header[1]]));
        let stains = [header[2], header[3]];
        let mtrl = Mtrl::parse(remaining.get(4..4 + size).ok_or("partial material")?)?;
        let base = mtrl
            .canonical_inline_color_table()?
            .ok_or("no inline color declaration")?;
        let profile = DyeOverrideProfile::for_shader_package(mtrl.shader_package()?);
        let profile_index = usize::from(profile == DyeOverrideProfile::Dawntrail);
        let result = dye::apply_table(&base, profile, Some(stms[profile_index]), stains)?;
        out.write_all(&[profile_index as u8])?;
        out.write_all(&base.to_le_bytes())?;
        out.write_all(&result.to_le_bytes())?;
        for mask in result.written_lane_masks {
            out.write_all(&mask.to_le_bytes())?;
        }
        out.write_all(&result.missing_template_rows.to_le_bytes())?;
        remaining = &remaining[4 + size..];
        count += 1;
    }
    out.flush()?;
    println!("{count} complete color tables");
    Ok(())
}

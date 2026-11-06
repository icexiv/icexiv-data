//! Args: legacy.stm dawntrail.stm jobs.bin new-output.bin.
//! Jobs: layout/profile/channel/reserved bytes, u32 dye, u16 stain, u16 reserved,
//! 32 u16 source half bits (76 bytes). Channel 255 means explicitly selected.
//! Results: status byte, u32 written mask, 32 u16 half bits (69 bytes).
use icexiv_data::{
    mtrl::{
        ColorLayout, DyeOverrideProfile, DyeRow,
        dye::{self, Error},
    },
    stm::{self, Stm},
};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("expected legacy.stm dawntrail.stm jobs new-output".into());
    }
    for path in &args[..2] {
        if std::fs::metadata(path)?.len() > 16 << 20 {
            return Err("STM file budget".into());
        }
    }
    let a = std::fs::read(&args[0])?;
    let b = std::fs::read(&args[1])?;
    let stms = [Stm::parse(&a)?, Stm::parse(&b)?];
    if std::fs::metadata(&args[2])?.len() > 16 << 20 {
        return Err("jobs budget".into());
    }
    let jobs = std::fs::read(&args[2])?;
    if !jobs.len().is_multiple_of(76) {
        return Err("partial job".into());
    }
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[3])?;
    let mut out = std::io::BufWriter::new(file);
    for job in jobs.chunks_exact(76) {
        let layout = match job[0] {
            0 => ColorLayout::Legacy,
            1 => ColorLayout::Dawntrail,
            _ => return Err("job layout".into()),
        };
        let profile = match job[1] {
            0 => DyeOverrideProfile::Legacy,
            1 => DyeOverrideProfile::Dawntrail,
            _ => return Err("job profile".into()),
        };
        let word = DyeRow {
            layout,
            raw: u32::from_le_bytes(job[4..8].try_into()?),
        };
        let stain = u16::from_le_bytes(job[8..10].try_into()?);
        let base = std::array::from_fn(|i| u16::from_le_bytes([job[12 + i * 2], job[13 + i * 2]]));
        let stm = stms[usize::from(job[1])];
        let result = if job[2] == 255 {
            dye::apply_selected_row(&base, word, profile, stm, stain).map(Some)
        } else {
            dye::apply_channel_row(&base, word, profile, stm, stain, job[2])
        };
        let (status, mask, bits) = match result {
            Ok(Some(row)) => (0, row.written_lane_mask, row.half_bits),
            Ok(None) => (1, 0, base),
            Err(Error::MissingTemplate(_)) => (2, 0, base),
            Err(Error::Stm(stm::Error::StainOutOfBounds { .. })) => (3, 0, base),
            Err(Error::ChannelNotEncoded) => (4, 0, base),
            Err(Error::InvalidChannel(_)) => (5, 0, base),
            Err(other) => return Err(other.into()),
        };
        out.write_all(&[status])?;
        out.write_all(&mask.to_le_bytes())?;
        for half in bits {
            out.write_all(&half.to_le_bytes())?;
        }
    }
    out.flush()?;
    println!("{} row requests", jobs.len() / 76);
    Ok(())
}

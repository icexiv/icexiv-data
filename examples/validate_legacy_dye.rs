//! Args: controlled 544-byte-dataset MTRL. Exhaust all u16 dye words in place.
//! File writing and sample preparation belong to the external evidence tool.
use icexiv_data::mtrl::{ColorLayout, Mtrl};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("expected controlled MTRL")?;
    if std::fs::metadata(&path)?.len() > u16::MAX.into() {
        return Err("MTRL size".into());
    }
    let mut bytes = std::fs::read(path)?;
    let m = Mtrl::parse(&bytes)?;
    let d = m
        .inline_color_table_declaration()
        .ok_or("expected inline declaration")?;
    if (d.half_lanes, d.rows, d.has_dye) != (16, 16, true) {
        return Err("Legacy declaration".into());
    }
    let t = m.color_table()?.ok_or("expected color table")?;
    if t.layout() != ColorLayout::Legacy || !t.has_dye() {
        return Err("expected Legacy dye table".into());
    }
    let dataset = 16
        + (usize::from(bytes[12]) + usize::from(bytes[13]) + usize::from(bytes[14])) * 4
        + usize::from(u16::from_le_bytes([bytes[8], bytes[9]]))
        + usize::from(bytes[15]);
    let colors = bytes
        .get(dataset..dataset + 512)
        .ok_or("color range")?
        .to_vec();
    for page in 0..4096u32 {
        for row in 0..16usize {
            let at = dataset + 512 + row * 2;
            bytes
                .get_mut(at..at + 2)
                .ok_or("dye range")?
                .copy_from_slice(&((page * 16 + row as u32) as u16).to_le_bytes());
        }
        let m = Mtrl::parse(&bytes)?;
        let t = m.color_table()?.ok_or("color table")?;
        for row in 0..16usize {
            if t.row(row)?.raw() != &colors[row * 32..(row + 1) * 32] {
                return Err("color rows changed".into());
            }
            let dye = t.dye_row(row)?.ok_or("dye row")?;
            println!(
                "dye\t{}\t{}\t{}\t{}\t{}",
                dye.raw,
                dye.flags(),
                dye.template(),
                dye.channel()
                    .map_or_else(|| "none".into(), |v| v.to_string()),
                dye.unknown_bits()
            );
        }
    }
    Ok(())
}

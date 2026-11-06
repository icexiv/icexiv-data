//! Emit complete STM half/f32 bits for independent comparison, without game access.
//! Args: legacy.stm dawntrail.stm. Little-endian u32 pairs (half bits, f32 bits),
//! three lanes per stain, in file/template/column/stain/lane order. No header.
use icexiv_data::stm::Stm;
use std::io::{BufWriter, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.len() != 2 {
        return Err("expected two extracted STM paths".into());
    }
    let mut remaining = 16 * 1024 * 1024usize;
    let mut output = BufWriter::new(std::io::stdout().lock());
    for path in paths {
        let input = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        input.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("input exceeds 16 MiB".into());
        }
        let stm = Stm::parse(&bytes)?;
        for template in 0..stm.template_count() {
            let template = stm.template(template)?;
            for column in 0..template.column_count() {
                let column = template.column(column)?;
                for stain in 1..=column.stain_capacity() as u16 {
                    remaining = remaining.checked_sub(24).ok_or("output exceeds 16 MiB")?;
                    let half = column.half_bits(stain)?;
                    let values = column.value(stain)?;
                    for (half, value) in half.into_iter().zip(values) {
                        output.write_all(&u32::from(half).to_le_bytes())?;
                        output.write_all(&value.to_bits().to_le_bytes())?;
                    }
                }
            }
        }
    }
    output.flush()?;
    Ok(())
}

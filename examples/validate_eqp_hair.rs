//! Args: extracted EQP. Hair code fields and controlled first-byte cases.
use icexiv_data::eqp::{ENTRY_COUNT, Eqp, HeadFlags};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected EQP")?;
    if std::fs::metadata(&path)?.len() > 64 * 160 * 8 {
        return Err("input limit".into());
    }
    let bytes = std::fs::read(path)?;
    let eqp = Eqp::parse(&bytes)?;
    for id in 1..ENTRY_COUNT {
        if let Some(flags) = eqp.entry(id)? {
            let head = flags.head();
            println!("hair\t{id}\t{:06x}\t{}", head.0, head.hair_coverage_code());
        } else {
            println!("hair\t{id}\tabsent");
        }
    }
    for byte in 0..=255u32 {
        let h = HeadFlags(0x55aa00 | byte);
        println!(
            "control\t{byte}\t{}\t{}\t{}\t{}",
            h.hair_coverage_code(),
            u8::from(h.hide_scalp()),
            u8::from(h.hide_hair()),
            u8::from(h.show_hair_override())
        );
    }
    Ok(())
}

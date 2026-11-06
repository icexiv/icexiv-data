//! Args: extracted.eqp; every logical entry, preserving absent-block distinction.
use icexiv_data::eqp::{ENTRY_COUNT, Eqp};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected EQP path")?;
    if std::fs::metadata(&path)?.len() > 81920 {
        return Err("EQP exceeds 64 blocks".into());
    }
    let b = std::fs::read(path)?;
    let e = Eqp::parse(&b)?;
    println!(
        "eqp\t{}\t{:016x}\t{}",
        b.len(),
        e.block_bitmap(),
        e.stored_block_count()
    );
    for id in 1..ENTRY_COUNT {
        if let Some(f) = e.entry(id)? {
            println!(
                "entry\t{id}\t{:016x}\t{:04x},{:02x},{:02x},{:02x},{:06x}",
                f.0,
                f.body().0,
                f.legs().0,
                f.hands().0,
                f.feet().0,
                f.head().0
            );
        } else {
            println!("entry\t{id}\tabsent");
        }
    }
    Ok(())
}

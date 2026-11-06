//! Args: extracted.eqdp ...; emits every logical entry for independent checks.
use icexiv_data::eqdp::{AccessorySlot, Eqdp, EquipmentSlot};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = std::env::args_os().skip(1).peekable();
    let mode = if paths.peek().is_some_and(|p| p == "--accessory-slots") {
        paths.next();
        Some(true)
    } else if paths.peek().is_some_and(|p| p == "--equipment-slots") {
        paths.next();
        Some(false)
    } else {
        None
    };
    for path in paths {
        if std::fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
            return Err("input exceeds 16 MiB".into());
        }
        let b = std::fs::read(path)?;
        let q = Eqdp::parse(&b)?;
        // Validator output is separately bounded, even for a tiny sparse file.
        if q.entry_count() > 1_000_000 {
            return Err("validation output budget exceeded".into());
        }
        println!(
            "eqdp\t{}\t{}\t{}\t{}",
            b.len(),
            q.block_size(),
            q.block_count(),
            q.trailing_bytes().len()
        );
        for id in 0..q.entry_count() {
            let e = q.entry(id)?;
            let slots = match mode {
                Some(true) => Some(
                    [
                        AccessorySlot::Ears,
                        AccessorySlot::Neck,
                        AccessorySlot::Wrists,
                        AccessorySlot::RightRing,
                        AccessorySlot::LeftRing,
                    ]
                    .map(|s| e.accessory_slot(s).raw()),
                ),
                Some(false) => Some(
                    [
                        EquipmentSlot::Head,
                        EquipmentSlot::Body,
                        EquipmentSlot::Hands,
                        EquipmentSlot::Legs,
                        EquipmentSlot::Feet,
                    ]
                    .map(|s| e.equipment_slot(s).raw()),
                ),
                None => None,
            };
            if let Some(slots) = slots {
                println!(
                    "entry\t{id}\t{}\t{}\t{}",
                    e.raw,
                    u8::from(e.implicit_zero),
                    slots.map(|v| v.to_string()).join(",")
                );
            } else {
                println!("entry\t{id}\t{}\t{}", e.raw, u8::from(e.implicit_zero));
            }
        }
    }
    Ok(())
}

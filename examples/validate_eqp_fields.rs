//! Print declaration fields with static client-consumer evidence, not visibility.
use icexiv_data::eqp::{ENTRY_COUNT, Eqp};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("EQP path")?;
    if std::fs::metadata(&path)?.len() > 81920 {
        return Err("EQP size".into());
    }
    let b = std::fs::read(path)?;
    let table = Eqp::parse(&b)?;
    let ear_tail = std::env::args().skip(2).any(|arg| arg == "--ear-tail");
    for id in 1..ENTRY_COUNT {
        if let Some(f) = table.entry(id)? {
            println!(
                "fields\t{id}\t{:016x}\t{},{},{},{},{},{},{},{},{},{}",
                f.0,
                u8::from(f.body().uses_head_parameters()),
                u8::from(f.body().uses_hand_parameters()),
                u8::from(f.body().uses_leg_parameters()),
                u8::from(f.legs().uses_foot_parameters()),
                f.body().glove_culling_code(),
                f.legs().boot_culling_code(),
                f.hands().coverage_selector(),
                f.feet().coverage_selector(),
                u8::from(f.hands().coverage_comparison_bit()),
                u8::from(f.feet().coverage_comparison_bit())
            );
            if ear_tail {
                let fields = [
                    u8::from(f.body().enabled()),
                    u8::from(f.body().show_tail()),
                    u8::from(f.legs().enabled()),
                    u8::from(f.legs().show_tail()),
                    u8::from(f.head().enabled()),
                    f.head().hair_coverage_code(),
                    u8::from(f.head().show_human_ears()),
                    u8::from(f.head().show_miqote_ears()),
                    u8::from(f.head().show_aura_ears()),
                    u8::from(f.head().show_viera_ears()),
                    u8::from(f.head().show_hrothgar_hat()),
                    u8::from(f.head().show_viera_hat()),
                    u8::from(f.bit(59).ok_or("bit index")?),
                ];
                println!(
                    "ear_tail\t{id}\t{}",
                    fields.map(|v| v.to_string()).join(",")
                );
            }
        } else {
            println!("fields\t{id}\tabsent");
        }
    }
    Ok(())
}

//! Print declaration lane masks, not applied colors or native routing policy.
use icexiv_data::mtrl::{ColorLayout, DyeOverrideProfile, DyeRow, Mtrl};
fn emit(prefix: &str, dye: DyeRow) {
    for (name, profile) in [
        ("legacy", DyeOverrideProfile::Legacy),
        ("dawntrail", DyeOverrideProfile::Dawntrail),
    ] {
        println!(
            "{prefix}\t{name}\t{:08x}\t{:08x}",
            dye.raw,
            dye.canonical_override_lane_mask(profile)
        );
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for flags in 0..4096 {
        for extra in [0, 0xe000_f000, 0x1fff_0000] {
            emit(
                "control",
                DyeRow {
                    layout: ColorLayout::Dawntrail,
                    raw: flags | extra,
                },
            );
        }
    }
    for raw in 0..65536 {
        emit(
            "legacy_control",
            DyeRow {
                layout: ColorLayout::Legacy,
                raw,
            },
        );
    }
    for (file, path) in std::env::args_os().skip(1).enumerate() {
        if std::fs::metadata(&path)?.len() > 65535 {
            return Err("MTRL size".into());
        }
        let bytes = std::fs::read(path)?;
        let m = Mtrl::parse(&bytes)?;
        let table = m.color_table()?.ok_or("no color table")?;
        for row in 0..table.row_count() {
            emit(
                &format!("file\t{file}\t{row}"),
                table.dye_row(row)?.ok_or("no dye row")?,
            );
        }
    }
    Ok(())
}

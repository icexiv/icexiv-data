//! Args: extracted.pbd. Exhaustive u16 ID lookup, without consumer defaults.
use icexiv_data::pbd::Pbd;
fn show(p: Pbd<'_>, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("table\t{path}");
    for id in 0..=u16::MAX {
        match p.find_deformer(id)? {
            Some(d) => println!(
                "lookup\t{id}\t{:08x}\t{}\t{}\t{}",
                d.unknown_scale.to_bits(),
                u8::from(d.has_payload()),
                d.bone_count(),
                d.connection_index
            ),
            None => println!("lookup\t{id}\tabsent"),
        }
    }
    if let Some(e) = p.extension() {
        for i in 0..e.entry_count() {
            show(e.entry(i)?.pbd()?, &format!("{path}/{i}"))?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected PBD input")?;
    if std::fs::metadata(&path)?.len() > 64 << 20 {
        return Err("PBD size".into());
    }
    let b = std::fs::read(path)?;
    show(Pbd::parse(&b)?, "root")
}

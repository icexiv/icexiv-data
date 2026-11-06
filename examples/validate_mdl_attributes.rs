//! Resolve names to this MDL's attribute mask bits, without deciding visibility.
use icexiv_data::mdl::Mdl;
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn bit(v: Option<u32>) -> String {
    v.map_or_else(|| "none".into(), |v| format!("{v:08x}"))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("MDL path")?;
    if std::fs::metadata(&path)?.len() > 64 << 20 {
        return Err("MDL size".into());
    }
    let b = std::fs::read(path)?;
    let m = Mdl::parse(&b)?;
    for i in 0..m.attribute_count() {
        let name = m.attribute(i)?;
        println!(
            "name\t{i}\t{}\t{}",
            hex(name),
            bit(m.attribute_bit(name, 16 << 20)?)
        );
    }
    for name in [
        b"atr_nek".as_slice(),
        b"atr_hij",
        b"atr_ude",
        b"atr_sne",
        b"atr_hiz",
        b"atr_tlh",
        b"atr_tls",
        b"atr_inr",
        b"missing_attribute",
    ] {
        println!(
            "lookup\t{}\t{}",
            hex(name),
            bit(m.attribute_bit(name, 16 << 20)?)
        );
    }
    // Optional literal names add focused queries without changing the pinned
    // baseline output. These remain file-local lookups, not display policy.
    for name in std::env::args().skip(2) {
        println!(
            "lookup\t{}\t{}",
            hex(name.as_bytes()),
            bit(m.attribute_bit(name.as_bytes(), 16 << 20)?)
        );
    }
    for i in 0..m.submesh_count() {
        println!("submesh_mask\t{i}\t{:08x}", m.submesh(i)?.attribute_mask);
    }
    Ok(())
}

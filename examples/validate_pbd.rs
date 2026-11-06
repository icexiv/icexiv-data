//! Args: extracted.pbd. Prints every relation/name/matrix coefficient recursively.
use icexiv_data::pbd::Pbd;
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn show(pbd: Pbd<'_>, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("pbd\t{path}\t{}", pbd.deformer_count());
    for i in 0..pbd.deformer_count() {
        let d = pbd.deformer(i)?;
        println!(
            "deformer\t{i}\t{}\t{}\t{:08x}\t{}\t{}",
            d.skeleton_id,
            d.connection_index,
            d.unknown_scale.to_bits(),
            u8::from(d.has_payload()),
            d.bone_count()
        );
        for j in 0..d.bone_count() {
            let b = d.bone(j)?;
            let matrix = b
                .matrix
                .0
                .into_iter()
                .flatten()
                .map(|v| format!("{:08x}", v.to_bits()))
                .collect::<Vec<_>>()
                .join(",");
            println!("bone\t{j}\t{}\t{matrix}", hex(b.name));
            // Exercise matrix storage conversion without approximation/decomposition.
            let m = b.matrix.column_major_4x4();
            for row in 0..3 {
                for col in 0..4 {
                    if m[col * 4 + row].to_bits() != b.matrix.0[row][col].to_bits() {
                        return Err("matrix conversion differs".into());
                    }
                }
            }
        }
    }
    let index = |x: Option<usize>| x.map_or(-1, |v| v as i64);
    for i in 0..pbd.deformer_count() {
        let c = pbd.connection(i)?;
        println!(
            "connection\t{i}\t{}\t{}\t{}\t{}",
            index(c.parent),
            index(c.first_child),
            index(c.next_sibling),
            c.deformer_index
        );
    }
    if let Some(e) = pbd.extension() {
        println!("epbd\t{}", e.entry_count());
        for i in 0..e.entry_count() {
            let v = e.entry(i)?;
            println!(
                "extended\t{i}\t{}\t{}\t{}\t{}",
                v.child_race,
                v.parent_race,
                hex(v.raw_conditions()),
                v.pbd_bytes().len()
            );
            show(v.pbd()?, &format!("{path}/{i}"))?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected PBD input")?;
    if std::fs::metadata(&path)?.len() > 64 * 1024 * 1024 {
        return Err("sample too large".into());
    }
    let b = std::fs::read(path)?;
    show(Pbd::parse(&b)?, "root")
}

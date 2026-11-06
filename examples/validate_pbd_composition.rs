//! Args: extracted.pbd. Generic affine products, not runtime PBD chain selection.
use icexiv_data::pbd::Pbd;
fn show(p: Pbd<'_>, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..p.deformer_count() {
        let d = p.deformer(i)?;
        for j in 0..d.bone_count() {
            let a = d.bone(j)?.matrix;
            let b = d.bone((j + 1) % d.bone_count())?.matrix;
            let ab = a.compose(b);
            let ba = b.compose(a);
            let values =
                ab.0.into_iter()
                    .flatten()
                    .chain(ba.0.into_iter().flatten())
                    .chain(
                        [[0., 0., 0.], [1., -2., 3.], [-4., 5., 0.5]]
                            .into_iter()
                            .flat_map(|p| ab.transform_point(p)),
                    )
                    .map(|v| format!("{v:.9e}"))
                    .collect::<Vec<_>>()
                    .join(",");
            println!("product\t{path}\t{i}\t{j}\t{values}");
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

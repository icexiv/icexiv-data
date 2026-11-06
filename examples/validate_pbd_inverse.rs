//! Fixed-size affine inverse on file coefficients; no skeleton or race routing.
use icexiv_data::pbd::Pbd;
fn show(p: Pbd<'_>, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..p.deformer_count() {
        let d = p.deformer(i)?;
        for j in 0..d.bone_count() {
            match d.bone(j)?.matrix.try_inverse() {
                Ok(inverse) => println!(
                    "inverse\t{path}\t{i}\t{j}\t{}",
                    inverse
                        .0
                        .into_iter()
                        .flatten()
                        .map(|v| format!("{v:.9e}"))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Err(error) => println!("inverse_error\t{path}\t{i}\t{j}\t{error:?}"),
            }
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
    let path = std::env::args_os().nth(1).ok_or("PBD path")?;
    if std::fs::metadata(&path)?.len() > 64 << 20 {
        return Err("PBD size".into());
    }
    let bytes = std::fs::read(path)?;
    show(Pbd::parse(&bytes)?, "root")
}

//! Args: extracted.pbd. All ordered deformer-index pairs, not race assembly.
use icexiv_data::pbd::Pbd;
fn show(p: Pbd<'_>, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let n = p.deformer_count();
    if n > 128 {
        return Err("path evidence table exceeds 128 deformers".into());
    }
    println!("table\t{path}\t{n}");
    for descendant in 0..n {
        for ancestor in 0..n {
            let value = match p.ancestor_path(descendant, ancestor, 1024)? {
                None => "unrelated".into(),
                Some(v) => {
                    let indices = v.collect::<Result<Vec<_>, _>>()?;
                    if indices.is_empty() {
                        "empty".into()
                    } else {
                        indices
                            .iter()
                            .map(usize::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    }
                }
            };
            println!("path\t{descendant}\t{ancestor}\t{value}");
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

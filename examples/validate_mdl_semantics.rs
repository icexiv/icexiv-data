//! Args: extracted/control MDL [--all-meshes]; query by semantic identity.
use icexiv_data::mdl::{Error, Mdl, MeshRole};
fn status(e: Error) -> Result<&'static str, Error> {
    match e {
        Error::InvalidData("ambiguous vertex semantic") => Ok("ambiguous"),
        Error::InvalidData("incomplete skin influence pair") => Ok("incomplete"),
        Error::OutOfBounds("vertex") => Ok("vertex"),
        other => Err(other),
    }
}
fn bits(values: [f32; 8]) -> String {
    values.map(|v| format!("{:08x}", v.to_bits())).join(",")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected MDL")?;
    if std::fs::metadata(&path)?.len() > 64 << 20 {
        return Err("input limit".into());
    }
    let bytes = std::fs::read(path)?;
    let model = Mdl::parse(&bytes)?;
    if std::env::args_os().nth(2).as_deref() == Some(std::ffi::OsStr::new("--all-meshes")) {
        for lod in 0..usize::from(model.header().lod_count) {
            let mut meshes = Vec::new();
            for range in model.lod(lod)?.ranges {
                if range.role != MeshRole::TerrainShadow {
                    meshes.extend(
                        usize::from(range.start)
                            ..usize::from(range.start) + usize::from(range.count),
                    );
                }
            }
            meshes.sort_unstable();
            meshes.dedup();
            for mesh in meshes {
                println!("mesh\t{lod}\t{mesh}");
                inspect(&model, lod, mesh, true)?;
            }
        }
    } else {
        inspect(&model, 0, 0, false)?;
    }
    Ok(())
}
fn inspect(
    model: &Mdl<'_>,
    lod: usize,
    mesh: usize,
    include_colors: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let g = model.geometry(lod, mesh)?;
    let mut queries: Vec<_> = g.declaration()?.map(|e| (e.usage, e.usage_index)).collect();
    queries.push((255, 255));
    if include_colors {
        queries.extend([(7, 0), (7, 1)]);
    }
    queries.sort_unstable();
    queries.dedup();
    let count = usize::from(model.mesh_metadata(mesh)?.vertex_count);
    for v in 0..=count {
        for &(usage, index) in &queries {
            let result = match g.attribute(v, usage, index) {
                Ok(Some(value)) => bits(value),
                Ok(None) => "absent".into(),
                Err(e) => status(e)?.into(),
            };
            println!("attribute\t{v}\t{usage}\t{index}\t{result}");
        }
        let result = match g.skin_influences(v) {
            Ok(Some(s)) => format!(
                "{}\t{}\t{}",
                s.count,
                bits(s.weights),
                s.indices.map(|v| v.to_string()).join(",")
            ),
            Ok(None) => "absent".into(),
            Err(e) => status(e)?.into(),
        };
        println!("skin\t{v}\t{result}");
    }
    Ok(())
}

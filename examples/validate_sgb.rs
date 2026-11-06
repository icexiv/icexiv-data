//! Full structural furniture pointer observations. Args: extracted.sgb ...
use icexiv_data::sgb::{Asset, Sgb};
fn hex(v: &[u8]) -> String {
    v.iter().map(|v| format!("{v:02x}")).collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args_os().skip(1) {
        if std::fs::metadata(&path)?.len() > 64 << 20 {
            return Err("sample size".into());
        }
        let bytes = std::fs::read(path)?;
        let f = Sgb::parse(&bytes)?;
        println!(
            "sgb\t{}\t{}\t{}\t{}\t{}",
            bytes.len(),
            f.group.offset,
            f.group.id,
            hex(f.group.name),
            f.group.layers.len()
        );
        for l in &f.group.layers {
            println!(
                "layer\t{}\t{}\t{}\t{}",
                l.offset,
                l.id,
                hex(l.name),
                l.instances.len()
            );
            for i in &l.instances {
                let typ = match i.asset {
                    Asset::Background { .. } => 1,
                    Asset::SharedGroup { .. } => 6,
                    Asset::Other(v) => v,
                };
                let trs = i
                    .transform
                    .translation
                    .into_iter()
                    .chain(i.transform.rotation)
                    .chain(i.transform.scale)
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "instance\t{}\t{}\t{}\t{}\t{trs}",
                    i.offset,
                    typ,
                    i.id,
                    hex(i.name)
                );
                match i.asset {
                    Asset::Background {
                        model_path,
                        collision_path,
                    } => {
                        println!("path\t{}\tmodel\t{}", i.offset, hex(model_path));
                        println!("path\t{}\tcollision\t{}", i.offset, hex(collision_path));
                    }
                    Asset::SharedGroup { path } => {
                        println!("path\t{}\tshared_group\t{}", i.offset, hex(path))
                    }
                    Asset::Other(_) => (),
                }
            }
        }
        for d in f.dependencies() {
            println!(
                "dependency\t{:?}\t{}\t{}\t{}\t{}\t{}",
                d.kind,
                d.layer_index,
                d.instance_index,
                d.instance_id,
                d.instance_offset,
                hex(d.path)
            );
        }
    }
    Ok(())
}

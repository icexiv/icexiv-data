//! Typed AVFX Data and raw reference-range evidence; no simulation or I/O graph.
use icexiv_data::avfx::{AssetLimits, Avfx, DataView, Kind, ParameterValue};
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn opt(v: Option<i32>) -> String {
    v.map_or_else(|| "none".into(), |v| v.to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args_os().skip(1) {
        if std::fs::metadata(&path)?.len() > 64 << 20 {
            return Err("input size".into());
        }
        let raw = std::fs::read(path)?;
        let a = Avfx::parse(&raw)?;
        for n in &a.nodes {
            match n.data_view(AssetLimits::default())? {
                DataView::Absent => println!("data\t{}\tabsent", n.block.offset),
                DataView::Unknown { variant, .. } => {
                    println!("data\t{}\tunknown\t{}", n.block.offset, opt(variant))
                }
                DataView::Known {
                    kind, parameters, ..
                } => {
                    println!("data\t{}\t{:?}\t{}", n.block.offset, kind, parameters.len());
                    for p in parameters {
                        let value = match p.value {
                            ParameterValue::Integer(v) => format!("int\t{v}"),
                            ParameterValue::Byte(v) => format!("byte\t{v}"),
                            ParameterValue::Real(v) => format!("float\t{:08x}", v.to_bits()),
                            ParameterValue::Bytes(v) => format!("bytes\t{}", hex(v)),
                            ParameterValue::Curve(c) => format!("curve\t{}", c.keys().len()),
                            ParameterValue::Group(f) => format!("group\t{}", f.len()),
                            ParameterValue::Unknown(b) => format!("unknown\t{}", hex(b.payload)),
                        };
                        println!(
                            "parameter\t{}\t{}\t{}\t{value}",
                            n.block.offset,
                            p.block.offset,
                            hex(&p.block.name)
                        );
                    }
                }
            }
            if n.kind == Kind::Emitter {
                for (i, item) in n.emitter_items(AssetLimits::default())?.iter().enumerate() {
                    println!(
                        "emitter_item\t{}\t{i}\t{:?}\t{}\t{}\t{}\t{}\t{}",
                        n.block.offset,
                        item.target_kind,
                        opt(item.enabled),
                        opt(item.target_index),
                        opt(item.create_time),
                        opt(item.create_count),
                        opt(item.create_probability)
                    );
                }
            }
            if n.kind == Kind::Particle {
                for slot in n.texture_slots(AssetLimits::default())? {
                    println!(
                        "texture_slot\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        n.block.offset,
                        slot.block.offset,
                        hex(&slot.block.name),
                        opt(slot.enabled),
                        opt(slot.texture_index),
                        opt(slot.uv_set),
                        slot.mask_indices.map_or_else(|| "none".into(), hex),
                        slot.texture_curve
                            .map_or_else(|| "none".into(), |v| v.keys().len().to_string())
                    );
                }
            }
        }
        let report = a.asset_report(AssetLimits::default())?;
        println!(
            "report\t{}\t{}\t{}",
            report.references.len(),
            report.animated_texture_curves,
            report.unknown_data_nodes
        );
        for r in report.references {
            println!(
                "reference\t{}\t{}\t{}\t{:?}\t{}\t{:?}",
                r.owner_offset, r.field_offset, r.element, r.kind, r.raw_index, r.status
            );
        }
    }
    Ok(())
}

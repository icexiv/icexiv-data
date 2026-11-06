//! Full borrowed block, geometry and curve evidence. Args: extracted.avfx ...
use icexiv_data::avfx::{Avfx, Clip, Curve, Fields, Kind};
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn bits(v: impl IntoIterator<Item = f32>) -> String {
    v.into_iter()
        .map(|v| format!("{:08x}", v.to_bits()))
        .collect::<Vec<_>>()
        .join(",")
}
fn nested(name: [u8; 4]) -> bool {
    let start = name.iter().position(|v| *v != 0).unwrap_or(4);
    let end = name.iter().rposition(|v| *v != 0).map_or(start, |i| i + 1);
    let name = &name[start..end];
    [
        b"A" as &[u8],
        b"ARs",
        b"Ang",
        b"Att",
        b"Bri",
        b"CEI",
        b"CEO",
        b"Col",
        b"ColB",
        b"ColE",
        b"CrC",
        b"CrI",
        b"CrIR",
        b"DPow",
        b"Data",
        b"DstS",
        b"FrC",
        b"FrRt",
        b"Gra",
        b"IjS",
        b"IjSR",
        b"ItEm",
        b"ItPr",
        b"Item",
        b"Len",
        b"Life",
        b"NPow",
        b"ORad",
        b"POff",
        b"Pos",
        b"PrpS",
        b"Prp1",
        b"Prp2",
        b"PrpG",
        b"RB",
        b"RE",
        b"RGB",
        b"Rad",
        b"Rads",
        b"RdI",
        b"RdO",
        b"Rot",
        b"RotR",
        b"Scl",
        b"SclA",
        b"SclB",
        b"SclG",
        b"SclR",
        b"Scr",
        b"Smpl",
        b"SpS",
        b"Str",
        b"TC1",
        b"TC2",
        b"TC3",
        b"TC4",
        b"TD",
        b"TN",
        b"TP",
        b"TR",
        b"Trgr",
        b"TxN",
        b"UvSt",
        b"WB",
        b"WE",
        b"X",
        b"XR",
        b"Y",
        b"YR",
        b"Z",
        b"ZR",
    ]
    .contains(&name)
}
fn walk(
    fields: Fields<'_>,
    depth: usize,
    work: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if depth > 32 {
        return Err("observation depth".into());
    }
    for b in fields.iter() {
        *work = work.checked_sub(1).ok_or("observation budget")?;
        println!(
            "block\t{}\t{}\t{}\t{}",
            b.offset,
            hex(&b.name),
            b.payload.len(),
            hex(b.padding)
        );
        if b.is(b"Clip") {
            let c = Clip::parse(b)?;
            println!(
                "clip\t{}\t{}\t{}\t{}\t{}",
                b.offset,
                hex(&c.tag),
                c.integers.map(|v| v.to_string()).join(","),
                bits(c.reals),
                c.names.map(hex).join(",")
            );
        }
        let scalar_variant = b.payload.len() == 4
            && [
                b"Life" as &[u8],
                b"CrI",
                b"CrIR",
                b"Att",
                b"Gra",
                b"ORad",
                b"RdI",
                b"RdO",
                b"Str",
            ]
            .iter()
            .any(|name| b.is(name));
        if nested(b.name) && !scalar_variant {
            let f = b.fields(1_000_000)?;
            if f.one(b"Keys")?.is_some() {
                let c = Curve::parse(b, 1_000_000)?;
                for k in c.keys() {
                    println!(
                        "key\t{}\t{}\t{}\t{}",
                        b.offset,
                        k.time,
                        k.kind,
                        bits(k.values)
                    );
                }
            }
            walk(f, depth + 1, work)?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args_os().skip(1) {
        if std::fs::metadata(&path)?.len() > 64 << 20 {
            return Err("input size".into());
        }
        let bytes = std::fs::read(path)?;
        let a = Avfx::parse(&bytes)?;
        println!(
            "avfx\t{}\t{}\t{}\t{}",
            bytes.len(),
            a.nodes.len(),
            a.textures.len(),
            a.models.len()
        );
        for (i, t) in a.textures.iter().enumerate() {
            println!("texture\t{i}\t{}", hex(t));
        }
        for n in &a.nodes {
            println!(
                "node\t{}\t{}\t{}",
                n.block.offset,
                hex(&n.block.name),
                n.variant()?
                    .map_or_else(|| "none".into(), |v| v.to_string())
            );
            walk(n.fields, 0, &mut 1_000_000)?;
            if n.kind == Kind::Scheduler {
                for trigger in [false, true] {
                    for (i, s) in n.scheduler_items(trigger, 1_000_000)?.iter().enumerate() {
                        println!(
                            "schedule\t{}\t{}\t{i}\t{}\t{}\t{}",
                            n.block.offset,
                            u8::from(trigger),
                            s.enabled,
                            s.start_time,
                            s.timeline_index
                        );
                    }
                }
            }
            if n.kind == Kind::Timeline {
                for (i, s) in n.timeline_items(1_000_000)?.iter().enumerate() {
                    println!(
                        "timeline_item\t{}\t{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                        n.block.offset,
                        s.enabled,
                        s.start_time,
                        s.end_time,
                        s.binder_index,
                        s.effector_index,
                        s.emitter_index,
                        s.platform,
                        s.clip_index
                            .map_or_else(|| "none".into(), |v| v.to_string())
                    );
                }
            }
        }
        for (i, m) in a.models.iter().enumerate() {
            println!(
                "model\t{i}\t{}\t{}\t{}",
                m.draw_vertices().len(),
                m.indices().len(),
                m.emit_vertices().len()
            );
            for v in m.draw_vertices() {
                println!(
                    "draw\t{i}\t{}\t{}\t{}\t{}\t{}",
                    bits(v.position),
                    v.normal.map(|v| v.to_string()).join(","),
                    v.tangent.map(|v| v.to_string()).join(","),
                    hex(&v.color),
                    bits(v.uv.into_iter().flatten())
                );
            }
            println!(
                "indices\t{i}\t{}",
                m.indices()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for (v, num) in m.emit_vertices().zip(m.emit_numbers()) {
                println!(
                    "emit\t{i}\t{}\t{}\t{}\t{num}",
                    bits(v.position),
                    bits(v.normal),
                    hex(&v.color)
                );
            }
        }
    }
    Ok(())
}

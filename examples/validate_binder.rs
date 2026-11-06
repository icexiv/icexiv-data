//! Binder asset fields and optional static MDL/SKLB candidate matches.
//! Args: extracted.avfx [owner.mdl owner.sklb]. No transforms are composed.
use icexiv_data::{
    avfx::{AssetLimits, Avfx, Kind},
    mdl::Mdl,
    sklb::Sklb,
};
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn opt(v: Option<i32>) -> String {
    v.map_or_else(|| "none".into(), |v| v.to_string())
}
fn scalar(v: Option<f32>) -> String {
    v.map_or_else(|| "none".into(), |v| format!("{:08x}", v.to_bits()))
}
fn load(p: &std::ffi::OsStr) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if std::fs::metadata(p)?.len() > 64 << 20 {
        return Err("input size".into());
    }
    Ok(std::fs::read(p)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !matches!(args.len(), 1 | 3) {
        return Err("avfx [mdl sklb]".into());
    }
    let raw = load(&args[0])?;
    let avfx = Avfx::parse(&raw)?;
    let model_bytes = if args.len() == 3 {
        Some(load(&args[1])?)
    } else {
        None
    };
    let skeleton_bytes = if args.len() == 3 {
        Some(load(&args[2])?)
    } else {
        None
    };
    let model = model_bytes.as_deref().map(Mdl::parse).transpose()?;
    let skeleton = skeleton_bytes.as_deref().map(Sklb::parse).transpose()?;
    let poses = skeleton
        .as_ref()
        .map(|s| s.skeletons(16 << 20))
        .transpose()?;
    let selected = skeleton
        .as_ref()
        .map(|s| s.tagfile.animation_skeleton_ids(1 << 20))
        .transpose()?;
    for n in avfx.nodes_of(Kind::Binder) {
        for p in n.binder_properties(AssetLimits::default())? {
            println!(
                "binder\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                n.block.offset,
                p.block.offset,
                p.slot,
                opt(p.bind_point),
                opt(p.target_point),
                opt(p.bind_point_id),
                p.name.map_or_else(|| "none".into(), hex),
                opt(p.generate_delay),
                opt(p.coordinate_update_frame),
                opt(p.coordinate_type),
                opt(p.ring.enabled),
                opt(p.ring.progress_time),
                scalar(p.ring.position[0]),
                scalar(p.ring.position[1]),
                scalar(p.ring.position[2]),
                scalar(p.ring.radius),
                p.position
                    .map_or_else(|| "none".into(), |f| f.len().to_string())
            );
            if let (Some(m), Some(poses), Some(selected), Some(id)) = (
                model.as_ref(),
                poses.as_ref(),
                selected.as_ref(),
                p.bind_point_id,
            ) {
                let mut matches = 0;
                let mut bone_matches = 0;
                if let Ok(id) = u32::try_from(id) {
                    for i in 0..m.element_id_count() {
                        let e = m.element_id(i)?;
                        if e.id != id {
                            continue;
                        }
                        matches += 1;
                        let name = m.string(e.parent_bone_string_offset)?;
                        let transform = e
                            .translation
                            .into_iter()
                            .chain(e.rotation)
                            .map(|v| format!("{:08x}", v.to_bits()))
                            .collect::<Vec<_>>()
                            .join(",");
                        println!(
                            "element_candidate\t{}\t{i}\t{id}\t{}\t{transform}",
                            p.block.offset,
                            hex(name)
                        );
                        for s in poses {
                            if !selected.contains(&s.object_id) {
                                continue;
                            }
                            for (j, bone) in s.bones.iter().enumerate() {
                                if bone.name == name {
                                    bone_matches += 1;
                                    println!(
                                        "bone_candidate\t{}\t{}\t{j}\t{}",
                                        p.block.offset,
                                        s.object_id,
                                        hex(bone.name)
                                    );
                                }
                            }
                        }
                    }
                }
                println!(
                    "candidate_counts\t{}\t{matches}\t{bone_matches}",
                    p.block.offset
                );
            }
        }
    }
    Ok(())
}

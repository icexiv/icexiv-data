//! Evaluate the ByName/BPID Start used by the saw, for an explicitly supplied
//! owner-world frame and a weapon in reference pose. Arguments: avfx mdl sklb
//! followed by 12 floats (X/Y/Z axis vectors, then origin, comma-separated).
use icexiv_data::{
    avfx::{AssetLimits, Avfx, BinderSlot, Kind},
    mdl::Mdl,
    pbd::Matrix,
    sklb::Sklb,
};
use std::{error::Error, io::Read};

fn load(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((64 << 20) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 << 20 {
        return Err("asset exceeds 64 MiB".into());
    }
    Ok(bytes)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [avfx, model, sklb, input] = args.as_slice() else {
        return Err("AVFX MDL SKLB x,y,z,... (12 owner frame floats)".into());
    };
    let axes = input
        .split(',')
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()?;
    if axes.len() != 12 || !axes.iter().all(|x| x.is_finite()) {
        return Err("invalid owner frame".into());
    }
    let owner = Matrix(std::array::from_fn(|r| {
        std::array::from_fn(|c| axes[c * 3 + r])
    }));
    let b = load(avfx)?;
    let m = load(model)?;
    let s = load(sklb)?;
    let asset = Avfx::parse(&b)?;
    let model = Mdl::parse(&m)?;
    let sk = Sklb::parse(&s)?;
    let ids = sk.tagfile.animation_skeleton_ids(1 << 20)?;
    let [id] = ids.as_slice() else {
        return Err("select one owner skeleton".into());
    };
    let skeleton = sk
        .skeletons(1 << 24)?
        .into_iter()
        .find(|s| s.object_id == *id)
        .ok_or("missing skeleton")?;
    let rest = vec![None; skeleton.bones.len()];
    for binder in asset.nodes_of(Kind::Binder) {
        for p in binder.binder_properties(AssetLimits::default())? {
            if p.slot != BinderSlot::Start || p.target_point != Some(3) || p.bind_point != Some(0) {
                continue;
            }
            let id = u32::try_from(p.bind_point_id.ok_or("missing BPID")?)?;
            let e = model.element_by_id(id, 65536)?.ok_or("no owner element")?;
            if e.rotation != [0.; 3] {
                return Err("example requires zero ElementId rotation".into());
            }
            let name = model.string(e.parent_bone_string_offset)?;
            let bone = skeleton
                .bone_by_name(name, 1 << 20)?
                .ok_or("no owner bone")?;
            let frame = skeleton.posed_frame(bone, e.translation, &rest, 65536)?;
            let world = owner.compose(Matrix(std::array::from_fn(|r| {
                std::array::from_fn(|c| frame[c][r])
            })));
            let vector = |c: usize| std::array::from_fn(|r| world.0[r][c]);
            let f = binder
                .binder_transform_flags()?
                .host_frame([vector(0), vector(1), vector(2)], vector(3))?;
            println!(
                "{id}\t{bone}\t{}\t{:?}\t{:?}\t{:?}",
                String::from_utf8_lossy(name),
                f.axes,
                f.translation,
                f.scale
            );
        }
    }
    Ok(())
}

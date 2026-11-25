//! Sample a C010 animation name against explicitly selected PAP and SKLB
//! assets. Args: action.tmb clip.pap host.sklb bone-name milliseconds.
use icexiv_data::{
    havok::SplineLimits,
    pap::Pap,
    sklb::Sklb,
    tmb::{ResourceKind, Timeline, TimelineLimits},
};
use std::{error::Error, io::Read};

fn load(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut b = Vec::new();
    std::fs::File::open(path)?
        .take((64 << 20) + 1)
        .read_to_end(&mut b)?;
    if b.len() > 64 << 20 {
        return Err("asset exceeds 64 MiB".into());
    }
    Ok(b)
}

fn unique<T>(mut iter: impl Iterator<Item = T>) -> Result<T, Box<dyn Error>> {
    let first = iter.next().ok_or("no matching resource")?;
    if iter.next().is_some() {
        return Err("ambiguous resource selection".into());
    }
    Ok(first)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [tmb, pap, sklb, name, time] = args.as_slice() else {
        return Err("action.tmb clip.pap host.sklb bone-name milliseconds".into());
    };
    let tb = load(tmb)?;
    let pb = load(pap)?;
    let sb = load(sklb)?;
    let t = unique(Timeline::parse_sequence(&tb, TimelineLimits::default())?.into_iter())?;
    let mut clip = None;
    for link in t.command_links(1 << 20, 100000)? {
        if let Some(r) = t.command_resource(link.command)?
            && r.kind == ResourceKind::Animation
            && clip.replace(r.name).is_some()
        {
            return Err("expected one linked clip".into());
        }
    }
    let name_key = clip.ok_or("no linked animation")?;
    let p = Pap::parse(&pb)?;
    let record = unique(p.records().filter(|r| r.name == name_key))?;
    let binding = p.binding(record.havok_index)?;
    if binding.blend_hint != 0 {
        return Err("example requires a normal clip".into());
    }
    let anim = p
        .tagfile
        .as_ref()
        .ok_or("no animation stream")?
        .spline_animation(binding.animation_object_id, SplineLimits::default())?;
    let tracks = anim.sample(time.parse()?, 1 << 20, 1_000_000)?;
    let sk = Sklb::parse(&sb)?;
    let id = unique(sk.tagfile.animation_skeleton_ids(1 << 20)?.into_iter())?;
    let skeleton = unique(
        sk.skeletons(4 << 20)?
            .into_iter()
            .filter(|s| s.object_id == id),
    )?;
    let bone = unique(
        skeleton
            .bones
            .iter()
            .enumerate()
            .filter(|(_, b)| b.name == name.as_bytes()),
    )?
    .0;
    let samples = binding.mapped_samples(&tracks, skeleton.bones.len(), 1 << 20)?;
    let frame = skeleton.posed_frame(bone, [0.; 3], &samples, 1024)?;
    println!(
        "{}\t{}\t{}\t{}\t{:?}",
        String::from_utf8_lossy(name_key),
        record.havok_index,
        bone,
        time,
        frame
    );
    Ok(())
}

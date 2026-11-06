//! Args: extracted.sklb ...; all local rest-pose lanes and original graph IDs.
use icexiv_data::sklb::Sklb;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args_os().skip(1) {
        if std::fs::metadata(&path)?.len() > 64 * 1024 * 1024 {
            return Err("sample too large".into());
        }
        let b = std::fs::read(path)?;
        let sklb = Sklb::parse(&b)?;
        let skeletons = sklb.skeletons(32 * 1024 * 1024)?;
        println!(
            "sklb\t{}\t{}\t{}\t{}\t{}\t{}",
            b.len(),
            sklb.tagfile_offset,
            sklb.auxiliary.len(),
            sklb.tagfile.classes.len(),
            sklb.tagfile.objects.len(),
            skeletons.len()
        );
        println!(
            "animation_skeletons\t{}",
            sklb.tagfile
                .animation_skeleton_ids(1024 * 1024)?
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        for skeleton in skeletons {
            println!(
                "skeleton\t{}\t{}\t{}",
                skeleton.object_id,
                skeleton.name.map_or_else(|| "null".into(), hex),
                skeleton.bones.len()
            );
            for (i, bone) in skeleton.bones.iter().enumerate() {
                let lanes = bone
                    .rest
                    .translation
                    .into_iter()
                    .chain(bone.rest.rotation)
                    .chain(bone.rest.scale)
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "bone\t{i}\t{}\t{}\t{}\t{lanes}",
                    hex(bone.name),
                    bone.parent.map_or(-1, |p| p as i64),
                    u8::from(bone.lock_translation)
                );
            }
        }
    }
    Ok(())
}

//! Print all track samples for independent Python comparison.
//! Args: PAP [explicit-array-length-budget, at most 4 Mi elements] [--block-boundaries].
use icexiv_data::{havok::SplineLimits, pap::Pap};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("PAP input")?;
    let mut limits = icexiv_data::havok::Limits::default();
    if let Some(value) = args.next() {
        limits.max_array_length = value.to_str().ok_or("array budget encoding")?.parse()?;
        if limits.max_array_length > 4 << 20 {
            return Err("array budget ceiling".into());
        }
    }
    let boundaries = match args.next() {
        None => false,
        Some(v) if v == "--block-boundaries" => true,
        _ => return Err("PAP [array budget] [--block-boundaries]".into()),
    };
    if args.next().is_some() {
        return Err("extra argument".into());
    }
    if std::fs::metadata(&path)?.len() > 64 << 20 {
        return Err("input size".into());
    }
    let data = std::fs::read(path)?;
    let pap = Pap::parse_with_limits(&data, limits)?;
    let Some(tagfile) = pap.tagfile.as_ref() else {
        println!(
            "empty\t{}\t{}",
            pap.records().len(),
            pap.havok_padding.len()
        );
        return Ok(());
    };
    let mut samples_left = 1_000_000usize;
    for record in pap.records() {
        let binding = pap.binding(record.havok_index)?;
        let a = tagfile.spline_animation(binding.animation_object_id, SplineLimits::default())?;
        println!(
            "animation\t{}\t{}\t{}\t{}\t{}",
            record.havok_index,
            binding.object_id,
            a.object_id,
            a.num_frames,
            a.blocks.len()
        );
        println!(
            "binding\t{}\t{}\t{}\t{}\t{}",
            a.object_id,
            record
                .name
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            binding.blend_hint,
            binding
                .transform_track_to_bone
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(","),
            binding
                .original_skeleton_name
                .unwrap_or(&[])
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        for (block, b) in a.blocks.iter().enumerate() {
            for frame in [0., 0.125, 0.5, 1., 3.25, 8.5, 12., 17., 29., 70.] {
                samples_left = samples_left
                    .checked_sub(b.track_count())
                    .ok_or("validation sample output budget")?;
                for (track, s) in b.sample(frame, 16 << 20, 16_000_000)?.iter().enumerate() {
                    let v = s
                        .translation
                        .into_iter()
                        .chain(s.rotation)
                        .chain(s.scale)
                        .map(|v| format!("{v:.9e}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    println!("sample\t{}\t{block}\t{frame}\t{track}\t{v}", a.object_id);
                }
            }
        }
        let mut times = vec![-1., 0., 125., 500., 566.6666, 566.6667, 700., 1000., 2500.];
        if boundaries {
            for block in 1..a.blocks.len() {
                let time = (block * (a.max_frames_per_block - 1)) as f64
                    * f64::from(a.frame_duration_seconds)
                    * 1000.;
                for value in [time - 0.0001, time, time + 0.0001] {
                    if !times.contains(&value) {
                        times.push(value);
                    }
                }
            }
            let end = f64::from(a.duration_seconds) * 1000.;
            for value in [end - 0.0001, end, end + 1.] {
                if !times.contains(&value) {
                    times.push(value);
                }
            }
        }
        for time in times {
            samples_left = samples_left
                .checked_sub(a.blocks.first().ok_or("no spline blocks")?.track_count())
                .ok_or("validation sample output budget")?;
            for (track, s) in a.sample(time, 16 << 20, 16_000_000)?.iter().enumerate() {
                let v = s
                    .translation
                    .into_iter()
                    .chain(s.rotation)
                    .chain(s.scale)
                    .map(|v| format!("{v:.9e}"))
                    .collect::<Vec<_>>()
                    .join(",");
                println!("time\t{}\t{time}\t{track}\t{v}", a.object_id);
            }
        }
    }
    Ok(())
}

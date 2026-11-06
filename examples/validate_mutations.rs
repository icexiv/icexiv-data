//! Deterministic CPU-only mutation safety checks on caller-supplied snapshots.
//! Args: manifest.tsv new-output-directory. Rows: kind, input, auxiliary or '-'.
//! Snapshots must be <=16 MiB each, <=64 MiB total, at most 64 entries.
//! Accepted bytes are not claimed semantically valid. Unwinding panics fail
//! and preserve the exact mutated input outside the source snapshots. Aborts
//! and abnormal process exits are also failures, without a snapshot guarantee.
use icexiv_data as data;
use std::{io::Write, path::Path};

fn exercise(kind: &str, bytes: &[u8], auxiliary: &[u8]) -> u8 {
    // Bit 0: parsing accepted. Bit 1: the selected bounded follow-up accepted.
    match kind {
        "tex" => match data::tex::Tex::parse(bytes) {
            Ok(t) => {
                let queried = t
                    .mip(0)
                    .and_then(|m| m.image_by_index(0))
                    .map(|surface| {
                        let rgba8 = surface.decode_rgba8(1 << 20);
                        let _float = surface.decode_rgba32f(1 << 20);
                        rgba8.is_ok()
                    })
                    .unwrap_or(false);
                1 | (u8::from(queried) << 1)
            }
            Err(_) => 0,
        },
        "sklb" => match data::sklb::Sklb::parse_with_limits(bytes, graph_limits()) {
            Ok(s) => 1 | (u8::from(s.skeletons(1 << 20).is_ok()) << 1),
            Err(_) => 0,
        },
        "pap" => match data::pap::Pap::parse_with_limits(bytes, graph_limits()) {
            Ok(p) => {
                let sampled = (|| {
                    let record = p.records().next()?;
                    let binding = p.binding(record.havok_index).ok()?;
                    let animation = p
                        .tagfile
                        .as_ref()?
                        .spline_animation(
                            binding.animation_object_id,
                            data::havok::SplineLimits {
                                max_allocation_bytes: 8 << 20,
                                max_tracks: 4096,
                                max_blocks: 1024,
                                max_control_points: 100_000,
                                max_work: 1_000_000,
                            },
                        )
                        .ok()?;
                    animation.sample(125., 1 << 20, 100_000).ok()
                })();
                1 | (u8::from(sampled.is_some()) << 1)
            }
            Err(_) => 0,
        },
        "mdl" => u8::from(data::mdl::Mdl::parse(bytes).is_ok()),
        "mtrl" => match data::mtrl::Mtrl::parse(bytes) {
            Ok(m) => 1 | (u8::from(m.color_table().is_ok()) << 1),
            Err(_) => 0,
        },
        "shpk" => u8::from(data::shpk::Shpk::parse(bytes).is_ok()),
        "pbd" => u8::from(data::pbd::Pbd::parse(bytes).is_ok()),
        "stm" => u8::from(data::stm::Stm::parse(bytes).is_ok()),
        "cmp" => u8::from(data::cmp::Cmp::parse(bytes).is_ok()),
        "eqp" => u8::from(data::eqp::Eqp::parse(bytes).is_ok()),
        "eqdp" => u8::from(data::eqdp::Eqdp::parse(bytes).is_ok()),
        "imc" => u8::from(data::imc::Imc::parse(bytes).is_ok()),
        "sgb" => u8::from(data::sgb::Sgb::parse(bytes).is_ok()),
        "avfx" => u8::from(data::avfx::Avfx::parse(bytes).is_ok()),
        "exh" => u8::from(data::excel::Exh::parse(bytes).is_ok()),
        "exd" => match data::excel::Exh::parse(auxiliary) {
            Ok(header) => u8::from(data::excel::Exd::parse(bytes, &header).is_ok()),
            Err(_) => 0,
        },
        "index2" => u8::from(
            data::sqpack::index::Index::parse(bytes, data::sqpack::index::IndexKind::Index2)
                .is_ok(),
        ),
        _ => 0,
    }
}

fn graph_limits() -> data::havok::Limits {
    data::havok::Limits {
        max_input_bytes: 16 << 20,
        max_allocation_bytes: 16 << 20,
        max_values: 200_000,
        max_array_length: 200_000,
        max_validation_steps: 1_000_000,
        ..Default::default()
    }
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn check(
    kind: &str,
    bytes: &[u8],
    auxiliary: &[u8],
    case: usize,
    iteration: usize,
    counts: &mut [usize; 3],
    out: &Path,
) -> Result<u8, Box<dyn std::error::Error>> {
    match std::panic::catch_unwind(|| exercise(kind, bytes, auxiliary)) {
        Ok(status) => {
            counts[0] += 1;
            counts[1] += usize::from(status & 1 != 0);
            counts[2] += usize::from(status & 2 != 0);
            Ok(status)
        }
        Err(_) => {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out.join(format!("panic-{case}-{iteration}.bin")))?
                .write_all(bytes)?;
            Err(format!("panic: case={case} kind={kind} iteration={iteration}").into())
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("expected manifest new-output-directory".into());
    }
    let out = Path::new(&args[1]);
    std::fs::create_dir(out)?;
    if std::fs::metadata(&args[0])?.len() > 64 << 10 {
        return Err("manifest byte budget".into());
    }
    let manifest = std::fs::read_to_string(&args[0])?;
    if manifest.lines().count() > 64 {
        return Err("case budget".into());
    }
    let mut total = 0u64;
    for (case, line) in manifest.lines().enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 {
            return Err("manifest columns".into());
        }
        for path in [fields[1], fields[2]].into_iter().filter(|p| *p != "-") {
            let size = std::fs::metadata(path)?.len();
            if size > 16 << 20 {
                return Err("snapshot budget".into());
            }
            total += size;
            if total > 64 << 20 {
                return Err("total snapshot budget".into());
            }
        }
        let original = std::fs::read(fields[1])?;
        let auxiliary = if fields[2] == "-" {
            Vec::new()
        } else {
            std::fs::read(fields[2])?
        };
        let mut counts = [0; 3];
        if check(fields[0], &original, &auxiliary, case, 0, &mut counts, out)? & 1 == 0 {
            return Err(format!("nominal input not accepted: {case} {}", fields[0]).into());
        }
        for length in 0..original.len().min(128) {
            check(
                fields[0],
                &original[..length],
                &auxiliary,
                case,
                counts[0],
                &mut counts,
                out,
            )?;
        }
        for length in [
            original.len() / 4,
            original.len() / 2,
            original.len() * 3 / 4,
            original.len().saturating_sub(1),
        ] {
            check(
                fields[0],
                &original[..length],
                &auxiliary,
                case,
                counts[0],
                &mut counts,
                out,
            )?;
        }
        for offset in (0..original.len().min(256).saturating_sub(3)).step_by(4) {
            for word in [0u32, u32::MAX, 0x80000000, 0x7fffffff] {
                let mut changed = original.clone();
                changed[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                check(
                    fields[0],
                    &changed,
                    &auxiliary,
                    case,
                    counts[0],
                    &mut counts,
                    out,
                )?;
            }
        }
        let mut state = 0x8bd42f193a7e6501u64 ^ case as u64;
        for i in 0..256 {
            if original.is_empty() {
                break;
            }
            let mut changed = original.clone();
            let span = if i % 2 == 0 {
                original.len().min(512)
            } else {
                original.len()
            };
            let start = (next(&mut state) % span as u64) as usize;
            let width = [1, 2, 4, 8][(next(&mut state) % 4) as usize];
            for byte in &mut changed[start..(start + width).min(original.len())] {
                *byte ^= next(&mut state) as u8;
            }
            check(
                fields[0],
                &changed,
                &auxiliary,
                case,
                counts[0],
                &mut counts,
                out,
            )?;
        }
        println!(
            "{case}\t{}\t{}\t{}\t{}",
            fields[0], counts[0], counts[1], counts[2]
        );
    }
    Ok(())
}

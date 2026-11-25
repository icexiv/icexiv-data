//! Inspect extracted PAP timeline links or AVFX binder transform inputs.
use icexiv_data::{
    avfx::{Avfx, Curve, Kind},
    pap::Pap,
    tmb::{Timeline, TimelineLimits},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    let path = std::env::args_os()
        .nth(1)
        .ok_or("expected extracted PAP or AVFX")?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((64 << 20) + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 << 20 {
        return Err("input budget".into());
    }
    if bytes.starts_with(b"pap ") {
        for t in Pap::parse(&bytes)?.timelines(TimelineLimits::default())? {
            for (i, e) in t.entries.iter().enumerate() {
                print!(
                    "{}\t{}\t{}",
                    t.offset,
                    e.offset,
                    String::from_utf8_lossy(&e.tag)
                );
                if let Some(ids) = t.linked_ids(i)? {
                    for id in ids {
                        print!("\t{id}");
                    }
                }
                if let Some(name) = t.c009_name(i)? {
                    print!("\t{}", String::from_utf8_lossy(name));
                }
                println!();
            }
        }
    } else if bytes.starts_with(b"TMLB") {
        for timeline in Timeline::parse_sequence(&bytes, TimelineLimits::default())? {
            for (i, e) in timeline.entries.iter().enumerate() {
                if e.tag == *b"TMTR" {
                    let track = timeline.track(i)?;
                    if let Some(cs) = track.conditions() {
                        for c in cs {
                            println!(
                                "condition\t{}\t{}\t{}\t{}\t{:08x}",
                                track.id,
                                track.start,
                                c.operation,
                                c.value,
                                c.float_value.to_bits()
                            );
                        }
                    }
                }
            }
            for link in timeline.command_links(16 << 20, 1_000_000)? {
                let i = link.command;
                let entry = &timeline.entries[i];
                if let Some(r) = timeline.command_resource(i)? {
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{:?}\t{}",
                        entry.offset,
                        String::from_utf8_lossy(&entry.tag),
                        r.id,
                        r.start,
                        r.duration,
                        r.kind,
                        String::from_utf8_lossy(r.name)
                    );
                }
                if let Some(fx) = timeline.effect_command(i)? {
                    println!(
                        "effect\tactor@{}\ttrack@{}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{}",
                        timeline.entries[link.actor].offset,
                        timeline.entries[link.track].offset,
                        fx.targets,
                        fx.scale,
                        fx.rotation_radians,
                        fx.position,
                        fx.color,
                        fx.visibility
                    );
                }
            }
        }
    } else {
        let avfx = Avfx::parse(&bytes)?;
        for n in avfx.nodes_of(Kind::Binder) {
            println!(
                "{}\t{:?}\tSpS={:?}",
                n.block.offset,
                n.binder_transform_flags()?,
                n.constant_point_sps(1024)?
            );
        }
        // Optional argument: sample time in authored frames.
        if let Some(time) = std::env::args().nth(2) {
            let time: f32 = time.parse()?;
            for (i, p) in avfx.nodes_of(Kind::Particle).enumerate() {
                let Some(color) = p.fields.one(b"Col")? else {
                    continue;
                };
                let channels = color.fields(1024)?;
                if let Some(rgb) = channels.one(b"RGB")? {
                    let v = Curve::parse(rgb, 1024)?.rgb(127)?.sample(time)?;
                    println!("rgb\t{i}\t{time}\t{}\t{}\t{}", v[0], v[1], v[2]);
                }
                let Some(alpha) = channels.one(b"A")? else {
                    continue;
                };
                let curve = Curve::parse(alpha, 1024)?.scalar(255, 65536)?;
                println!("alpha\t{i}\t{time}\t{}", curve.sample(time)?);
            }
        }
    }
    Ok(())
}

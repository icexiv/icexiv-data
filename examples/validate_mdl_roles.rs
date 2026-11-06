//! Print all LOD role ranges and terrain-shadow metadata/buffers. Args: MDL.
use icexiv_data::mdl::Mdl;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("MDL input")?;
    if std::fs::metadata(&path)?.len() > 64 << 20 {
        return Err("input size".into());
    }
    let b = std::fs::read(path)?;
    let m = Mdl::parse(&b)?;
    let mut output_work = 1_000_000usize;
    for lod in 0..usize::from(m.header().lod_count) {
        for r in m.lod(lod)?.ranges {
            println!("range\t{lod}\t{:?}\t{}\t{}", r.role, r.start, r.count);
        }
        let r = m.lod(lod)?.ranges[3];
        for index in usize::from(r.start)..usize::from(r.start) + usize::from(r.count) {
            let g = m.terrain_shadow_geometry(lod, index)?;
            let t = g.metadata;
            output_work = output_work
                .checked_sub(usize::from(t.vertex_count) + usize::try_from(t.index_count)?)
                .ok_or("output work budget")?;
            println!(
                "terrain\t{lod}\t{index}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                t.index_count,
                t.start_index,
                t.vertex_offset,
                t.vertex_count,
                t.submesh_start,
                t.submesh_count,
                t.stride
            );
            for v in 0..usize::from(t.vertex_count) {
                println!(
                    "terrain_vertex\t{lod}\t{index}\t{v}\t{}",
                    g.position_half4(v)?
                        .map(|v| format!("{:08x}", v.to_bits()))
                        .join(",")
                );
            }
            println!(
                "terrain_indices\t{lod}\t{index}\t{}",
                g.indices()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for s in usize::from(t.submesh_start)
                ..usize::from(t.submesh_start) + usize::from(t.submesh_count)
            {
                let r = m.terrain_shadow_submesh(s)?;
                println!(
                    "terrain_submesh\t{s}\t{}\t{}\t{},{}",
                    r.index_offset, r.index_count, r.unknown[0], r.unknown[1]
                );
            }
        }
    }
    Ok(())
}

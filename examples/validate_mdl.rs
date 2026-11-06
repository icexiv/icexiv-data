//! Inspect extracted MDL files without installation access or GPU use.
//! Usage: cargo run --example validate_mdl -- <extracted.mdl> ...
//! FNV-1a fingerprints cover decoded scalar bits and every index, not just headers.

use icexiv_data::mdl::Mdl;

fn mix(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("expected extracted MDL file paths".into());
    }
    for path in paths {
        if std::fs::metadata(&path)?.len() > 64 * 1024 * 1024 {
            return Err("sample exceeds 64 MiB".into());
        }
        let bytes = std::fs::read(&path)?;
        let mdl = Mdl::parse(&bytes)?;
        println!(
            "file\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            bytes.len(),
            mdl.mesh_count(),
            mdl.material_count(),
            mdl.bone_count(),
            mdl.shape_count(),
            mdl.shape_mesh_count(),
            mdl.shape_value_count()
        );
        for i in 0..mdl.attribute_count() {
            println!("attribute\t{i}\t{}", hex(mdl.attribute(i)?));
        }
        for i in 0..mdl.material_count() {
            println!("material\t{i}\t{}", hex(mdl.material(i)?));
        }
        for i in 0..mdl.bone_count() {
            println!("bone\t{i}\t{}", hex(mdl.bone_name(i)?));
        }
        for i in 0..mdl.bone_table_count() {
            println!(
                "palette\t{i}\t{}",
                mdl.bone_table(i)?
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        for i in 0..mdl.submesh_count() {
            let s = mdl.submesh(i)?;
            println!(
                "submesh\t{i}\t{}\t{}\t{}\t{}\t{}",
                s.index_offset, s.index_count, s.attribute_mask, s.bone_start, s.bone_count
            );
        }
        for i in 0..mdl.bounding_box_count() {
            let box_ = mdl.bounding_box(i)?;
            println!(
                "bounds\t{i}\t{}",
                box_.min
                    .into_iter()
                    .chain(box_.max)
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        for i in 0..mdl.element_id_count() {
            let e = mdl.element_id(i)?;
            println!(
                "element\t{}\t{}\t{}",
                e.id,
                hex(mdl.string(e.parent_bone_string_offset)?),
                e.translation
                    .into_iter()
                    .chain(e.rotation)
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        for i in 0..mdl.shape_count() {
            let s = mdl.shape(i)?;
            println!(
                "shape\t{i}\t{}\t{}\t{}",
                hex(s.name),
                s.mesh_starts.map(|n| n.to_string()).join(","),
                s.mesh_counts.map(|n| n.to_string()).join(",")
            );
        }
        for i in 0..mdl.shape_mesh_count() {
            let sm = mdl.shape_mesh(i)?;
            let mut hash = 0xcbf29ce484222325;
            for v in sm.value_offset..sm.value_offset + sm.value_count {
                let value = mdl.shape_value(v as usize)?;
                mix(&mut hash, u32::from(value.base_index));
                mix(&mut hash, u32::from(value.replacement_vertex));
            }
            println!(
                "replacement\t{i}\t{}\t{}\t{}\t{hash:016x}",
                sm.mesh_index_offset, sm.value_count, sm.value_offset
            );
        }
        for lod in 0..usize::from(mdl.header().lod_count) {
            for i in 0..mdl.mesh_count() {
                let Ok(geometry) = mdl.geometry(lod, i) else {
                    continue;
                };
                let m = geometry.metadata();
                let mut hash = 0xcbf29ce484222325;
                for element in geometry.declaration()? {
                    for v in [
                        element.stream,
                        element.offset,
                        element.kind,
                        element.usage,
                        element.usage_index,
                    ] {
                        mix(&mut hash, u32::from(v));
                    }
                    for v in 0..usize::from(m.vertex_count) {
                        for lane in geometry.vertex_element(v, element)? {
                            mix(&mut hash, lane.to_bits());
                        }
                    }
                }
                for index in geometry.indices()? {
                    mix(&mut hash, u32::from(index));
                }
                println!(
                    "mesh\t{lod}\t{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{hash:016x}",
                    m.vertex_count,
                    m.index_count,
                    m.material_index,
                    m.submesh_start,
                    m.submesh_count,
                    m.bone_table_index
                );
            }
        }
    }
    Ok(())
}

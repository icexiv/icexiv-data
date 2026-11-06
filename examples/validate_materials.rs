//! CPU-only inspection of extracted MTRL/SHPK files.
//! Args: extracted-material.mtrl or shader.shpk ...
//! Hashes cover complete shader bytes, per-shader resources and node metadata.

use icexiv_data::{
    mtrl::Mtrl,
    shpk::{KeyGroup, Resource, ResourceKind, Shpk, Stage},
};

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
struct Hash(u64);
impl Hash {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    fn word(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    fn resource(&mut self, r: Resource<'_>) {
        self.word(r.id);
        self.word(r.name.len() as u32);
        self.bytes(r.name);
        self.word(u32::from(r.flags));
        self.word(u32::from(r.slot));
        self.word(u32::from(r.size));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("expected extracted MTRL/SHPK files".into());
    }
    for path in paths {
        if std::fs::metadata(&path)?.len() > 128 * 1024 * 1024 {
            return Err("sample exceeds 128 MiB".into());
        }
        let bytes = std::fs::read(path)?;
        if bytes.starts_with(b"ShPk") {
            inspect_shpk(&bytes)?;
        } else {
            inspect_mtrl(&bytes)?;
        }
    }
    Ok(())
}
fn inspect_mtrl(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let m = Mtrl::parse(bytes)?;
    println!(
        "mtrl\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        bytes.len(),
        hex(m.shader_package()?),
        m.texture_count(),
        m.uv_set_count(),
        m.color_set_count(),
        m.key_count(),
        m.constant_count(),
        m.sampler_count(),
        m.shader_flags()
    );
    println!("additional\t{}", hex(m.additional_data()));
    for i in 0..m.texture_count() {
        let t = m.texture(i)?;
        println!("texture\t{i}\t{}\t{}", hex(t.path), t.flags);
    }
    for (kind, count) in [("uv", m.uv_set_count()), ("color-set", m.color_set_count())] {
        for i in 0..count {
            let a = if kind == "uv" {
                m.uv_set(i)?
            } else {
                m.color_set(i)?
            };
            println!("{kind}\t{i}\t{}\t{}\t{}", hex(a.name), a.index, a.unknown);
        }
    }
    for i in 0..m.key_count() {
        let k = m.shader_key(i)?;
        println!("key\t{i}\t{}\t{}", k.id, k.value);
    }
    for i in 0..m.constant_count() {
        let c = m.constant(i)?;
        println!(
            "constant\t{i}\t{}\t{}\t{}\t{}",
            c.id,
            c.offset,
            c.size,
            hex(m.constant_bytes(i)?)
        );
    }
    for i in 0..m.sampler_count() {
        let s = m.sampler(i)?;
        println!(
            "sampler\t{i}\t{}\t{}\t{}\t{}",
            s.id,
            s.flags,
            s.texture_index,
            hex(&s.reserved)
        );
    }
    if let Some(table) = m.color_table()? {
        println!(
            "table\t{:?}\t{}\t{}",
            table.layout(),
            table.row_count(),
            table.has_dye()
        );
        for i in 0..table.row_count() {
            let row = table.row(i)?;
            println!(
                "row\t{i}\t{}\t{}",
                hex(row.raw()),
                row.values()
                    .map(|v| format!("{:08x}", v.to_bits()))
                    .join(",")
            );
            if let Some(d) = table.dye_row(i)? {
                println!(
                    "dye\t{i}\t{}\t{}\t{}\t{}\t{}",
                    d.raw,
                    d.flags(),
                    d.template(),
                    d.channel().map_or(-1, i32::from),
                    d.unknown_bits()
                );
            }
        }
    } else {
        println!("table\tabsent");
    }
    Ok(())
}
fn inspect_shpk(bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let s = Shpk::parse(bytes)?;
    let stages = [
        Stage::Vertex,
        Stage::Pixel,
        Stage::Hull,
        Stage::Domain,
        Stage::Geometry,
    ];
    let kinds = [
        ResourceKind::Constant,
        ResourceKind::Sampler,
        ResourceKind::Texture,
        ResourceKind::UnorderedAccess,
    ];
    let groups = [
        KeyGroup::System,
        KeyGroup::Scene,
        KeyGroup::Material,
        KeyGroup::SubView,
    ];
    println!(
        "shpk\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        bytes.len(),
        stages
            .map(|stage| s.shader_count(stage).to_string())
            .join(","),
        s.parameter_count(),
        s.material_buffer_size(),
        s.node_count(),
        s.alias_count(),
        s.cluster_count()
    );
    for i in 0..s.parameter_count() {
        let p = s.material_parameter(i)?;
        println!(
            "parameter\t{i}\t{}\t{}\t{}\t{}",
            p.id,
            p.offset,
            p.size,
            s.default_bytes(i)?
                .map(hex)
                .unwrap_or_else(|| "absent".into())
        );
    }
    for kind in kinds {
        for i in 0..s.resource_count(kind) {
            let r = s.resource(kind, i)?;
            println!(
                "resource\t{kind:?}\t{i}\t{}\t{}\t{}\t{}\t{}",
                r.id,
                hex(r.name),
                r.flags,
                r.slot,
                r.size
            );
        }
    }
    for group in groups {
        for i in 0..s.key_count(group) {
            let k = s.key(group, i)?;
            println!("default-key\t{group:?}\t{i}\t{}\t{}", k.id, k.default_value);
        }
    }
    for stage in stages {
        for i in 0..s.shader_count(stage) {
            let sh = s.shader(stage, i)?;
            let mut code = Hash::new();
            code.bytes(sh.bytecode());
            let mut resources = Hash::new();
            for kind in kinds {
                resources.word(sh.resource_count(kind) as u32);
                for r in 0..sh.resource_count(kind) {
                    resources.resource(sh.resource(kind, r)?);
                }
            }
            println!(
                "shader\t{stage:?}\t{i}\t{}\t{}\t{}\t{:016x}\t{:016x}",
                sh.unknown(),
                hex(sh.prefix()),
                sh.bytecode().len(),
                code.0,
                resources.0
            );
        }
    }
    let mut nodes = Hash::new();
    for i in 0..s.node_count() {
        let n = s.node(i)?;
        nodes.word(n.selector());
        nodes.word(n.pass_count() as u32);
        for index in n.pass_indices() {
            nodes.word(index as i32 as u32);
        }
        for group in groups {
            for k in 0..n.key_count(group) {
                nodes.word(n.key_value(group, k)?);
            }
        }
        for p in 0..n.pass_count() {
            let p = n.pass(p)?;
            for v in [p.id, p.vertex_shader, p.pixel_shader]
                .into_iter()
                .chain(p.unknown)
            {
                nodes.word(v);
            }
        }
        for v in n.unknown() {
            nodes.word(v);
        }
    }
    let mut aliases = Hash::new();
    for i in 0..s.alias_count() {
        let a = s.alias(i)?;
        aliases.word(a.selector);
        aliases.word(a.node_index);
    }
    let mut clusters = Hash::new();
    clusters.bytes(s.raw_alias_clusters());
    println!(
        "node-fingerprints\t{:016x}\t{:016x}\t{}\t{:016x}",
        nodes.0,
        aliases.0,
        s.raw_alias_clusters().len(),
        clusters.0
    );
    Ok(())
}

//! SHPK metadata, resource names, defaults and checked shader byte ranges.
//!
//! Parses Win32 versions 0x0d01/0x0e01 (DX11) without executing or compiling shaders.
//! Shader and node views borrow input; allocation is limited to their indices.
//! Node-alias clusters remain raw, with their declared count and bounded region.
//! Other versions return errors rather than guessed layouts. Limits bound input,
//! metadata records, index allocation and cumulative string-validation work.

use crate::binary::{Reader, record, slice, u16_at, u32_at};
use std::{fmt, mem::size_of};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion(u32),
    UnsupportedPlatform(u32),
    BudgetExceeded(&'static str),
    AllocationFailed,
}
impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Self::InvalidData(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(e) => write!(f, "invalid SHPK data: {e}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported SHPK version {v:#x}"),
            Self::UnsupportedPlatform(v) => write!(f, "unsupported SHPK platform {v:#x}"),
            Self::BudgetExceeded(s) => write!(f, "SHPK {s} budget exceeded"),
            Self::AllocationFailed => f.write_str("SHPK allocation failed"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_records: usize,
    pub max_metadata_bytes: usize,
    pub max_string_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 128 * 1024 * 1024,
            max_records: 1024 * 1024,
            max_metadata_bytes: 64 * 1024 * 1024,
            max_string_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Shader-name CRC-32: reflected IEEE polynomial, initial state zero, no final
/// xor. This differs from SqPack path JAMCRC and the usual zlib default CRC.
pub fn name_id(name: &[u8]) -> u32 {
    let mut hash = crc32fast::Hasher::new_with_initial(u32::MAX);
    hash.update(name);
    !hash.finalize()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Vertex,
    Pixel,
    Hull,
    Domain,
    Geometry,
}
impl Stage {
    fn index(self) -> usize {
        match self {
            Self::Vertex => 0,
            Self::Pixel => 1,
            Self::Hull => 2,
            Self::Domain => 3,
            Self::Geometry => 4,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Constant,
    Sampler,
    Texture,
    UnorderedAccess,
}
impl ResourceKind {
    fn index(self) -> usize {
        match self {
            Self::Constant => 0,
            Self::Sampler => 1,
            Self::Texture => 2,
            Self::UnorderedAccess => 3,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyGroup {
    System,
    Scene,
    Material,
    SubView,
}
impl KeyGroup {
    fn index(self) -> usize {
        match self {
            Self::System => 0,
            Self::Scene => 1,
            Self::Material => 2,
            Self::SubView => 3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Resource<'a> {
    pub id: u32,
    pub name: &'a [u8],
    pub flags: u16,
    pub slot: u16,
    pub size: u16,
}
fn resource<'a>(b: &[u8], names: &'a [u8]) -> Result<Resource<'a>> {
    let offset = u32_at(b, 4)? as usize;
    let length = usize::from(u16_at(b, 8)?);
    let data = slice(
        names,
        offset,
        length
            .checked_add(1)
            .ok_or(Error::InvalidData("resource name size overflow"))?,
    )?;
    if data[length] != 0 || data[..length].contains(&0) {
        return Err(Error::InvalidData("resource name length/terminator"));
    }
    Ok(Resource {
        id: u32_at(b, 0)?,
        name: &data[..length],
        flags: u16_at(b, 10)?,
        slot: u16_at(b, 12)?,
        size: u16_at(b, 14)?,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Shader<'a> {
    stage: Stage,
    unknown: u32,
    resources: [&'a [u8]; 4],
    names: &'a [u8],
    prefix: &'a [u8],
    bytecode: &'a [u8],
}
impl<'a> Shader<'a> {
    pub fn stage(self) -> Stage {
        self.stage
    }
    pub fn unknown(self) -> u32 {
        self.unknown
    }
    pub fn prefix(self) -> &'a [u8] {
        self.prefix
    }
    pub fn bytecode(self) -> &'a [u8] {
        self.bytecode
    }
    pub fn resource_count(self, kind: ResourceKind) -> usize {
        self.resources[kind.index()].len() / 16
    }
    pub fn resource(self, kind: ResourceKind, index: usize) -> Result<Resource<'a>> {
        resource(record(self.resources[kind.index()], index, 16)?, self.names)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialParameter {
    pub id: u32,
    pub offset: u16,
    pub size: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Key {
    pub id: u32,
    pub default_value: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pass {
    pub id: u32,
    pub vertex_shader: u32,
    pub pixel_shader: u32,
    pub unknown: [u32; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct Node<'a> {
    selector: u32,
    pass_indices: [i8; 16],
    keys: [&'a [u8]; 4],
    passes: &'a [u8],
    unknown: [u32; 2],
}
impl Node<'_> {
    pub fn selector(self) -> u32 {
        self.selector
    }
    pub fn pass_indices(self) -> [i8; 16] {
        self.pass_indices
    }
    pub fn unknown(self) -> [u32; 2] {
        self.unknown
    }
    pub fn key_count(self, group: KeyGroup) -> usize {
        self.keys[group.index()].len() / 4
    }
    pub fn key_value(self, group: KeyGroup, index: usize) -> Result<u32> {
        Ok(u32_at(record(self.keys[group.index()], index, 4)?, 0)?)
    }
    pub fn pass_count(self) -> usize {
        self.passes.len() / 24
    }
    pub fn pass(self, index: usize) -> Result<Pass> {
        let b = record(self.passes, index, 24)?;
        Ok(Pass {
            id: u32_at(b, 0)?,
            vertex_shader: u32_at(b, 4)?,
            pixel_shader: u32_at(b, 8)?,
            unknown: [u32_at(b, 12)?, u32_at(b, 16)?, u32_at(b, 20)?],
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Alias {
    pub selector: u32,
    pub node_index: u32,
}

#[derive(Debug)]
pub struct Shpk<'a> {
    version: u32,
    shader_counts: [usize; 5],
    shaders: Vec<Shader<'a>>,
    parameters: &'a [u8],
    material_size: u32,
    defaults: Option<&'a [u8]>,
    resources: [&'a [u8]; 4],
    keys: [&'a [u8]; 3],
    subview: [u32; 2],
    nodes: Vec<Node<'a>>,
    aliases: &'a [u8],
    cluster_count: u32,
    clusters: &'a [u8],
    names: &'a [u8],
    header_unknown: [u16; 2],
}
impl<'a> Shpk<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("input"));
        }
        let h = slice(bytes, 0, 12)?;
        if &h[..4] != b"ShPk" {
            return Err(Error::InvalidData("magic"));
        }
        let version = u32_at(h, 4)?;
        let header_size = match version {
            0x0d01 => 84,
            0x0e01 => 88,
            _ => return Err(Error::UnsupportedVersion(version)),
        };
        let h = slice(bytes, 0, header_size)?;
        let dx = u32_at(h, 8)?;
        if &h[8..12] != b"DX11" {
            return Err(Error::UnsupportedPlatform(dx));
        }
        if u32_at(h, 12)? as usize != bytes.len() {
            return Err(Error::InvalidData("declared file size mismatch"));
        }
        let shader_offset = u32_at(h, 16)? as usize;
        let names_offset = u32_at(h, 20)? as usize;
        if shader_offset < header_size || names_offset < shader_offset {
            return Err(Error::InvalidData("region order"));
        }
        let metadata = slice(bytes, 0, shader_offset)?;
        let code = slice(bytes, shader_offset, names_offset - shader_offset)?;
        let names = bytes
            .get(names_offset..)
            .ok_or(Error::InvalidData("names offset"))?;
        let counts = [
            u32_at(h, 24)? as usize,
            u32_at(h, 28)? as usize,
            u32_at(h, 72)? as usize,
            u32_at(h, 76)? as usize,
            u32_at(h, 80)? as usize,
        ];
        let node_count = u32_at(h, 64)? as usize;
        let alias_count = u32_at(h, 68)? as usize;
        let cluster_count = if version == 0x0e01 { u32_at(h, 84)? } else { 0 };
        let mut records_left = limits.max_records;
        let mut charge = |n: usize| -> Result<()> {
            records_left = records_left
                .checked_sub(n)
                .ok_or(Error::BudgetExceeded("records"))?;
            Ok(())
        };
        let total_shaders = counts
            .iter()
            .try_fold(0usize, |a, &b| a.checked_add(b))
            .ok_or(Error::InvalidData("shader count overflow"))?;
        charge(total_shaders)?;
        charge(node_count)?;
        charge(alias_count)?;
        charge(cluster_count as usize)?;
        let allocation = total_shaders
            .checked_mul(size_of::<Shader<'a>>())
            .and_then(|s| {
                node_count
                    .checked_mul(size_of::<Node<'a>>())
                    .and_then(|n| s.checked_add(n))
            })
            .ok_or(Error::BudgetExceeded("index allocation"))?;
        if allocation > limits.max_metadata_bytes {
            return Err(Error::BudgetExceeded("index allocation"));
        }
        // Minimum fixed prefixes must fit before allocating count-driven indices.
        let minimum = total_shaders
            .checked_mul(20)
            .and_then(|s| node_count.checked_mul(32).and_then(|n| s.checked_add(n)))
            .and_then(|n| alias_count.checked_mul(8).and_then(|a| n.checked_add(a)))
            .ok_or(Error::InvalidData("minimum metadata size overflow"))?;
        if minimum > metadata.len() - header_size {
            return Err(Error::InvalidData("counts exceed metadata"));
        }
        let mut shaders = Vec::new();
        shaders
            .try_reserve_exact(total_shaders)
            .map_err(|_| Error::AllocationFailed)?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(node_count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut r = Reader {
            data: metadata,
            pos: header_size,
        };
        for stage in [
            Stage::Vertex,
            Stage::Pixel,
            Stage::Hull,
            Stage::Domain,
            Stage::Geometry,
        ] {
            for _ in 0..counts[stage.index()] {
                let b = r.take(1, 20)?;
                let mut tables = [&[][..]; 4];
                for (kind, offset) in [
                    (ResourceKind::Constant, 8),
                    (ResourceKind::Sampler, 10),
                    (ResourceKind::UnorderedAccess, 12),
                    (ResourceKind::Texture, 14),
                ] {
                    let count = usize::from(u16_at(b, offset)?);
                    charge(count)?;
                    tables[kind.index()] = r.take(count, 16)?;
                }
                let data = slice(code, u32_at(b, 0)? as usize, u32_at(b, 4)? as usize)?;
                let prefix = if stage == Stage::Vertex { 8 } else { 0 };
                shaders.push(Shader {
                    stage,
                    unknown: u32_at(b, 16)?,
                    resources: tables,
                    names,
                    prefix: slice(data, 0, prefix)?,
                    bytecode: data
                        .get(prefix..)
                        .ok_or(Error::InvalidData("shader prefix"))?,
                });
            }
        }
        let count = usize::from(u16_at(h, 36)?);
        charge(count)?;
        let parameters = r.take(count, 8)?;
        let material_size = u32_at(h, 32)?;
        let default_flag = u16_at(h, 38)?;
        if default_flag > 1 {
            return Err(Error::InvalidData("default-value flag"));
        }
        let defaults = if default_flag != 0 {
            Some(r.take(material_size as usize, 1)?)
        } else {
            None
        };
        let mut resources = [&[][..]; 4];
        for (kind, offset) in [
            (ResourceKind::Constant, 40),
            (ResourceKind::Sampler, 44),
            (ResourceKind::Texture, 46),
            (ResourceKind::UnorderedAccess, 48),
        ] {
            let count = usize::from(u16_at(h, offset)?);
            charge(count)?;
            resources[kind.index()] = r.take(count, 16)?;
        }
        let mut keys = [&[][..]; 3];
        for (i, table) in keys.iter_mut().enumerate() {
            let count = u32_at(h, 52 + i * 4)? as usize;
            charge(count)?;
            *table = r.take(count, 8)?;
        }
        let b = r.take(1, 8)?;
        let subview = [u32_at(b, 0)?, u32_at(b, 4)?];
        for _ in 0..node_count {
            let b = r.take(1, 24)?;
            let passes = u32_at(b, 4)? as usize;
            charge(passes)?;
            let unknown = r.take(1, 8)?;
            let mut node_keys = [&[][..]; 4];
            for i in 0..4 {
                node_keys[i] = r.take(if i < 3 { keys[i].len() / 8 } else { 2 }, 4)?;
            }
            let pass_data = r.take(passes, 24)?;
            let node = Node {
                selector: u32_at(b, 0)?,
                pass_indices: std::array::from_fn(|i| b[8 + i] as i8),
                keys: node_keys,
                passes: pass_data,
                unknown: [u32_at(unknown, 0)?, u32_at(unknown, 4)?],
            };
            for index in node.pass_indices {
                if index < -1 || (index >= 0 && index as usize >= passes) {
                    return Err(Error::InvalidData("node pass index"));
                }
            }
            for i in 0..passes {
                let pass = node.pass(i)?;
                if pass.vertex_shader != u32::MAX && pass.vertex_shader as usize >= counts[0] {
                    return Err(Error::InvalidData("pass vertex shader index"));
                }
                if pass.pixel_shader != u32::MAX && pass.pixel_shader as usize >= counts[1] {
                    return Err(Error::InvalidData("pass pixel shader index"));
                }
            }
            nodes.push(node);
        }
        let aliases = r.take(alias_count, 8)?;
        let clusters = metadata
            .get(r.pos..)
            .ok_or(Error::InvalidData("cluster offset"))?;
        if cluster_count as usize > clusters.len() / 4 {
            return Err(Error::InvalidData(
                "cluster count exceeds remaining metadata",
            ));
        }
        if cluster_count == 0 && !clusters.is_empty() {
            return Err(Error::InvalidData("unattributed metadata tail"));
        }
        let package = Self {
            version,
            shader_counts: counts,
            shaders,
            parameters,
            material_size,
            defaults,
            resources,
            keys,
            subview,
            nodes,
            aliases,
            cluster_count,
            clusters,
            names,
            header_unknown: [u16_at(h, 42)?, u16_at(h, 50)?],
        };
        for i in 0..package.parameter_count() {
            let p = package.material_parameter(i)?;
            let end = usize::from(p.offset) + usize::from(p.size);
            if end > material_size as usize {
                return Err(Error::InvalidData("material parameter exceeds buffer size"));
            }
        }
        for i in 0..alias_count {
            if package.alias(i)?.node_index as usize >= node_count {
                return Err(Error::InvalidData("alias node index"));
            }
        }
        let mut string_budget = limits.max_string_bytes;
        for tables in package
            .shaders
            .iter()
            .map(|s| s.resources)
            .chain(std::iter::once(resources))
        {
            for table in tables {
                for b in table.chunks_exact(16) {
                    let size = usize::from(u16_at(b, 8)?) + 1;
                    string_budget = string_budget
                        .checked_sub(size)
                        .ok_or(Error::BudgetExceeded("name validation"))?;
                    resource(b, names)?;
                }
            }
        }
        Ok(package)
    }
    pub fn header_unknown(&self) -> [u16; 2] {
        self.header_unknown
    }
    pub fn shader_count(&self, stage: Stage) -> usize {
        self.shader_counts[stage.index()]
    }
    pub fn shader(&self, stage: Stage, index: usize) -> Result<Shader<'a>> {
        if index >= self.shader_count(stage) {
            return Err(Error::InvalidData("shader index"));
        }
        let start: usize = self.shader_counts[..stage.index()].iter().sum();
        Ok(self.shaders[start + index])
    }
    pub fn parameter_count(&self) -> usize {
        self.parameters.len() / 8
    }
    pub fn material_buffer_size(&self) -> u32 {
        self.material_size
    }
    pub fn material_parameter(&self, index: usize) -> Result<MaterialParameter> {
        let b = record(self.parameters, index, 8)?;
        Ok(MaterialParameter {
            id: u32_at(b, 0)?,
            offset: u16_at(b, 4)?,
            size: u16_at(b, 6)?,
        })
    }
    pub fn default_bytes(&self, parameter: usize) -> Result<Option<&'a [u8]>> {
        let p = self.material_parameter(parameter)?;
        self.defaults
            .map(|b| slice(b, usize::from(p.offset), usize::from(p.size)).map_err(Error::from))
            .transpose()
    }
    pub fn default_floats(
        &self,
        parameter: usize,
    ) -> Result<Option<impl ExactSizeIterator<Item = f32> + 'a>> {
        let p = self.material_parameter(parameter)?;
        if p.offset % 4 != 0 || p.size % 4 != 0 {
            return Err(Error::InvalidData("unaligned float parameter"));
        }
        Ok(self.default_bytes(parameter)?.map(|b| {
            b.chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        }))
    }
    pub fn resource_count(&self, kind: ResourceKind) -> usize {
        self.resources[kind.index()].len() / 16
    }
    pub fn resource(&self, kind: ResourceKind, index: usize) -> Result<Resource<'a>> {
        resource(record(self.resources[kind.index()], index, 16)?, self.names)
    }
    pub fn key_count(&self, group: KeyGroup) -> usize {
        if group == KeyGroup::SubView {
            2
        } else {
            self.keys[group.index()].len() / 8
        }
    }
    /// SubView IDs 1 and 2 are implicit ordinals; only their default values are
    /// stored in the file. Other groups carry explicit IDs.
    pub fn key(&self, group: KeyGroup, index: usize) -> Result<Key> {
        if group == KeyGroup::SubView {
            return self
                .subview
                .get(index)
                .map(|&value| Key {
                    id: index as u32 + 1,
                    default_value: value,
                })
                .ok_or(Error::InvalidData("subview key index"));
        }
        let b = record(self.keys[group.index()], index, 8)?;
        Ok(Key {
            id: u32_at(b, 0)?,
            default_value: u32_at(b, 4)?,
        })
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn node(&self, index: usize) -> Result<Node<'a>> {
        self.nodes
            .get(index)
            .copied()
            .ok_or(Error::InvalidData("node index"))
    }
    pub fn alias_count(&self) -> usize {
        self.aliases.len() / 8
    }
    pub fn alias(&self, index: usize) -> Result<Alias> {
        let b = record(self.aliases, index, 8)?;
        Ok(Alias {
            selector: u32_at(b, 0)?,
            node_index: u32_at(b, 4)?,
        })
    }
    pub fn cluster_count(&self) -> u32 {
        self.cluster_count
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    /// The field is absent in the 84-byte 0x0d01 header. `cluster_count` gives
    /// the logical count (zero when no cluster section is declared).
    pub fn declared_cluster_count(&self) -> Option<u32> {
        (self.version == 0x0e01).then_some(self.cluster_count)
    }
    pub fn raw_alias_clusters(&self) -> &'a [u8] {
        self.clusters
    }
    pub fn strings(&self) -> &'a [u8] {
        self.names
    }
}

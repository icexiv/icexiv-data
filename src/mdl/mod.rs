//! Checked, byte-only parsing of little-endian MDL models (version 0x01000006).
//!
//! Parsing borrows metadata and buffers without allocating. Mesh views expose
//! raw declarations, scalar vertex conversion, checked u16 indices, submeshes,
//! bone palettes, shapes and ElementId attachments. No coordinate conversion,
//! shape baking, skeleton evaluation or material path resolution is performed.
//! Limits bound input, metadata and index validation work. Attribute decoding
//! to a Vec additionally requires an explicit output byte budget.
//!
//! Unknown runtime extension bytes and header flags remain accessible. APIs
//! return errors for unsupported versions/types, invalid references and budgets;
//! strings are NUL-terminated raw bytes, not assumed UTF-8.

mod vertex;
use std::{fmt, ops::Range};
pub use vertex::Element;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion(u32),
    UnsupportedVertexType(u8),
    OutOfBounds(&'static str),
    BudgetExceeded(&'static str),
    AllocationFailed,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(s) => write!(f, "invalid MDL data: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported MDL version {v:#x}"),
            Self::UnsupportedVertexType(v) => write!(f, "unsupported MDL vertex type {v}"),
            Self::OutOfBounds(s) => write!(f, "MDL {s} out of bounds"),
            Self::BudgetExceeded(s) => write!(f, "MDL {s} budget exceeded"),
            Self::AllocationFailed => f.write_str("MDL allocation failed"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

fn span(bytes: &[u8], start: usize, length: usize) -> Result<&[u8]> {
    let end = start
        .checked_add(length)
        .ok_or(Error::InvalidData("range overflow"))?;
    bytes
        .get(start..end)
        .ok_or(Error::InvalidData("range exceeds input"))
}
fn u16_at(bytes: &[u8], start: usize) -> Result<u16> {
    let b = span(bytes, start, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}
fn u32_at(bytes: &[u8], start: usize) -> Result<u32> {
    let b = span(bytes, start, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn f32_at(bytes: &[u8], start: usize) -> Result<f32> {
    Ok(f32::from_bits(u32_at(bytes, start)?))
}
fn record(bytes: &[u8], index: usize, stride: usize) -> Result<&[u8]> {
    span(
        bytes,
        index
            .checked_mul(stride)
            .ok_or(Error::InvalidData("record overflow"))?,
        stride,
    )
}
fn range(start: usize, count: usize, total: usize) -> Result<Range<usize>> {
    if count == 0 {
        return Ok(start..start);
    }
    let end = start
        .checked_add(count)
        .ok_or(Error::InvalidData("reference overflow"))?;
    if end > total {
        return Err(Error::InvalidData("reference exceeds table"));
    }
    Ok(start..end)
}
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize, stride: usize) -> Result<&'a [u8]> {
        let size = count
            .checked_mul(stride)
            .ok_or(Error::InvalidData("table overflow"))?;
        let data = span(self.data, self.pos, size)?;
        self.pos += size;
        Ok(data)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_records: usize,
    pub max_indices: usize,
    pub max_validation_steps: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 512 * 1024 * 1024,
            max_records: 1024 * 1024,
            max_indices: 32 * 1024 * 1024,
            max_validation_steps: 64 * 1024 * 1024,
        }
    }
}

struct Work(usize);
impl Work {
    fn charge(&mut self, count: usize) -> Result<()> {
        self.0 = self
            .0
            .checked_sub(count)
            .ok_or(Error::BudgetExceeded("validation work"))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FileHeader {
    pub version: u32,
    pub stack_size: u32,
    pub runtime_size: u32,
    pub declarations: u16,
    pub materials: u16,
    pub vertex_offsets: [u32; 3],
    pub index_offsets: [u32; 3],
    pub vertex_sizes: [u32; 3],
    pub index_sizes: [u32; 3],
    pub lod_count: u8,
    pub streaming: u8,
    pub edge_geometry: u8,
}
fn array3(bytes: &[u8], offset: usize) -> Result<[u32; 3]> {
    Ok([
        u32_at(bytes, offset)?,
        u32_at(bytes, offset + 4)?,
        u32_at(bytes, offset + 8)?,
    ])
}

#[derive(Clone, Copy, Debug)]
pub struct Mesh {
    pub vertex_count: u16,
    pub index_count: u32,
    pub material_index: u16,
    pub submesh_start: u16,
    pub submesh_count: u16,
    pub bone_table_index: u16,
    /// Offset in u16 index elements, relative to this LOD's index buffer.
    pub start_index: u32,
    pub stream_offsets: [u32; 3],
    pub stream_strides: [u8; 3],
    pub stream_count: u8,
    /// Original byte, including flags above the low two stream-count bits.
    pub raw_stream_flags: u8,
}
impl Mesh {
    fn parse(b: &[u8]) -> Result<Self> {
        Ok(Self {
            vertex_count: u16_at(b, 0)?,
            index_count: u32_at(b, 4)?,
            material_index: u16_at(b, 8)?,
            submesh_start: u16_at(b, 10)?,
            submesh_count: u16_at(b, 12)?,
            bone_table_index: u16_at(b, 14)?,
            start_index: u32_at(b, 16)?,
            stream_offsets: array3(b, 20)?,
            stream_strides: [b[32], b[33], b[34]],
            stream_count: b[35] & 3,
            raw_stream_flags: b[35],
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshRole {
    Normal,
    Water,
    Shadow,
    TerrainShadow,
    VerticalFog,
    LightShaft,
    Glass,
    MaterialChange,
    CrestChange,
}
#[derive(Clone, Copy, Debug)]
pub struct MeshRange {
    pub role: MeshRole,
    pub start: u16,
    pub count: u16,
}
#[derive(Clone, Copy, Debug)]
pub struct Lod {
    pub ranges: [MeshRange; 9],
    pub model_range: f32,
    pub texture_range: f32,
    pub edge_size: u32,
    pub edge_offset: u32,
    pub polygon_count: u32,
    pub unknown: u32,
    pub vertex_size: u32,
    pub index_size: u32,
    pub vertex_offset: u32,
    pub index_offset: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Submesh {
    /// LOD-relative index-element offset, not mesh-relative.
    pub index_offset: u32,
    pub index_count: u32,
    pub attribute_mask: u32,
    pub bone_start: u16,
    pub bone_count: u16,
}
#[derive(Clone, Copy, Debug)]
pub struct TerrainShadowMesh {
    pub index_count: u32,
    pub start_index: u32,
    pub vertex_offset: u32,
    pub vertex_count: u16,
    pub submesh_start: u16,
    pub submesh_count: u16,
    pub stride: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct TerrainShadowSubmesh {
    pub index_offset: u32,
    pub index_count: u32,
    pub unknown: [u16; 2],
}
/// Borrowed buffers for a terrain-shadow mesh, which uses its own metadata
/// table and no normal-mesh vertex declaration.
#[derive(Clone, Copy, Debug)]
pub struct TerrainShadowGeometry<'a> {
    pub metadata: TerrainShadowMesh,
    vertices: &'a [u8],
    indices: &'a [u8],
}
impl<'a> TerrainShadowGeometry<'a> {
    pub fn raw_vertices(self) -> &'a [u8] {
        self.vertices
    }
    pub fn raw_vertex(self, index: usize) -> Result<&'a [u8]> {
        if index >= usize::from(self.metadata.vertex_count) {
            return Err(Error::OutOfBounds("terrain vertex"));
        }
        record(self.vertices, index, usize::from(self.metadata.stride))
    }
    /// Decode the observed eight-byte half4 layout. XYZ are position lanes;
    /// the fourth stored lane is retained without inventing a homogeneous W.
    /// Other strides remain available through `raw_vertex`.
    pub fn position_half4(self, index: usize) -> Result<[f32; 4]> {
        if self.metadata.stride != 8 {
            return Err(Error::InvalidData("terrain half4 stride"));
        }
        let b = self.raw_vertex(index)?;
        Ok([
            crate::tex::half_to_f32(u16_at(b, 0)?),
            crate::tex::half_to_f32(u16_at(b, 2)?),
            crate::tex::half_to_f32(u16_at(b, 4)?),
            crate::tex::half_to_f32(u16_at(b, 6)?),
        ])
    }
    pub fn indices(self) -> impl ExactSizeIterator<Item = u16> + 'a {
        self.indices
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ElementId {
    pub id: u32,
    pub parent_bone_string_offset: u32,
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct Shape<'a> {
    pub name: &'a [u8],
    pub mesh_starts: [u16; 3],
    pub mesh_counts: [u16; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct ShapeMesh {
    /// Identifies the source mesh by its LOD-relative start index.
    pub mesh_index_offset: u32,
    pub value_count: u32,
    pub value_offset: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShapeValue {
    /// Index occurrence within the mesh, not a vertex number.
    pub base_index: u16,
    pub replacement_vertex: u16,
}
#[derive(Clone, Copy, Debug)]
pub struct BoundingBox {
    pub min: [f32; 4],
    pub max: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct SkinInfluences {
    pub count: u8,
    /// Byte weights divided by 255, without renormalization or bone remapping.
    pub weights: [f32; 8],
    /// Mesh palette-local indices, not indices into the file's bone-name table.
    pub indices: [u16; 8],
}

#[derive(Debug)]
pub struct Mdl<'a> {
    file: FileHeader,
    strings: &'a [u8],
    string_count: u16,
    raw_header: &'a [u8],
    declarations: &'a [u8],
    elements: &'a [u8],
    lods: &'a [u8],
    extra_lods: &'a [u8],
    meshes: &'a [u8],
    attributes: &'a [u8],
    submeshes: &'a [u8],
    terrain_meshes: &'a [u8],
    terrain_submeshes: &'a [u8],
    materials: &'a [u8],
    bones: &'a [u8],
    bone_tables: &'a [u8],
    bone_data: &'a [u8],
    shapes: &'a [u8],
    shape_meshes: &'a [u8],
    shape_values: &'a [u8],
    submesh_bones: &'a [u8],
    runtime_tail: &'a [u8],
    bounds: &'a [u8],
    vertex_buffers: [&'a [u8]; 3],
    index_buffers: [&'a [u8]; 3],
}

impl<'a> Mdl<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }

    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("input"));
        }
        let h = span(bytes, 0, 68)?;
        let version = u32_at(h, 0)?;
        if version != 0x0100_0006 {
            return Err(Error::UnsupportedVersion(version));
        }
        let file = FileHeader {
            version,
            stack_size: u32_at(h, 4)?,
            runtime_size: u32_at(h, 8)?,
            declarations: u16_at(h, 12)?,
            materials: u16_at(h, 14)?,
            vertex_offsets: array3(h, 16)?,
            index_offsets: array3(h, 28)?,
            vertex_sizes: array3(h, 40)?,
            index_sizes: array3(h, 52)?,
            lod_count: h[64],
            streaming: h[65],
            edge_geometry: h[66],
        };
        if !(1..=3).contains(&file.lod_count) || file.streaming > 1 || file.edge_geometry > 1 {
            return Err(Error::InvalidData("LOD count or file flags"));
        }
        let stack = span(bytes, 68, file.stack_size as usize)?;
        let declarations = span(stack, 0, usize::from(file.declarations) * 136)?;
        let runtime_start = 68usize
            .checked_add(stack.len())
            .ok_or(Error::InvalidData("runtime offset overflow"))?;
        let runtime = span(bytes, runtime_start, file.runtime_size as usize)?;
        let mut r = Reader {
            data: runtime,
            pos: 0,
        };
        let strings_header = r.take(1, 8)?;
        let string_count = u16_at(strings_header, 0)?;
        let strings = r.take(u32_at(strings_header, 4)? as usize, 1)?;
        let raw_header = r.take(1, 56)?;
        let count = |offset| -> Result<usize> { Ok(usize::from(u16_at(raw_header, offset)?)) };
        let records = (4..=20)
            .step_by(2)
            .try_fold(usize::from(file.declarations), |total, offset| {
                Ok::<_, Error>(total + count(offset)?)
            })?
            + count(24)?
            + count(38)?
            + usize::from(raw_header[26]);
        if records > limits.max_records {
            return Err(Error::BudgetExceeded("metadata records"));
        }
        if raw_header[22] != file.lod_count || count(10)? != usize::from(file.materials) {
            return Err(Error::InvalidData("conflicting model/file counts"));
        }
        let elements = r.take(count(24)?, 32)?;
        let lods = r.take(3, 60)?;
        let extra_lods = r.take(if raw_header[27] & 16 != 0 { 3 } else { 0 }, 40)?;
        let meshes = r.take(count(4)?, 36)?;
        if count(4)? > usize::from(file.declarations) {
            return Err(Error::InvalidData("mesh has no vertex declaration"));
        }
        let attributes = r.take(count(6)?, 4)?;
        let terrain_meshes = r.take(usize::from(raw_header[26]), 20)?;
        let submeshes = r.take(count(8)?, 16)?;
        let terrain_submeshes = r.take(count(38)?, 12)?;
        let materials = r.take(count(10)?, 4)?;
        let bones = r.take(count(12)?, 4)?;
        let bone_start = r.pos;
        let bone_tables = r.take(count(14)?, 4)?;
        let bone_pool = r.take(count(44)?, 2)?;
        let bone_data = span(runtime, bone_start, bone_tables.len() + bone_pool.len())?;
        let shapes = r.take(count(16)?, 16)?;
        let shape_meshes = r.take(count(18)?, 12)?;
        let shape_values = r.take(count(20)?, 4)?;
        let map_length = u32_at(r.take(1, 4)?, 0)? as usize;
        if map_length % 2 != 0 {
            return Err(Error::InvalidData("odd submesh bone map size"));
        }
        let submesh_bones = r.take(map_length, 1)?;
        let bounds_size = (4 + count(12)? + count(36)?)
            .checked_mul(32)
            .ok_or(Error::InvalidData("bounds size overflow"))?;
        let bounds_start = runtime
            .len()
            .checked_sub(bounds_size)
            .ok_or(Error::InvalidData("missing bounds"))?;
        if r.pos >= bounds_start {
            return Err(Error::InvalidData("runtime tables overlap bounds/padding"));
        }
        let runtime_tail = span(runtime, r.pos, bounds_start - r.pos)?;
        let bounds = span(runtime, bounds_start, bounds_size)?;
        let mut vertex_buffers = [&[][..]; 3];
        let mut index_buffers = [&[][..]; 3];
        let runtime_end = runtime_start + runtime.len();
        for i in 0..3 {
            for (offset, size) in [
                (file.vertex_offsets[i], file.vertex_sizes[i]),
                (file.index_offsets[i], file.index_sizes[i]),
            ] {
                if size != 0 && (offset as usize) < runtime_end {
                    return Err(Error::InvalidData("buffer overlaps metadata"));
                }
                if size != 0 {
                    span(bytes, offset as usize, size as usize)?;
                }
            }
            if file.vertex_sizes[i] != 0 {
                vertex_buffers[i] = span(
                    bytes,
                    file.vertex_offsets[i] as usize,
                    file.vertex_sizes[i] as usize,
                )?;
            }
            if file.index_sizes[i] != 0 {
                index_buffers[i] = span(
                    bytes,
                    file.index_offsets[i] as usize,
                    file.index_sizes[i] as usize,
                )?;
            }
        }
        let model = Self {
            file,
            strings,
            string_count,
            raw_header,
            declarations,
            elements,
            lods,
            extra_lods,
            meshes,
            attributes,
            submeshes,
            terrain_meshes,
            terrain_submeshes,
            materials,
            bones,
            bone_tables,
            bone_data,
            shapes,
            shape_meshes,
            shape_values,
            submesh_bones,
            runtime_tail,
            bounds,
            vertex_buffers,
            index_buffers,
        };
        model.validate(limits)?;
        Ok(model)
    }

    pub fn header(&self) -> FileHeader {
        self.file
    }
    pub fn raw_model_header(&self) -> &'a [u8] {
        self.raw_header
    }
    pub fn string_count(&self) -> u16 {
        self.string_count
    }
    pub fn strings(&self) -> &'a [u8] {
        self.strings
    }
    pub fn radius(&self) -> Result<f32> {
        f32_at(self.raw_header, 0)
    }
    pub fn runtime_tail(&self) -> &'a [u8] {
        self.runtime_tail
    }
    pub fn terrain_shadow_mesh_records(&self) -> &'a [u8] {
        self.terrain_meshes
    }
    pub fn terrain_shadow_submesh_records(&self) -> &'a [u8] {
        self.terrain_submeshes
    }
    pub fn terrain_shadow_mesh(&self, index: usize) -> Result<TerrainShadowMesh> {
        let b = record(self.terrain_meshes, index, 20)?;
        Ok(TerrainShadowMesh {
            index_count: u32_at(b, 0)?,
            start_index: u32_at(b, 4)?,
            vertex_offset: u32_at(b, 8)?,
            vertex_count: u16_at(b, 12)?,
            submesh_start: u16_at(b, 14)?,
            submesh_count: u16_at(b, 16)?,
            stride: b[18],
        })
    }
    pub fn terrain_shadow_submesh(&self, index: usize) -> Result<TerrainShadowSubmesh> {
        let b = record(self.terrain_submeshes, index, 12)?;
        Ok(TerrainShadowSubmesh {
            index_offset: u32_at(b, 0)?,
            index_count: u32_at(b, 4)?,
            unknown: [u16_at(b, 8)?, u16_at(b, 10)?],
        })
    }
    /// Select from the LOD's terrain range, not its normal mesh/declaration table.
    pub fn terrain_shadow_geometry(
        &self,
        lod: usize,
        index: usize,
    ) -> Result<TerrainShadowGeometry<'a>> {
        let range = self.lod(lod)?.ranges[3];
        let start = usize::from(range.start);
        let end = start
            .checked_add(usize::from(range.count))
            .ok_or(Error::InvalidData("terrain range overflow"))?;
        if index < start || index >= end {
            return Err(Error::OutOfBounds("terrain LOD mesh"));
        }
        let metadata = self.terrain_shadow_mesh(index)?;
        let size = usize::from(metadata.vertex_count)
            .checked_mul(usize::from(metadata.stride))
            .ok_or(Error::InvalidData("terrain vertex size overflow"))?;
        let vertices = span(
            self.vertex_buffer(lod)?,
            usize::try_from(metadata.vertex_offset)
                .map_err(|_| Error::InvalidData("terrain vertex offset"))?,
            size,
        )?;
        let start = usize::try_from(metadata.start_index)
            .ok()
            .and_then(|v| v.checked_mul(2))
            .ok_or(Error::InvalidData("terrain index offset"))?;
        let size = usize::try_from(metadata.index_count)
            .ok()
            .and_then(|v| v.checked_mul(2))
            .ok_or(Error::InvalidData("terrain index size"))?;
        let indices = span(self.index_buffer(lod)?, start, size)?;
        Ok(TerrainShadowGeometry {
            metadata,
            vertices,
            indices,
        })
    }
    pub fn vertex_buffer(&self, lod: usize) -> Result<&'a [u8]> {
        self.lod(lod)?;
        Ok(self.vertex_buffers[lod])
    }
    pub fn index_buffer(&self, lod: usize) -> Result<&'a [u8]> {
        self.lod(lod)?;
        Ok(self.index_buffers[lod])
    }
    pub fn mesh_count(&self) -> usize {
        self.meshes.len() / 36
    }
    pub fn submesh_count(&self) -> usize {
        self.submeshes.len() / 16
    }
    pub fn attribute_count(&self) -> usize {
        self.attributes.len() / 4
    }
    pub fn material_count(&self) -> usize {
        self.materials.len() / 4
    }
    pub fn bone_count(&self) -> usize {
        self.bones.len() / 4
    }
    pub fn bone_table_count(&self) -> usize {
        self.bone_tables.len() / 4
    }
    pub fn shape_count(&self) -> usize {
        self.shapes.len() / 16
    }
    pub fn shape_mesh_count(&self) -> usize {
        self.shape_meshes.len() / 12
    }
    pub fn shape_value_count(&self) -> usize {
        self.shape_values.len() / 4
    }
    pub fn element_id_count(&self) -> usize {
        self.elements.len() / 32
    }
    pub fn submesh_bone_count(&self) -> usize {
        self.submesh_bones.len() / 2
    }

    pub fn string(&self, offset: u32) -> Result<&'a [u8]> {
        let suffix = self
            .strings
            .get(offset as usize..)
            .ok_or(Error::InvalidData("string offset"))?;
        let end = suffix
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::InvalidData("unterminated string"))?;
        Ok(&suffix[..end])
    }
    fn name(&self, table: &[u8], index: usize) -> Result<&'a [u8]> {
        self.string(u32_at(record(table, index, 4)?, 0)?)
    }
    pub fn material(&self, index: usize) -> Result<&'a [u8]> {
        self.name(self.materials, index)
    }
    pub fn attribute(&self, index: usize) -> Result<&'a [u8]> {
        self.name(self.attributes, index)
    }
    /// Resolve a raw name to this file's submesh-attribute bit. None confirms
    /// that the name is absent; budget exhaustion and duplicate names are errors.
    /// A matching table index >=32 cannot be represented by the u32 mask and
    /// returns an error; the raw name table remains accessible with `attribute`.
    /// Does not decide which attributes a character should enable or hide.
    pub fn attribute_bit(&self, name: &[u8], max_work: usize) -> Result<Option<u32>> {
        let mut work = Work(max_work);
        work.charge(name.len())?;
        let mut found = None;
        for index in 0..self.attribute_count() {
            work.charge(1)?;
            let offset = usize::try_from(u32_at(self.attributes, index * 4)?)
                .map_err(|_| Error::InvalidData("attribute string offset"))?;
            let tail = self
                .strings
                .get(offset..)
                .ok_or(Error::InvalidData("attribute string offset"))?;
            let scan = tail.len().min(work.0);
            let end = tail[..scan]
                .iter()
                .position(|b| *b == 0)
                .ok_or(if scan < tail.len() {
                    Error::BudgetExceeded("validation work")
                } else {
                    Error::InvalidData("attribute string terminator")
                })?;
            work.charge(end + 1)?;
            if end == name.len() {
                work.charge(end)?;
                if &tail[..end] == name {
                    let bit = 1u32
                        .checked_shl(
                            u32::try_from(index)
                                .map_err(|_| Error::OutOfBounds("attribute bit index"))?,
                        )
                        .ok_or(Error::OutOfBounds("attribute bit index"))?;
                    if found.replace(bit).is_some() {
                        return Err(Error::InvalidData("ambiguous attribute name"));
                    }
                }
            }
        }
        Ok(found)
    }
    pub fn bone_name(&self, index: usize) -> Result<&'a [u8]> {
        self.name(self.bones, index)
    }
    pub fn mesh_metadata(&self, index: usize) -> Result<Mesh> {
        Mesh::parse(record(self.meshes, index, 36)?)
    }

    pub fn declaration(&self, mesh: usize) -> Result<impl Iterator<Item = Element> + 'a> {
        let declaration = record(self.declarations, mesh, 136)?;
        if !declaration.chunks_exact(8).any(|b| b[0] == 255) {
            return Err(Error::InvalidData("unterminated vertex declaration"));
        }
        Ok(declaration
            .chunks_exact(8)
            .take_while(|b| b[0] != 255)
            .map(|b| Element {
                stream: b[0],
                offset: b[1],
                kind: b[2],
                usage: b[3],
                usage_index: b[4],
            }))
    }
    pub fn lod(&self, index: usize) -> Result<Lod> {
        if index >= usize::from(self.file.lod_count) {
            return Err(Error::OutOfBounds("LOD"));
        }
        let b = record(self.lods, index, 60)?;
        let extra = if self.extra_lods.is_empty() {
            &[][..]
        } else {
            record(self.extra_lods, index, 40)?
        };
        let roles = [
            MeshRole::Normal,
            MeshRole::Water,
            MeshRole::Shadow,
            MeshRole::TerrainShadow,
            MeshRole::VerticalFog,
            MeshRole::LightShaft,
            MeshRole::Glass,
            MeshRole::MaterialChange,
            MeshRole::CrestChange,
        ];
        let mut ranges = roles.map(|role| MeshRange {
            role,
            start: 0,
            count: 0,
        });
        for (i, range) in ranges.iter_mut().enumerate() {
            if i < 5 {
                let offset = if i == 0 { 0 } else { 12 + (i - 1) * 4 };
                range.start = u16_at(b, offset)?;
                range.count = u16_at(b, offset + 2)?;
            } else if !extra.is_empty() {
                range.start = u16_at(extra, (i - 5) * 4)?;
                range.count = u16_at(extra, (i - 5) * 4 + 2)?;
            }
        }
        Ok(Lod {
            ranges,
            model_range: f32_at(b, 4)?,
            texture_range: f32_at(b, 8)?,
            edge_size: u32_at(b, 28)?,
            edge_offset: u32_at(b, 32)?,
            polygon_count: u32_at(b, 36)?,
            unknown: u32_at(b, 40)?,
            vertex_size: u32_at(b, 44)?,
            index_size: u32_at(b, 48)?,
            vertex_offset: u32_at(b, 52)?,
            index_offset: u32_at(b, 56)?,
        })
    }
    pub fn submesh(&self, index: usize) -> Result<Submesh> {
        let b = record(self.submeshes, index, 16)?;
        Ok(Submesh {
            index_offset: u32_at(b, 0)?,
            index_count: u32_at(b, 4)?,
            attribute_mask: u32_at(b, 8)?,
            bone_start: u16_at(b, 12)?,
            bone_count: u16_at(b, 14)?,
        })
    }
    pub fn bone_table(&self, index: usize) -> Result<impl ExactSizeIterator<Item = u16> + 'a> {
        let b = record(self.bone_tables, index, 4)?;
        let offset = usize::from(u16_at(b, 0)?) * 4;
        let start = index
            .checked_mul(4)
            .and_then(|base| base.checked_add(offset))
            .ok_or(Error::InvalidData("bone table offset overflow"))?;
        if start < self.bone_tables.len() {
            return Err(Error::InvalidData("bone palette overlaps descriptors"));
        }
        let data = span(self.bone_data, start, usize::from(u16_at(b, 2)?) * 2)?;
        Ok(data
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]])))
    }
    pub fn submesh_bone(&self, index: usize) -> Result<u16> {
        u16_at(record(self.submesh_bones, index, 2)?, 0)
    }
    pub fn element_id(&self, index: usize) -> Result<ElementId> {
        let b = record(self.elements, index, 32)?;
        Ok(ElementId {
            id: u32_at(b, 0)?,
            parent_bone_string_offset: u32_at(b, 4)?,
            translation: [f32_at(b, 8)?, f32_at(b, 12)?, f32_at(b, 16)?],
            rotation: [f32_at(b, 20)?, f32_at(b, 24)?, f32_at(b, 28)?],
        })
    }
    pub fn shape(&self, index: usize) -> Result<Shape<'a>> {
        let b = record(self.shapes, index, 16)?;
        Ok(Shape {
            name: self.string(u32_at(b, 0)?)?,
            mesh_starts: [u16_at(b, 4)?, u16_at(b, 6)?, u16_at(b, 8)?],
            mesh_counts: [u16_at(b, 10)?, u16_at(b, 12)?, u16_at(b, 14)?],
        })
    }
    pub fn shape_mesh(&self, index: usize) -> Result<ShapeMesh> {
        let b = record(self.shape_meshes, index, 12)?;
        Ok(ShapeMesh {
            mesh_index_offset: u32_at(b, 0)?,
            value_count: u32_at(b, 4)?,
            value_offset: u32_at(b, 8)?,
        })
    }
    pub fn shape_value(&self, index: usize) -> Result<ShapeValue> {
        let b = record(self.shape_values, index, 4)?;
        Ok(ShapeValue {
            base_index: u16_at(b, 0)?,
            replacement_vertex: u16_at(b, 2)?,
        })
    }
    pub fn bounding_box_count(&self) -> usize {
        self.bounds.len() / 32
    }

    /// File/global, model, water, vertical-fog, bone boxes and additional boxes.
    /// The association of the additional boxes (count at model-header +0x24)
    /// is not established; their numeric payload is preserved without guessing.
    pub fn bounding_box(&self, index: usize) -> Result<BoundingBox> {
        let b = record(self.bounds, index, 32)?;
        let mut min = [0.; 4];
        let mut max = [0.; 4];
        for i in 0..4 {
            min[i] = f32_at(b, i * 4)?;
            max[i] = f32_at(b, 16 + i * 4)?;
        }
        Ok(BoundingBox { min, max })
    }

    pub fn geometry(&self, lod: usize, mesh: usize) -> Result<Geometry<'_, 'a>> {
        let level = self.lod(lod)?;
        if !level
            .ranges
            .iter()
            .filter(|r| r.role != MeshRole::TerrainShadow)
            .any(|r| {
                mesh >= usize::from(r.start) && mesh < usize::from(r.start) + usize::from(r.count)
            })
        {
            return Err(Error::OutOfBounds("mesh in LOD"));
        }
        self.geometry_unchecked_role(lod, mesh)
    }
    fn geometry_unchecked_role(&self, lod: usize, mesh: usize) -> Result<Geometry<'_, 'a>> {
        Ok(Geometry {
            model: self,
            lod,
            mesh_index: mesh,
            mesh: self.mesh_metadata(mesh)?,
        })
    }

    fn validate(&self, limits: Limits) -> Result<()> {
        let mut work = Work(limits.max_validation_steps);
        let check_string = |offset: u32, work: &mut Work| -> Result<()> {
            let suffix = self
                .strings
                .get(offset as usize..)
                .ok_or(Error::InvalidData("string offset"))?;
            let length = match suffix.iter().take(work.0).position(|&b| b == 0) {
                Some(length) => length,
                None if suffix.len() > work.0 => {
                    return Err(Error::BudgetExceeded("string validation"));
                }
                None => return Err(Error::InvalidData("unterminated string")),
            };
            work.charge(length + 1)
        };
        for table in [self.attributes, self.materials, self.bones] {
            for b in table.chunks_exact(4) {
                check_string(u32_at(b, 0)?, &mut work)?;
            }
        }
        for i in 0..self.element_id_count() {
            check_string(self.element_id(i)?.parent_bone_string_offset, &mut work)?;
        }
        for b in self.shapes.chunks_exact(16) {
            check_string(u32_at(b, 0)?, &mut work)?;
        }
        for i in 0..self.bone_table_count() {
            let palette = self.bone_table(i)?;
            work.charge(palette.len())?;
            for bone in palette {
                if usize::from(bone) >= self.bone_count() {
                    return Err(Error::InvalidData("bone palette index"));
                }
            }
        }
        for i in 0..self.mesh_count() {
            let mesh = self.mesh_metadata(i)?;
            if mesh.stream_count > 3 || usize::from(mesh.material_index) >= self.material_count() {
                return Err(Error::InvalidData("mesh stream or material"));
            }
            if self.bone_table_count() != 0
                && mesh.bone_table_index != u16::MAX
                && usize::from(mesh.bone_table_index) >= self.bone_table_count()
            {
                return Err(Error::InvalidData("mesh bone table"));
            }
            range(
                usize::from(mesh.submesh_start),
                usize::from(mesh.submesh_count),
                self.submesh_count(),
            )?;
            for e in self.declaration(i)? {
                if e.stream >= mesh.stream_count || e.stream >= 3 {
                    return Err(Error::InvalidData("vertex stream"));
                }
                let width = match e.byte_width() {
                    Ok(w) => w,
                    Err(Error::UnsupportedVertexType(_)) => 1,
                    Err(e) => return Err(e),
                };
                if usize::from(e.offset) + width
                    > usize::from(mesh.stream_strides[usize::from(e.stream)])
                {
                    return Err(Error::InvalidData("vertex element exceeds stride"));
                }
            }
            for s in usize::from(mesh.submesh_start)
                ..usize::from(mesh.submesh_start) + usize::from(mesh.submesh_count)
            {
                work.charge(1)?;
                let sub = self.submesh(s)?;
                if sub.index_offset < mesh.start_index
                    || u64::from(sub.index_offset) + u64::from(sub.index_count)
                        > u64::from(mesh.start_index) + u64::from(mesh.index_count)
                {
                    return Err(Error::InvalidData("submesh index range"));
                }
                range(
                    usize::from(sub.bone_start),
                    usize::from(sub.bone_count),
                    self.submesh_bone_count(),
                )?;
                work.charge(usize::from(sub.bone_count))?;
                for index in usize::from(sub.bone_start)
                    ..usize::from(sub.bone_start) + usize::from(sub.bone_count)
                {
                    if usize::from(self.submesh_bone(index)?) >= self.bone_count() {
                        return Err(Error::InvalidData("submesh bone index"));
                    }
                }
            }
        }
        let mut indices_left = limits.max_indices;
        for l in 0..usize::from(self.file.lod_count) {
            let lod = self.lod(l)?;
            if lod.vertex_offset != self.file.vertex_offsets[l]
                || lod.index_offset != self.file.index_offsets[l]
                || lod.vertex_size != self.file.vertex_sizes[l]
                || lod.index_size != self.file.index_sizes[l]
            {
                return Err(Error::InvalidData("LOD/file buffer disagreement"));
            }
            for r in lod.ranges {
                let total = if r.role == MeshRole::TerrainShadow {
                    self.terrain_meshes.len() / 20
                } else {
                    self.mesh_count()
                };
                if r.count != 0 {
                    range(usize::from(r.start), usize::from(r.count), total)?;
                }
                if r.role == MeshRole::TerrainShadow && r.count != 0 {
                    for index in usize::from(r.start)..usize::from(r.start) + usize::from(r.count) {
                        work.charge(1)?;
                        let mesh = self.terrain_shadow_mesh(index)?;
                        span(
                            self.vertex_buffers[l],
                            mesh.vertex_offset as usize,
                            usize::from(mesh.vertex_count) * usize::from(mesh.stride),
                        )?;
                        let index_start = (mesh.start_index as usize)
                            .checked_mul(2)
                            .ok_or(Error::InvalidData("terrain index offset overflow"))?;
                        let index_bytes = (mesh.index_count as usize)
                            .checked_mul(2)
                            .ok_or(Error::InvalidData("terrain index size overflow"))?;
                        let data = span(self.index_buffers[l], index_start, index_bytes)?;
                        indices_left = indices_left
                            .checked_sub(mesh.index_count as usize)
                            .ok_or(Error::BudgetExceeded("terrain index validation"))?;
                        work.charge(mesh.index_count as usize)?;
                        for b in data.chunks_exact(2) {
                            if u16::from_le_bytes([b[0], b[1]]) >= mesh.vertex_count {
                                return Err(Error::InvalidData("terrain index exceeds vertices"));
                            }
                        }
                        let submeshes = range(
                            usize::from(mesh.submesh_start),
                            usize::from(mesh.submesh_count),
                            self.terrain_submeshes.len() / 12,
                        )?;
                        work.charge(submeshes.len())?;
                        for sub in submeshes {
                            let sub = self.terrain_shadow_submesh(sub)?;
                            if sub.index_offset < mesh.start_index
                                || u64::from(sub.index_offset) + u64::from(sub.index_count)
                                    > u64::from(mesh.start_index) + u64::from(mesh.index_count)
                            {
                                return Err(Error::InvalidData("terrain submesh index range"));
                            }
                        }
                    }
                }
            }
            for m in 0..self.mesh_count() {
                if !lod
                    .ranges
                    .iter()
                    .filter(|r| r.role != MeshRole::TerrainShadow)
                    .any(|r| {
                        m >= usize::from(r.start) && m < usize::from(r.start) + usize::from(r.count)
                    })
                {
                    continue;
                }
                let geometry = self.geometry_unchecked_role(l, m)?;
                let mesh = geometry.mesh;
                indices_left = indices_left
                    .checked_sub(mesh.index_count as usize)
                    .ok_or(Error::BudgetExceeded("index validation"))?;
                work.charge(mesh.index_count as usize)?;
                for s in 0..usize::from(mesh.stream_count) {
                    let size = usize::from(mesh.vertex_count)
                        .checked_mul(usize::from(mesh.stream_strides[s]))
                        .ok_or(Error::InvalidData("vertex stream size overflow"))?;
                    span(
                        self.vertex_buffers[l],
                        mesh.stream_offsets[s] as usize,
                        size,
                    )?;
                }
                for index in geometry.indices()? {
                    if index >= mesh.vertex_count {
                        return Err(Error::InvalidData("index exceeds mesh vertex count"));
                    }
                }
            }
            for s in 0..self.shape_count() {
                work.charge(1)?;
                let shape = self.shape(s)?;
                let refs = range(
                    usize::from(shape.mesh_starts[l]),
                    usize::from(shape.mesh_counts[l]),
                    self.shape_mesh_count(),
                )?;
                for reference in refs {
                    work.charge(1)?;
                    let sm = self.shape_mesh(reference)?;
                    let values = range(
                        sm.value_offset as usize,
                        sm.value_count as usize,
                        self.shape_value_count(),
                    )?;
                    let mut found = false;
                    for m in 0..self.mesh_count() {
                        work.charge(1)?;
                        if let Ok(g) = self.geometry(l, m) {
                            if g.mesh.start_index != sm.mesh_index_offset {
                                continue;
                            }
                            found = true;
                            work.charge(values.len())?;
                            for v in values.clone() {
                                let value = self.shape_value(v)?;
                                if u32::from(value.base_index) >= g.mesh.index_count
                                    || value.replacement_vertex >= g.mesh.vertex_count
                                {
                                    return Err(Error::InvalidData(
                                        "shape occurrence or replacement vertex",
                                    ));
                                }
                            }
                        }
                    }
                    if !found {
                        return Err(Error::InvalidData("shape mesh has no source mesh"));
                    }
                }
            }
        }
        Ok(())
    }
}

/// A mesh view with LOD-relative streams and checked index access.
#[derive(Clone, Copy, Debug)]
pub struct Geometry<'model, 'data> {
    model: &'model Mdl<'data>,
    lod: usize,
    mesh_index: usize,
    mesh: Mesh,
}
impl<'data> Geometry<'_, 'data> {
    pub fn metadata(&self) -> Mesh {
        self.mesh
    }
    pub fn declaration(&self) -> Result<impl Iterator<Item = Element> + 'data> {
        self.model.declaration(self.mesh_index)
    }
    pub fn indices(&self) -> Result<impl ExactSizeIterator<Item = u16> + 'data> {
        let start = (self.mesh.start_index as usize)
            .checked_mul(2)
            .ok_or(Error::InvalidData("index offset overflow"))?;
        let size = (self.mesh.index_count as usize)
            .checked_mul(2)
            .ok_or(Error::InvalidData("index size overflow"))?;
        let data = span(self.model.index_buffers[self.lod], start, size)?;
        Ok(data
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]])))
    }
    pub fn raw_vertex(&self, vertex: usize, stream: u8) -> Result<&'data [u8]> {
        if vertex >= usize::from(self.mesh.vertex_count)
            || stream >= self.mesh.stream_count
            || stream >= 3
        {
            return Err(Error::OutOfBounds("vertex or stream"));
        }
        let s = usize::from(stream);
        let stride = usize::from(self.mesh.stream_strides[s]);
        let offset = vertex
            .checked_mul(stride)
            .and_then(|v| v.checked_add(self.mesh.stream_offsets[s] as usize))
            .ok_or(Error::InvalidData("vertex offset overflow"))?;
        span(self.model.vertex_buffers[self.lod], offset, stride)
    }
    pub fn vertex_element(&self, vertex: usize, element: Element) -> Result<[f32; 8]> {
        let b = self.raw_vertex(vertex, element.stream)?;
        let data = b
            .get(usize::from(element.offset)..)
            .ok_or(Error::InvalidData("vertex element offset"))?;
        element.decode(data)
    }
    fn unique_element(&self, usage: u8, usage_index: u8) -> Result<Option<Element>> {
        let mut found = None;
        for e in self.declaration()? {
            if e.usage == usage && e.usage_index == usage_index {
                if found.is_some() {
                    return Err(Error::InvalidData("ambiguous vertex semantic"));
                }
                found = Some(e);
            }
        }
        Ok(found)
    }
    /// Look up a unique `(usage, usage_index)` without merging channels. Duplicate
    /// semantics are an error for this query; raw declarations remain accessible.
    /// An invalid vertex is an error even when the semantic is absent.
    /// Usage 5 carries the tangent/flow input (TANGENT0 in the verified Win32
    /// input path); usage 6 carries BINORMAL0. Values retain their declaration's
    /// scalar encoding: this query does not remap, normalize or interpret them
    /// using a shader formula. Preserving a nonzero raw usage index does not
    /// assert a corresponding native TANGENT1 or BINORMAL1 input.
    pub fn attribute(&self, vertex: usize, usage: u8, usage_index: u8) -> Result<Option<[f32; 8]>> {
        if vertex >= usize::from(self.mesh.vertex_count) {
            return Err(Error::OutOfBounds("vertex"));
        }
        self.unique_element(usage, usage_index)?
            .map(|e| self.vertex_element(vertex, e))
            .transpose()
    }
    pub fn decode_attribute(
        &self,
        element: Element,
        max_output_bytes: usize,
    ) -> Result<Vec<[f32; 8]>> {
        let count = usize::from(self.mesh.vertex_count);
        if count * 32 > max_output_bytes {
            return Err(Error::BudgetExceeded("vertex output"));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        for v in 0..count {
            output.push(self.vertex_element(v, element)?);
        }
        Ok(output)
    }
    /// Decode the usage-index-zero skin pair. Both absent means no such pair;
    /// a lone member or duplicate semantic is an error, not confirmed absence.
    pub fn skin_influences(&self, vertex: usize) -> Result<Option<SkinInfluences>> {
        if vertex >= usize::from(self.mesh.vertex_count) {
            return Err(Error::OutOfBounds("vertex"));
        }
        let (w, i) = match (self.unique_element(1, 0)?, self.unique_element(2, 0)?) {
            (None, None) => return Ok(None),
            (Some(w), Some(i)) => (w, i),
            _ => return Err(Error::InvalidData("incomplete skin influence pair")),
        };
        if !matches!(w.kind, 5 | 8 | 17)
            || !matches!(i.kind, 5 | 17)
            || w.byte_width()? != i.byte_width()?
        {
            return Err(Error::InvalidData("unsupported skin influence layout"));
        }
        let mut weights = self.vertex_element(vertex, w)?;
        if w.kind != 8 {
            for value in &mut weights {
                *value /= 255.;
            }
        }
        let raw_indices = self.vertex_element(vertex, i)?;
        Ok(Some(SkinInfluences {
            count: w.byte_width()? as u8,
            weights,
            indices: raw_indices.map(|v| v as u16),
        }))
    }
    /// Looks up the original vertex selected by an index occurrence. This is the
    /// source of a shape replacement, and may differ for repeated vertex IDs.
    pub fn vertex_for_occurrence(&self, occurrence: usize) -> Result<u16> {
        self.indices()?
            .nth(occurrence)
            .ok_or(Error::OutOfBounds("index occurrence"))
    }

    pub fn shape_meshes(
        &self,
        shape: usize,
    ) -> Result<impl Iterator<Item = Result<ShapeMesh>> + '_> {
        let shape = self.model.shape(shape)?;
        let refs = range(
            usize::from(shape.mesh_starts[self.lod]),
            usize::from(shape.mesh_counts[self.lod]),
            self.model.shape_mesh_count(),
        )?;
        Ok(refs.filter_map(move |i| match self.model.shape_mesh(i) {
            Ok(value) if value.mesh_index_offset == self.mesh.start_index => Some(Ok(value)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        }))
    }

    /// Replacement minus the source index occurrence's attribute. This does not
    /// split shared vertices, apply shapes or normalize direction vectors.
    pub fn shape_delta(&self, value: ShapeValue, element: Element) -> Result<[f32; 8]> {
        let source = self.vertex_for_occurrence(usize::from(value.base_index))?;
        let a = self.vertex_element(usize::from(source), element)?;
        let b = self.vertex_element(usize::from(value.replacement_vertex), element)?;
        Ok(std::array::from_fn(|i| b[i] - a[i]))
    }
}

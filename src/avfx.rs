//! Byte-only AVFX asset parsing. Block names are exposed in logical (reversed
//! wire) order including NUL padding. Unknown payloads are retained, not guessed
//! to be nested blocks. No particle simulation, GPU work or host binding.
use crate::binary::u32_at;
use std::fmt;
mod assets;
pub use assets::{
    AssetLimits, AssetReport, DataKind, DataView, EmitterItem, Parameter, ParameterValue,
    Reference, ReferenceKind, ReferenceStatus, TextureSlot,
};
mod binder;
pub use binder::{BinderProperty, BinderSlot, RingParameters};
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    BudgetExceeded,
    AllocationFailed,
}
impl From<&'static str> for Error {
    fn from(v: &'static str) -> Self {
        Self::InvalidData(v)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(v) => write!(f, "invalid AVFX: {v}"),
            Self::BudgetExceeded => f.write_str("AVFX resource budget exceeded"),
            Self::AllocationFailed => f.write_str("AVFX allocation failed"),
        }
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_blocks: usize,
    pub max_allocation_bytes: usize,
    pub max_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 << 20,
            max_blocks: 1_000_000,
            max_allocation_bytes: 64 << 20,
            max_work: 16_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Block<'a> {
    pub name: [u8; 4],
    pub payload: &'a [u8],
    pub offset: usize,
    pub padding: &'a [u8],
}
impl<'a> Block<'a> {
    pub fn is(self, name: &[u8]) -> bool {
        let start = self.name.iter().position(|v| *v != 0).unwrap_or(4);
        let end = self
            .name
            .iter()
            .rposition(|v| *v != 0)
            .map_or(start, |v| v + 1);
        self.name[start..end] == *name
    }
    pub fn u32(self) -> Result<u32> {
        if self.payload.len() != 4 {
            return Err("u32 payload size".into());
        }
        Ok(u32_at(self.payload, 0)?)
    }
    pub fn byte(self) -> Result<u8> {
        if self.payload.len() != 1 {
            return Err("byte payload size".into());
        }
        Ok(self.payload[0])
    }
    pub fn i32(self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn f32(self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn string(self) -> Result<&'a [u8]> {
        let end = self
            .payload
            .iter()
            .position(|v| *v == 0)
            .ok_or("unterminated string")?;
        if self.payload[end..].iter().any(|v| *v != 0) {
            return Err("nonzero string suffix".into());
        }
        Ok(&self.payload[..end])
    }
    /// Explicit interpretation as a block stream; arbitrary opaque payloads are
    /// never recursively classified by their first few bytes.
    pub fn fields(self, max_blocks: usize) -> Result<Fields<'a>> {
        Fields::parse(
            self.payload,
            self.offset.checked_add(8).ok_or("field origin overflow")?,
            max_blocks,
        )
    }
}
/// Validated borrowed sequence. Iteration cannot produce a partial-success
/// prefix of a malformed stream. Offsets remain absolute within the AVFX file.
#[derive(Clone, Copy, Debug)]
pub struct Fields<'a> {
    bytes: &'a [u8],
    origin: usize,
    count: usize,
}
fn block_at(bytes: &[u8], pos: usize, origin: usize) -> Result<(Block<'_>, usize)> {
    let header = bytes
        .get(pos..pos.checked_add(8).ok_or("block header overflow")?)
        .ok_or("truncated block header")?;
    let size = usize::try_from(u32_at(header, 4)?).map_err(|_| "block size overflow")?;
    let start = pos + 8;
    let end = start.checked_add(size).ok_or("block payload overflow")?;
    let next = end
        .checked_add(size.wrapping_neg() & 3)
        .ok_or("block alignment overflow")?;
    let payload = bytes.get(start..end).ok_or("truncated block payload")?;
    let padding = bytes.get(end..next).ok_or("truncated block padding")?;
    Ok((
        Block {
            name: [header[3], header[2], header[1], header[0]],
            payload,
            offset: origin.checked_add(pos).ok_or("block origin overflow")?,
            padding,
        },
        next,
    ))
}
impl<'a> Fields<'a> {
    fn parse(bytes: &'a [u8], origin: usize, max: usize) -> Result<Self> {
        let mut p = 0;
        let mut count = 0;
        while p < bytes.len() {
            if count >= max {
                return Err(Error::BudgetExceeded);
            }
            let (_, next) = block_at(bytes, p, origin)?;
            p = next;
            count += 1;
        }
        Ok(Self {
            bytes,
            origin,
            count,
        })
    }
    pub fn len(self) -> usize {
        self.count
    }
    pub fn is_empty(self) -> bool {
        self.count == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = Block<'a>> {
        FieldIter {
            fields: self,
            pos: 0,
            left: self.count,
        }
    }
    pub fn named(self, name: &'a [u8]) -> impl Iterator<Item = Block<'a>> {
        self.iter().filter(move |v| v.is(name))
    }
    pub fn one(self, name: &[u8]) -> Result<Option<Block<'a>>> {
        let mut result = None;
        for b in self.iter() {
            if b.is(name) {
                if result.is_some() {
                    return Err("duplicate singleton block".into());
                }
                result = Some(b);
            }
        }
        Ok(result)
    }
    pub fn integer(self, name: &[u8]) -> Result<Option<i32>> {
        self.one(name)?.map(Block::i32).transpose()
    }
    pub fn real(self, name: &[u8]) -> Result<Option<f32>> {
        self.one(name)?.map(Block::f32).transpose()
    }
}
struct FieldIter<'a> {
    fields: Fields<'a>,
    pos: usize,
    left: usize,
}
impl<'a> Iterator for FieldIter<'a> {
    type Item = Block<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        let (b, next) = block_at(self.fields.bytes, self.pos, self.fields.origin).ok()?;
        self.pos = next;
        self.left -= 1;
        Some(b)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}
impl ExactSizeIterator for FieldIter<'_> {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Scheduler,
    Timeline,
    Emitter,
    Particle,
    Effector,
    Binder,
}
impl Kind {
    fn tag(self) -> &'static [u8] {
        match self {
            Self::Scheduler => b"Schd",
            Self::Timeline => b"TmLn",
            Self::Emitter => b"Emit",
            Self::Particle => b"Ptcl",
            Self::Effector => b"Efct",
            Self::Binder => b"Bind",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Node<'a> {
    pub kind: Kind,
    pub block: Block<'a>,
    pub fields: Fields<'a>,
}
impl<'a> Node<'a> {
    pub fn timeline_items(self, max_blocks: usize) -> Result<Vec<TimelineItem>> {
        if self.kind != Kind::Timeline {
            return Err("not a timeline".into());
        }
        let mut last = None;
        for b in self.fields.iter() {
            if b.is(b"Item") {
                last = Some(b);
            }
        }
        let Some(last) = last else {
            return Ok(Vec::new());
        };
        let fields = last.fields(max_blocks)?;
        let mut out = Vec::new();
        out.try_reserve_exact(fields.len() / 7)
            .map_err(|_| Error::AllocationFailed)?;
        let mut row = [None; 8];
        let mut active = false;
        for b in fields.iter() {
            if b.is(b"bEna") && active {
                out.push(finish_timeline(row)?);
                row = [None; 8];
            }
            let index = [
                b"bEna", b"StTm", b"EdTm", b"BdNo", b"EfNo", b"EmNo", b"Plfm", b"ClNo",
            ]
            .iter()
            .position(|tag| b.is(*tag))
            .ok_or("unknown timeline item field")?;
            if !active && index != 0 {
                return Err("timeline field before enabled".into());
            }
            active = true;
            if row[index].replace(b.i32()?).is_some() {
                return Err("duplicate timeline item field".into());
            }
        }
        if active {
            out.push(finish_timeline(row)?);
        }
        Ok(out)
    }
    /// Type discriminator preserved numerically; no fallback simulation type.
    pub fn variant(self) -> Result<Option<i32>> {
        self.fields.integer(match self.kind {
            Kind::Emitter => b"EVT",
            Kind::Particle => b"PrVT",
            Kind::Effector => b"EfVT",
            Kind::Binder => b"BnVr",
            _ => return Ok(None),
        })
    }
    /// Read the final cumulative scheduler Item/Trgr record, splitting at bEna.
    /// Earlier blocks retain their original cumulative bytes in `fields`.
    pub fn scheduler_items(self, triggers: bool, max_blocks: usize) -> Result<Vec<ScheduleItem>> {
        if self.kind != Kind::Scheduler {
            return Err("not a scheduler".into());
        }
        let name = if triggers { b"Trgr" } else { b"Item" };
        let mut last = None;
        for b in self.fields.iter() {
            if b.is(name) {
                last = Some(b);
            }
        }
        let Some(last) = last else {
            return Ok(Vec::new());
        };
        let f = last.fields(max_blocks)?;
        let mut out = Vec::new();
        out.try_reserve_exact(f.len() / 3)
            .map_err(|_| Error::AllocationFailed)?;
        let mut row = None;
        for b in f.iter() {
            if b.is(b"bEna") {
                if let Some(r) = row.take() {
                    out.push(finish_schedule(r)?);
                }
                row = Some((Some(b.i32()?), None, None));
            } else if b.is(b"StTm") {
                let r = row.as_mut().ok_or("scheduler field before enabled")?;
                if r.1.replace(b.i32()?).is_some() {
                    return Err("duplicate scheduler start".into());
                }
            } else if b.is(b"TlNo") {
                let r = row.as_mut().ok_or("scheduler field before enabled")?;
                if r.2.replace(b.i32()?).is_some() {
                    return Err("duplicate scheduler timeline".into());
                }
            } else {
                return Err("unknown scheduler item field".into());
            }
        }
        if let Some(r) = row {
            out.push(finish_schedule(r)?);
        }
        Ok(out)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScheduleItem {
    pub enabled: i32,
    pub start_time: i32,
    pub timeline_index: i32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimelineItem {
    pub enabled: i32,
    pub start_time: i32,
    pub end_time: i32,
    pub binder_index: i32,
    pub effector_index: i32,
    pub emitter_index: i32,
    pub platform: i32,
    pub clip_index: Option<i32>,
}
fn finish_timeline(r: [Option<i32>; 8]) -> Result<TimelineItem> {
    Ok(TimelineItem {
        enabled: r[0].ok_or("timeline enabled")?,
        start_time: r[1].ok_or("timeline start")?,
        end_time: r[2].ok_or("timeline end")?,
        binder_index: r[3].ok_or("timeline binder")?,
        effector_index: r[4].ok_or("timeline effector")?,
        emitter_index: r[5].ok_or("timeline emitter")?,
        platform: r[6].ok_or("timeline platform")?,
        clip_index: r[7],
    })
}
fn finish_schedule(r: (Option<i32>, Option<i32>, Option<i32>)) -> Result<ScheduleItem> {
    Ok(ScheduleItem {
        enabled: r.0.ok_or("missing enabled")?,
        start_time: r.1.ok_or("missing start time")?,
        timeline_index: r.2.ok_or("missing timeline index")?,
    })
}
#[derive(Debug)]
pub struct Avfx<'a> {
    pub root: Block<'a>,
    pub global: Fields<'a>,
    pub nodes: Vec<Node<'a>>,
    pub textures: Vec<&'a [u8]>,
    pub models: Vec<Model<'a>>,
}
impl<'a> Avfx<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded);
        }
        let (root, end) = block_at(bytes, 0, 0)?;
        if !root.is(b"AVFX") || end != bytes.len() {
            return Err("AVFX root range/magic".into());
        }
        let global = root.fields(limits.max_blocks)?;
        let mut left = limits
            .max_blocks
            .checked_sub(global.len())
            .ok_or(Error::BudgetExceeded)?;
        let mut work = limits.max_work;
        let mut node_count = 0usize;
        let mut textures_count = 0usize;
        let mut model_count = 0usize;
        for b in global.iter() {
            if kind(b).is_some() {
                node_count += 1;
            } else if b.is(b"Tex") {
                textures_count += 1;
            } else if b.is(b"Modl") {
                model_count += 1;
            }
        }
        let allocation = node_count
            .checked_mul(std::mem::size_of::<Node<'_>>())
            .and_then(|v| v.checked_add(textures_count.checked_mul(std::mem::size_of::<&[u8]>())?))
            .and_then(|v| v.checked_add(model_count.checked_mul(std::mem::size_of::<Model<'_>>())?))
            .ok_or("AVFX allocation size")?;
        if allocation > limits.max_allocation_bytes {
            return Err(Error::BudgetExceeded);
        }
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(node_count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut textures = Vec::new();
        textures
            .try_reserve_exact(textures_count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut models = Vec::new();
        models
            .try_reserve_exact(model_count)
            .map_err(|_| Error::AllocationFailed)?;
        for b in global.iter() {
            work = work.checked_sub(1).ok_or(Error::BudgetExceeded)?;
            if let Some(kind) = kind(b) {
                let fields = b.fields(left)?;
                left = left
                    .checked_sub(fields.len())
                    .ok_or(Error::BudgetExceeded)?;
                nodes.push(Node {
                    kind,
                    block: b,
                    fields,
                });
            } else if b.is(b"Tex") {
                work = work
                    .checked_sub(b.payload.len())
                    .ok_or(Error::BudgetExceeded)?;
                textures.push(b.string()?);
            } else if b.is(b"Modl") {
                let f = b.fields(left)?;
                left = left.checked_sub(f.len()).ok_or(Error::BudgetExceeded)?;
                models.push(Model::parse(f, &mut work)?);
            }
        }
        for (count_tag, tag) in [
            (b"ScCn", b"Schd"),
            (b"TlCn", b"TmLn"),
            (b"EmCn", b"Emit"),
            (b"PrCn", b"Ptcl"),
            (b"EfCn", b"Efct"),
            (b"BdCn", b"Bind"),
            (b"TxCn", b"Tex\0"),
            (b"MdCn", b"Modl"),
        ] {
            let declared = global.one(count_tag)?.ok_or("missing AVFX count")?.u32()?;
            let tag = tag.strip_suffix(&[0]).unwrap_or(tag);
            let n = global.iter().filter(|v| v.is(tag)).count();
            if usize::try_from(declared).ok() != Some(n) {
                return Err("AVFX count mismatch".into());
            }
        }
        Ok(Self {
            root,
            global,
            nodes,
            textures,
            models,
        })
    }
    pub fn nodes_of(&self, kind: Kind) -> impl Iterator<Item = &Node<'a>> {
        self.nodes.iter().filter(move |n| n.kind == kind)
    }
}
fn kind(b: Block<'_>) -> Option<Kind> {
    [
        Kind::Scheduler,
        Kind::Timeline,
        Kind::Emitter,
        Kind::Particle,
        Kind::Effector,
        Kind::Binder,
    ]
    .into_iter()
    .find(|k| b.is(k.tag()))
}

#[derive(Clone, Copy, Debug)]
pub struct Curve<'a> {
    pub fields: Fields<'a>,
    keys: &'a [u8],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub time: i16,
    pub kind: u16,
    pub values: [f32; 3],
}
impl<'a> Curve<'a> {
    pub fn parse(block: Block<'a>, max_blocks: usize) -> Result<Self> {
        let fields = block.fields(max_blocks)?;
        let keys = fields.one(b"Keys")?.ok_or("missing curve keys")?.payload;
        if keys.len() % 16 != 0 {
            return Err("curve key size".into());
        }
        if let Some(n) = fields.integer(b"KeyC")? {
            if usize::try_from(n).ok() != Some(keys.len() / 16) {
                return Err("curve key count".into());
            }
        }
        Ok(Self { fields, keys })
    }
    pub fn keys(self) -> impl ExactSizeIterator<Item = Key> + 'a {
        self.keys.chunks_exact(16).map(|b| Key {
            time: i16::from_le_bytes([b[0], b[1]]),
            kind: u16::from_le_bytes([b[2], b[3]]),
            values: std::array::from_fn(|i| {
                f32::from_le_bytes(b[4 + i * 4..8 + i * 4].try_into().unwrap_or([0; 4]))
            }),
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Model<'a> {
    pub fields: Fields<'a>,
    draw: &'a [u8],
    indices: &'a [u8],
    emit: &'a [u8],
    numbers: &'a [u8],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawVertex {
    pub position: [f32; 4],
    pub normal: [i16; 4],
    pub tangent: [i16; 4],
    pub color: [u8; 4],
    pub uv: [[f32; 2]; 4],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [u8; 4],
}
impl<'a> Model<'a> {
    fn parse(fields: Fields<'a>, work: &mut usize) -> Result<Self> {
        let data =
            |name: &[u8]| -> Result<Option<&'a [u8]>> { Ok(fields.one(name)?.map(|b| b.payload)) };
        let draw = data(b"VDrw")?;
        let indices = data(b"VIdx")?;
        let emit = data(b"VEmt")?;
        let numbers = data(b"VNum")?;
        if draw.is_some() != indices.is_some() || emit.is_some() != numbers.is_some() {
            return Err("unpaired model streams".into());
        }
        let (draw, indices, emit, numbers) = (
            draw.unwrap_or(&[]),
            indices.unwrap_or(&[]),
            emit.unwrap_or(&[]),
            numbers.unwrap_or(&[]),
        );
        if draw.len() % 36 != 0
            || indices.len() % 6 != 0
            || emit.len() % 28 != 0
            || numbers.len() / 2 != emit.len() / 28
            || numbers.len() % 2 != 0
        {
            return Err("model stream sizes".into());
        }
        *work = work
            .checked_sub(indices.len() / 2)
            .ok_or(Error::BudgetExceeded)?;
        for raw in indices.chunks_exact(2) {
            if usize::from(u16::from_le_bytes([raw[0], raw[1]])) >= draw.len() / 36 {
                return Err("model vertex index".into());
            }
        }
        Ok(Self {
            fields,
            draw,
            indices,
            emit,
            numbers,
        })
    }
    pub fn draw_vertices(self) -> impl ExactSizeIterator<Item = DrawVertex> + 'a {
        self.draw.chunks_exact(36).map(|b| DrawVertex {
            position: std::array::from_fn(|i| half(u16::from_le_bytes([b[2 * i], b[2 * i + 1]]))),
            normal: std::array::from_fn(|i| i16::from(b[8 + i]) - 128),
            tangent: std::array::from_fn(|i| i16::from(b[12 + i]) - 128),
            color: b[16..20].try_into().unwrap_or([0; 4]),
            uv: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    let p = 20 + i * 4 + j * 2;
                    half(u16::from_le_bytes([b[p], b[p + 1]]))
                })
            }),
        })
    }
    pub fn indices(self) -> impl ExactSizeIterator<Item = u16> + 'a {
        self.indices
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
    }
    pub fn emit_vertices(self) -> impl ExactSizeIterator<Item = EmitVertex> + 'a {
        self.emit.chunks_exact(28).map(|b| EmitVertex {
            position: std::array::from_fn(|i| {
                f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4]))
            }),
            normal: std::array::from_fn(|i| {
                f32::from_le_bytes(b[12 + i * 4..16 + i * 4].try_into().unwrap_or([0; 4]))
            }),
            color: b[24..28].try_into().unwrap_or([0; 4]),
        })
    }
    /// Stored emission numbers; not interpreted as draw-vertex indices.
    pub fn emit_numbers(self) -> impl ExactSizeIterator<Item = u16> + 'a {
        self.numbers
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
    }
}
fn half(v: u16) -> f32 {
    let sign = if v & 0x8000 == 0 { 1. } else { -1. };
    let exponent = (v >> 10) & 31;
    let fraction = v & 1023;
    match exponent {
        0 => sign * f32::from(fraction) * 2f32.powi(-24),
        31 => {
            f32::from_bits((u32::from(v & 0x8000) << 16) | 0x7f800000 | (u32::from(fraction) << 13))
        }
        _ => sign * (1. + f32::from(fraction) / 1024.) * 2f32.powi(i32::from(exponent) - 15),
    }
}

/// Timeline Clip is an unframed fixed record, not a stream of AVFX blocks.
#[derive(Clone, Copy, Debug)]
pub struct Clip<'a> {
    pub tag: [u8; 4],
    pub integers: [i32; 4],
    pub reals: [f32; 4],
    pub names: [&'a [u8]; 4],
}
impl<'a> Clip<'a> {
    pub fn parse(block: Block<'a>) -> Result<Self> {
        let b = block.payload;
        if b.len() != 164 {
            return Err("timeline clip size".into());
        }
        let mut names = [&[][..]; 4];
        for (i, name) in names.iter_mut().enumerate() {
            let s = &b[36 + 32 * i..68 + 32 * i];
            *name = &s[..s.iter().position(|v| *v == 0).unwrap_or(32)];
        }
        Ok(Self {
            tag: [b[3], b[2], b[1], b[0]],
            integers: std::array::from_fn(|i| u32_at(b, 4 + i * 4).unwrap_or(0) as i32),
            reals: std::array::from_fn(|i| f32::from_bits(u32_at(b, 20 + i * 4).unwrap_or(0))),
            names,
        })
    }
}

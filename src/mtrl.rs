//! Borrowed little-endian MTRL material parameters and color/dye tables.
//!
//! Parses version 0x01030000, validates the declared file size and all referenced
//! strings/constant ranges/texture slots. No I/O, shader execution, color-space
//! transformation is performed during parsing. [`ColorTable::to_canonical`] and
//! [`dye::apply_table`] produce complete, immutable-base color/stain outputs.
//! Explicit half-row stain operations are also available in [`dye`]. Color fields retain HDR
//! and negative half-float values. Unrecognized dataset sizes are preserved as
//! raw bytes and return a specific error from `color_table`, not false absence.

use crate::binary::{Reader, record, slice, u16_at, u32_at};
use std::fmt;

mod color;
pub mod dye;
pub use color::CanonicalColorTable;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion(u32),
    UnsupportedDatasetSize(usize),
    UnsupportedColorTableLayout { half_lanes: usize, rows: usize },
    StringBudgetExceeded,
}
impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Self::InvalidData(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(e) => write!(f, "invalid MTRL data: {e}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported MTRL version {v:#x}"),
            Self::UnsupportedDatasetSize(n) => write!(f, "unsupported MTRL dataset size {n}"),
            Self::UnsupportedColorTableLayout { half_lanes, rows } => write!(
                f,
                "unsupported MTRL color table {half_lanes} half lanes by {rows} rows"
            ),
            Self::StringBudgetExceeded => f.write_str("MTRL string validation budget exceeded"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Texture<'a> {
    pub path: &'a [u8],
    pub flags: u16,
}
#[derive(Clone, Copy, Debug)]
pub struct AttributeSet<'a> {
    pub name: &'a [u8],
    pub index: u8,
    pub unknown: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShaderKey {
    pub id: u32,
    pub value: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Constant {
    pub id: u32,
    pub offset: u16,
    pub size: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sampler {
    pub id: u32,
    pub flags: u32,
    pub texture_index: u8,
    pub reserved: [u8; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorLayout {
    Legacy,
    Dawntrail,
}
/// Inline color-table dimensions from the first additional-data u32.
/// Width is expressed in binary16 scalar lanes, not bytes or half4 groups.
/// Other flag bits remain available verbatim; no dye or shader is applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorTableDeclaration {
    pub flags: u32,
    pub half_lanes: usize,
    pub rows: usize,
    pub has_dye: bool,
}
impl ColorLayout {
    pub fn rows(self) -> usize {
        match self {
            Self::Legacy => 16,
            Self::Dawntrail => 32,
        }
    }
    pub fn lanes(self) -> usize {
        match self {
            Self::Legacy => 16,
            Self::Dawntrail => 32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ColorTable<'a> {
    layout: ColorLayout,
    colors: &'a [u8],
    dye: &'a [u8],
}
impl<'a> ColorTable<'a> {
    pub fn layout(self) -> ColorLayout {
        self.layout
    }
    pub fn row_count(self) -> usize {
        self.layout.rows()
    }
    pub fn has_dye(self) -> bool {
        !self.dye.is_empty()
    }
    pub fn row(self, index: usize) -> Result<ColorRow<'a>> {
        Ok(ColorRow {
            layout: self.layout,
            data: record(self.colors, index, self.layout.lanes() * 2)?,
        })
    }
    pub fn dye_row(self, index: usize) -> Result<Option<DyeRow>> {
        if index >= self.row_count() {
            return Err(Error::InvalidData("dye row index"));
        }
        if self.dye.is_empty() {
            return Ok(None);
        }
        let raw = match self.layout {
            ColorLayout::Legacy => u32::from(u16_at(record(self.dye, index, 2)?, 0)?),
            ColorLayout::Dawntrail => u32_at(record(self.dye, index, 4)?, 0)?,
        };
        Ok(Some(DyeRow {
            layout: self.layout,
            raw,
        }))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ColorRow<'a> {
    layout: ColorLayout,
    data: &'a [u8],
}
impl<'a> ColorRow<'a> {
    pub fn layout(self) -> ColorLayout {
        self.layout
    }
    pub fn raw(self) -> &'a [u8] {
        self.data
    }
    pub fn half_bits(self, lane: usize) -> Result<u16> {
        Ok(u16_at(record(self.data, lane, 2)?, 0)?)
    }
    pub fn value(self, lane: usize) -> Result<f32> {
        Ok(crate::tex::half_to_f32(self.half_bits(lane)?))
    }
    /// File-layout values, padding the returned array with zero for Legacy rows.
    /// Use [`Self::canonical_half_bits`] for the 32-lane color conversion.
    pub fn values(self) -> [f32; 32] {
        let mut out = [0.; 32];
        for (value, b) in out.iter_mut().zip(self.data.chunks_exact(2)) {
            *value = crate::tex::half_to_f32(u16::from_le_bytes([b[0], b[1]]));
        }
        out
    }
    pub fn diffuse(self) -> [f32; 3] {
        let v = self.values();
        [v[0], v[1], v[2]]
    }
    pub fn specular(self) -> [f32; 3] {
        let v = self.values();
        [v[4], v[5], v[6]]
    }
    pub fn emissive(self) -> [f32; 3] {
        let v = self.values();
        [v[8], v[9], v[10]]
    }
    pub fn tile_matrix(self) -> [f32; 4] {
        let v = self.values();
        let start = match self.layout {
            ColorLayout::Legacy => 12,
            ColorLayout::Dawntrail => 28,
        };
        [v[start], v[start + 1], v[start + 2], v[start + 3]]
    }
    pub fn roughness(self) -> Option<f32> {
        (self.layout == ColorLayout::Dawntrail).then(|| self.values()[16])
    }
    pub fn metalness(self) -> Option<f32> {
        (self.layout == ColorLayout::Dawntrail).then(|| self.values()[18])
    }
    /// Encoded tile selector value; callers can inspect the half value without
    /// truncating NaNs/negative values to a guessed integer ID.
    pub fn tile_selector(self) -> f32 {
        let v = self.values();
        v[match self.layout {
            ColorLayout::Legacy => 11,
            ColorLayout::Dawntrail => 25,
        }] * 64.
    }
    pub fn shader_selector(self) -> Option<f32> {
        (self.layout == ColorLayout::Dawntrail).then(|| self.values()[24])
    }
    pub fn sphere_selector(self) -> Option<f32> {
        (self.layout == ColorLayout::Dawntrail).then(|| self.values()[27])
    }
}

/// Destination mapping for the observed 32-half intermediate color row.
/// This is selected independently of the file's [`ColorLayout`].
/// [`Self::for_shader_package`] implements the named inline-material routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DyeOverrideProfile {
    Legacy,
    Dawntrail,
}

impl DyeOverrideProfile {
    /// The inline color consumer uses the five-column profile for
    /// `characterlegacy.shpk`, and the twelve-column profile for other packages.
    /// Pass the package name from [`Mtrl::shader_package`], not a resource path.
    /// Storage dimensions do not select this profile.
    pub fn for_shader_package(name: &[u8]) -> Self {
        if name == b"characterlegacy.shpk" {
            Self::Legacy
        } else {
            Self::Dawntrail
        }
    }

    fn column_spans(self) -> &'static [(usize, usize)] {
        match self {
            Self::Legacy => &[(0, 3), (4, 3), (8, 3), (3, 1), (7, 1)],
            Self::Dawntrail => &[
                (0, 3),
                (4, 3),
                (8, 3),
                (11, 1),
                (18, 1),
                (16, 1),
                (12, 1),
                (13, 1),
                (14, 1),
                (19, 1),
                (27, 1),
                (21, 1),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DyeRow {
    pub layout: ColorLayout,
    pub raw: u32,
}
impl DyeRow {
    /// Half lanes eligible for replacement in a 32-half intermediate color row.
    ///
    /// Bit `n` addresses intermediate lane `n`, not a Legacy file-row lane.
    /// RGB flags select three lanes; scalar flags select one. The caller must
    /// choose the destination profile independently of the source table layout.
    /// Template/channel/unknown bits do not affect this declaration mask.
    /// It does not look up STM values or guarantee a write: the observed
    /// consumer checks the requested channel and preserves existing specular
    /// RGB when all three selected replacement half bit patterns are 0x0000.
    /// It neither converts Legacy colors nor applies stains.
    pub fn canonical_override_lane_mask(self, profile: DyeOverrideProfile) -> u32 {
        let mut lanes = 0;
        for (flag, &(start, width)) in profile.column_spans().iter().enumerate() {
            if self.flags() & (1 << flag) != 0 {
                lanes |= ((1 << width) - 1) << start;
            }
        }
        lanes
    }
    pub fn flags(self) -> u16 {
        (self.raw
            & match self.layout {
                ColorLayout::Legacy => 31,
                ColorLayout::Dawntrail => 4095,
            }) as u16
    }
    pub fn template(self) -> u16 {
        match self.layout {
            ColorLayout::Legacy => ((self.raw >> 5) & 2047) as u16,
            ColorLayout::Dawntrail => ((self.raw >> 16) & 2047) as u16,
        }
    }
    pub fn channel(self) -> Option<u8> {
        (self.layout == ColorLayout::Dawntrail).then_some(((self.raw >> 27) & 3) as u8)
    }
    pub fn unknown_bits(self) -> u32 {
        match self.layout {
            ColorLayout::Legacy => self.raw & 0xffff0000,
            ColorLayout::Dawntrail => self.raw & 0xe000f000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mtrl<'a> {
    textures: &'a [u8],
    uv_sets: &'a [u8],
    color_sets: &'a [u8],
    strings: &'a [u8],
    shader_offset: u16,
    additional: &'a [u8],
    dataset: &'a [u8],
    shader_flags: u32,
    keys: &'a [u8],
    constants: &'a [u8],
    samplers: &'a [u8],
    values: &'a [u8],
}
impl<'a> Mtrl<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_string_budget(bytes, 4 * 1024 * 1024)
    }
    pub fn parse_with_string_budget(bytes: &'a [u8], mut budget: usize) -> Result<Self> {
        let h = slice(bytes, 0, 16)?;
        let version = u32_at(h, 0)?;
        if version != 0x0103_0000 {
            return Err(Error::UnsupportedVersion(version));
        }
        if usize::from(u16_at(h, 4)?) != bytes.len() {
            return Err(Error::InvalidData("declared file size mismatch"));
        }
        let mut r = Reader {
            data: bytes,
            pos: 16,
        };
        let textures = r.take(usize::from(h[12]), 4)?;
        let uv_sets = r.take(usize::from(h[13]), 4)?;
        let color_sets = r.take(usize::from(h[14]), 4)?;
        let strings = r.take(usize::from(u16_at(h, 8)?), 1)?;
        let additional = r.take(usize::from(h[15]), 1)?;
        let dataset = r.take(usize::from(u16_at(h, 6)?), 1)?;
        let material_header = r.take(1, 12)?;
        let keys = r.take(usize::from(u16_at(material_header, 2)?), 8)?;
        let constants = r.take(usize::from(u16_at(material_header, 4)?), 8)?;
        let samplers = r.take(usize::from(u16_at(material_header, 6)?), 12)?;
        let values = r.take(usize::from(u16_at(material_header, 0)?), 1)?;
        if r.pos != bytes.len() {
            return Err(Error::InvalidData("trailing bytes"));
        }
        let model = Self {
            textures,
            uv_sets,
            color_sets,
            strings,
            shader_offset: u16_at(h, 10)?,
            additional,
            dataset,
            shader_flags: u32_at(material_header, 8)?,
            keys,
            constants,
            samplers,
            values,
        };
        let mut validate_string = |offset: u16| -> Result<()> {
            let suffix = strings
                .get(usize::from(offset)..)
                .ok_or(Error::InvalidData("string offset"))?;
            let end = match suffix.iter().take(budget).position(|&b| b == 0) {
                Some(end) => end,
                None if suffix.len() >= budget => return Err(Error::StringBudgetExceeded),
                None => return Err(Error::InvalidData("unterminated string")),
            };
            budget -= end + 1;
            Ok(())
        };
        validate_string(model.shader_offset)?;
        for table in [textures, uv_sets, color_sets] {
            for b in table.chunks_exact(4) {
                validate_string(u16_at(b, 0)?)?;
            }
        }
        for i in 0..model.constant_count() {
            model.constant_bytes(i)?;
        }
        for i in 0..model.sampler_count() {
            let slot = model.sampler(i)?.texture_index;
            if slot != u8::MAX && usize::from(slot) >= model.texture_count() {
                return Err(Error::InvalidData("sampler texture index"));
            }
        }
        Ok(model)
    }
    pub fn strings(&self) -> &'a [u8] {
        self.strings
    }
    pub fn string(&self, offset: u16) -> Result<&'a [u8]> {
        let s = self
            .strings
            .get(usize::from(offset)..)
            .ok_or(Error::InvalidData("string offset"))?;
        let end = s
            .iter()
            .position(|&b| b == 0)
            .ok_or(Error::InvalidData("unterminated string"))?;
        Ok(&s[..end])
    }
    pub fn shader_package(&self) -> Result<&'a [u8]> {
        self.string(self.shader_offset)
    }
    pub fn additional_data(&self) -> &'a [u8] {
        self.additional
    }
    pub fn table_flags(&self) -> Option<u8> {
        self.additional.first().copied()
    }
    /// Decode an inline-table declaration when four additional bytes exist and
    /// bit 2 selects embedded color data. `None` does not discard a nonempty raw
    /// dataset or establish absence of externally supplied color data.
    /// A zero width exponent means the Legacy 16-lane, 16-row default; otherwise
    /// width and row exponents occupy bits 4..7 and 8..11 respectively.
    pub fn inline_color_table_declaration(&self) -> Option<ColorTableDeclaration> {
        let raw: [u8; 4] = self.additional.get(..4)?.try_into().ok()?;
        let flags = u32::from_le_bytes(raw);
        if flags & 4 == 0 {
            return None;
        }
        let width = (flags >> 4) & 15;
        let (half_lanes, rows) = if width == 0 {
            (16, 16)
        } else {
            (4usize << width, 1usize << ((flags >> 8) & 15))
        };
        Some(ColorTableDeclaration {
            flags,
            half_lanes,
            rows,
            has_dye: flags & 8 != 0,
        })
    }
    pub fn raw_dataset(&self) -> &'a [u8] {
        self.dataset
    }
    pub fn shader_flags(&self) -> u32 {
        self.shader_flags
    }
    pub fn texture_count(&self) -> usize {
        self.textures.len() / 4
    }
    pub fn uv_set_count(&self) -> usize {
        self.uv_sets.len() / 4
    }
    pub fn color_set_count(&self) -> usize {
        self.color_sets.len() / 4
    }
    pub fn key_count(&self) -> usize {
        self.keys.len() / 8
    }
    pub fn constant_count(&self) -> usize {
        self.constants.len() / 8
    }
    pub fn sampler_count(&self) -> usize {
        self.samplers.len() / 12
    }
    pub fn texture(&self, index: usize) -> Result<Texture<'a>> {
        let b = record(self.textures, index, 4)?;
        Ok(Texture {
            path: self.string(u16_at(b, 0)?)?,
            flags: u16_at(b, 2)?,
        })
    }
    fn attribute_set(&self, table: &[u8], index: usize) -> Result<AttributeSet<'a>> {
        let b = record(table, index, 4)?;
        Ok(AttributeSet {
            name: self.string(u16_at(b, 0)?)?,
            index: b[2],
            unknown: b[3],
        })
    }
    pub fn uv_set(&self, index: usize) -> Result<AttributeSet<'a>> {
        self.attribute_set(self.uv_sets, index)
    }
    pub fn color_set(&self, index: usize) -> Result<AttributeSet<'a>> {
        self.attribute_set(self.color_sets, index)
    }
    pub fn shader_key(&self, index: usize) -> Result<ShaderKey> {
        let b = record(self.keys, index, 8)?;
        Ok(ShaderKey {
            id: u32_at(b, 0)?,
            value: u32_at(b, 4)?,
        })
    }
    pub fn constant(&self, index: usize) -> Result<Constant> {
        let b = record(self.constants, index, 8)?;
        Ok(Constant {
            id: u32_at(b, 0)?,
            offset: u16_at(b, 4)?,
            size: u16_at(b, 6)?,
        })
    }
    pub fn constant_bytes(&self, index: usize) -> Result<&'a [u8]> {
        let c = self.constant(index)?;
        Ok(slice(
            self.values,
            usize::from(c.offset),
            usize::from(c.size),
        )?)
    }
    pub fn constant_floats(&self, index: usize) -> Result<impl ExactSizeIterator<Item = f32> + 'a> {
        let c = self.constant(index)?;
        if c.offset % 4 != 0 || c.size % 4 != 0 {
            return Err(Error::InvalidData("unaligned float constant"));
        }
        Ok(self
            .constant_bytes(index)?
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])))
    }
    pub fn sampler(&self, index: usize) -> Result<Sampler> {
        let b = record(self.samplers, index, 12)?;
        Ok(Sampler {
            id: u32_at(b, 0)?,
            flags: u32_at(b, 4)?,
            texture_index: b[8],
            reserved: [b[9], b[10], b[11]],
        })
    }
    pub fn color_table(&self) -> Result<Option<ColorTable<'a>>> {
        let (layout, color_size) = match self.dataset.len() {
            0 => return Ok(None),
            512 | 544 => (ColorLayout::Legacy, 512),
            2048 | 2176 => (ColorLayout::Dawntrail, 2048),
            size => return Err(Error::UnsupportedDatasetSize(size)),
        };
        if let Some(d) = self.inline_color_table_declaration() {
            if d.half_lanes != layout.lanes() || d.rows != layout.rows() {
                return Err(Error::UnsupportedColorTableLayout {
                    half_lanes: d.half_lanes,
                    rows: d.rows,
                });
            }
            if d.has_dye != (self.dataset.len() > color_size) {
                return Err(Error::InvalidData("dye declaration disagrees with dataset"));
            }
        }
        Ok(Some(ColorTable {
            layout,
            colors: &self.dataset[..color_size],
            dye: &self.dataset[color_size..],
        }))
    }
}

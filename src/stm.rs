//! Staining templates: borrowed Legacy (0x0101) and Dawntrail (0x0201) STM.
//!
//! Supports zero, uniform, dense and palette-indexed half-float columns, checked
//! template-relative cumulative ends, and the observed 254-byte selector map.
//! Palette selector zero yields implicit zero; invalid selectors are errors,
//! never silently clamped. Stain IDs are one-based. The palette map's first byte
//! is a marker, leaving IDs 1..=253; no last value is read from the next column.
//! Layouts with no palette may carry 254 values. Parsing allocates no storage.
//! Raw lookup does not apply MTRL override masks or evaluate material shaders.

use crate::binary::{record, slice, u16_at, u32_at};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion(u16),
    UnsupportedChannels(u8, u8),
    StainOutOfBounds { id: u16, capacity: usize },
    ColumnOutOfBounds,
    BudgetExceeded(&'static str),
}
impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Self::InvalidData(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(s) => write!(f, "invalid STM data: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported STM version {v:#x}"),
            Self::UnsupportedChannels(c, s) => write!(f, "unsupported STM channel counts {c}/{s}"),
            Self::StainOutOfBounds { id, capacity } => {
                write!(f, "STM stain {id} outside 1..={capacity}")
            }
            Self::ColumnOutOfBounds => f.write_str("STM column out of bounds"),
            Self::BudgetExceeded(s) => write!(f, "STM {s} budget exceeded"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_templates: usize,
    pub max_validation_steps: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 16 * 1024 * 1024,
            max_templates: 4096,
            max_validation_steps: 16 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    Legacy,
    Dawntrail,
}
impl Layout {
    pub fn column_count(self) -> usize {
        match self {
            Self::Legacy => 5,
            Self::Dawntrail => 12,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    Zero,
    Uniform,
    Dense,
    Palette,
}

#[derive(Clone, Copy, Debug)]
pub struct Column<'a> {
    components: usize,
    bytes: &'a [u8],
    encoding: Encoding,
    palette_count: usize,
}
impl<'a> Column<'a> {
    fn parse(bytes: &'a [u8], components: usize) -> Result<Self> {
        let stride = components * 2;
        let (encoding, palette_count) = if bytes.is_empty() {
            (Encoding::Zero, 0)
        } else if bytes.len() == stride {
            (Encoding::Uniform, 1)
        } else if bytes.len() == 254 * stride {
            (Encoding::Dense, 254)
        } else if bytes.len() >= 254 && (bytes.len() - 254).is_multiple_of(stride) {
            let n = (bytes.len() - 254) / stride;
            if n > 254 {
                return Err(Error::InvalidData("palette too large"));
            }
            (Encoding::Palette, n)
        } else {
            return Err(Error::InvalidData("unrecognized column encoding length"));
        };
        Ok(Self {
            components,
            bytes,
            encoding,
            palette_count,
        })
    }
    pub fn components(self) -> usize {
        self.components
    }
    pub fn encoding(self) -> Encoding {
        self.encoding
    }
    pub fn raw(self) -> &'a [u8] {
        self.bytes
    }
    pub fn stain_capacity(self) -> usize {
        if self.encoding == Encoding::Palette {
            253
        } else {
            254
        }
    }
    pub fn palette_count(self) -> usize {
        self.palette_count
    }
    pub fn selector_map(self) -> Option<&'a [u8]> {
        (self.encoding == Encoding::Palette)
            .then(|| &self.bytes[self.palette_count * self.components * 2..])
    }
    /// Returns raw half bits, zero-filling unused scalar lanes. Numeric defaults
    /// are explicit zero; this does not mean the MTRL permits overriding a field.
    pub fn half_bits(self, stain_id: u16) -> Result<[u16; 3]> {
        let id = usize::from(stain_id);
        if id == 0 || id > self.stain_capacity() {
            return Err(Error::StainOutOfBounds {
                id: stain_id,
                capacity: self.stain_capacity(),
            });
        }
        let item = match self.encoding {
            Encoding::Zero => return Ok([0; 3]),
            Encoding::Uniform => 0,
            Encoding::Dense => id - 1,
            Encoding::Palette => {
                let map = self
                    .selector_map()
                    .ok_or(Error::InvalidData("missing selector map"))?;
                let index = usize::from(map[id]);
                if index == 0 {
                    return Ok([0; 3]);
                }
                if index > self.palette_count {
                    return Err(Error::InvalidData("palette selector exceeds values"));
                }
                index - 1
            }
        };
        let data = record(self.bytes, item, self.components * 2)?;
        let mut result = [0; 3];
        for (lane, value) in result.iter_mut().enumerate().take(self.components) {
            *value = u16_at(data, lane * 2)?;
        }
        Ok(result)
    }
    pub fn value(self, stain_id: u16) -> Result<[f32; 3]> {
        Ok(self.half_bits(stain_id)?.map(crate::tex::half_to_f32))
    }
    fn validate(self, steps: &mut usize) -> Result<()> {
        if let Some(map) = self.selector_map() {
            *steps = steps
                .checked_sub(map.len())
                .ok_or(Error::BudgetExceeded("selector validation"))?;
            if map[0] != 255 {
                return Err(Error::InvalidData("selector marker"));
            }
            for &index in &map[1..] {
                if usize::from(index) > self.palette_count {
                    return Err(Error::InvalidData("palette selector exceeds values"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DyeValues {
    pub layout: Layout,
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub scalars: [f32; 9],
}
impl DyeValues {
    pub fn gloss_strength(self) -> Option<f32> {
        (self.layout == Layout::Legacy).then_some(self.scalars[0])
    }
    pub fn specular_strength(self) -> Option<f32> {
        (self.layout == Layout::Legacy).then_some(self.scalars[1])
    }
    pub fn metalness(self) -> Option<f32> {
        (self.layout == Layout::Dawntrail).then_some(self.scalars[1])
    }
    pub fn roughness(self) -> Option<f32> {
        (self.layout == Layout::Dawntrail).then_some(self.scalars[2])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Template<'a> {
    layout: Layout,
    ends: &'a [u8],
    payload: &'a [u8],
}
impl<'a> Template<'a> {
    fn parse(layout: Layout, bytes: &'a [u8]) -> Result<Self> {
        let n = layout.column_count();
        let ends = slice(bytes, 0, n * 2)?;
        let length = usize::from(u16_at(ends, (n - 1) * 2)?) * 2;
        let payload = slice(bytes, n * 2, length)?;
        let mut previous = 0;
        for column in 0..n {
            let end = usize::from(u16_at(ends, column * 2)?) * 2;
            if end < previous {
                return Err(Error::InvalidData("nonmonotonic column ends"));
            }
            previous = end;
        }
        Ok(Self {
            layout,
            ends,
            payload,
        })
    }
    pub fn layout(self) -> Layout {
        self.layout
    }
    pub fn column_count(self) -> usize {
        self.layout.column_count()
    }
    pub fn encoded_size(self) -> usize {
        self.ends.len() + self.payload.len()
    }
    pub fn column(self, index: usize) -> Result<Column<'a>> {
        if index >= self.column_count() {
            return Err(Error::ColumnOutOfBounds);
        }
        let end = usize::from(u16_at(self.ends, index * 2)?) * 2;
        let start = if index == 0 {
            0
        } else {
            usize::from(u16_at(self.ends, (index - 1) * 2)?) * 2
        };
        Column::parse(
            slice(self.payload, start, end - start)?,
            if index < 3 { 3 } else { 1 },
        )
    }
    pub fn stain_capacity(self) -> Result<usize> {
        let mut capacity = 254;
        for i in 0..self.column_count() {
            capacity = capacity.min(self.column(i)?.stain_capacity());
        }
        Ok(capacity)
    }
    pub fn sample(self, stain_id: u16) -> Result<DyeValues> {
        let capacity = self.stain_capacity()?;
        if stain_id == 0 || usize::from(stain_id) > capacity {
            return Err(Error::StainOutOfBounds {
                id: stain_id,
                capacity,
            });
        }
        let mut scalars = [0.; 9];
        for (i, value) in scalars.iter_mut().enumerate().take(self.column_count() - 3) {
            *value = self.column(i + 3)?.value(stain_id)?[0];
        }
        Ok(DyeValues {
            layout: self.layout,
            diffuse: self.column(0)?.value(stain_id)?,
            specular: self.column(1)?.value(stain_id)?,
            emissive: self.column(2)?.value(stain_id)?,
            scalars,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Stm<'a> {
    layout: Layout,
    declared_channels: [u8; 2],
    keys: &'a [u8],
    offsets: &'a [u8],
    body: &'a [u8],
    used_end: usize,
}
impl<'a> Stm<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("input"));
        }
        let h = slice(bytes, 0, 8)?;
        if &h[..2] != b"MS" {
            return Err(Error::InvalidData("magic"));
        }
        let version = u16_at(h, 2)?;
        let layout = match version {
            0x0101 => Layout::Legacy,
            0x0201 => Layout::Dawntrail,
            _ => return Err(Error::UnsupportedVersion(version)),
        };
        if layout == Layout::Dawntrail && (h[6] != 3 || h[7] != 9) {
            return Err(Error::UnsupportedChannels(h[6], h[7]));
        }
        let count = usize::from(u16_at(h, 4)?);
        if count > limits.max_templates {
            return Err(Error::BudgetExceeded("templates"));
        }
        let keys = slice(bytes, 8, count * 4)?;
        let offsets = slice(bytes, 8 + count * 4, count * 4)?;
        let body = bytes
            .get(8 + count * 8..)
            .ok_or(Error::InvalidData("body offset"))?;
        let mut stm = Self {
            layout,
            declared_channels: [h[6], h[7]],
            keys,
            offsets,
            body,
            used_end: 0,
        };
        let mut steps = limits.max_validation_steps;
        for i in 0..count {
            let id = stm.template_id(i)?;
            for j in 0..i {
                steps = steps
                    .checked_sub(1)
                    .ok_or(Error::BudgetExceeded("key validation"))?;
                if id == stm.template_id(j)? {
                    return Err(Error::InvalidData("duplicate template ID"));
                }
            }
            let template = stm.template(i)?;
            let offset = stm.template_offset(i)?;
            stm.used_end = stm.used_end.max(offset + template.encoded_size());
            for column in 0..template.column_count() {
                template.column(column)?.validate(&mut steps)?;
            }
        }
        Ok(stm)
    }
    pub fn layout(self) -> Layout {
        self.layout
    }
    pub fn declared_channel_counts(self) -> [u8; 2] {
        self.declared_channels
    }
    pub fn template_count(self) -> usize {
        self.keys.len() / 4
    }
    pub fn template_id(self, index: usize) -> Result<u32> {
        Ok(u32_at(record(self.keys, index, 4)?, 0)?)
    }
    fn template_offset(self, index: usize) -> Result<usize> {
        (u32_at(record(self.offsets, index, 4)?, 0)? as usize)
            .checked_mul(2)
            .ok_or(Error::InvalidData("template offset overflow"))
    }
    pub fn template(self, index: usize) -> Result<Template<'a>> {
        let data = self
            .body
            .get(self.template_offset(index)?..)
            .ok_or(Error::InvalidData("template offset"))?;
        Template::parse(self.layout, data)
    }
    pub fn find_template(self, id: u32) -> Result<Option<Template<'a>>> {
        for index in 0..self.template_count() {
            if self.template_id(index)? == id {
                return self.template(index).map(Some);
            }
        }
        Ok(None)
    }
    /// Unreferenced tail after the last declared template payload.
    pub fn trailing_bytes(self) -> &'a [u8] {
        &self.body[self.used_end..]
    }
}

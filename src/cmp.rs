//! Borrowed Win32 eight-race human.cmp palettes and body parameter ranges.
//!
//! This profile has 178 blocks of 256 RGBA8 colors followed by 80 records of
//! fourteen f32 scalars. Exact profile length, finite parameters and ordered
//! range endpoints are validated, without allocation. Other sizes are explicit
//! errors rather than guessed tail layouts. Race indices are zero-based, body
//! types are 1..=5, and clan indices are 0..=1. No customize fallback, slider
//! clamping, pose scaling or color-space conversion is applied.
use crate::binary::{record, u32_at};
use std::fmt;
const COLORS: usize = 0x2c800;
const RECORDS: usize = 80;
const STRIDE: usize = 56;
pub const FILE_SIZE: usize = COLORS + RECORDS * STRIDE;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedLayout { bytes: usize },
    InvalidData(&'static str),
    InvalidPercentage(u8),
    IndexOutOfBounds,
}
impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::InvalidData(s)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedLayout { bytes } => {
                write!(f, "unsupported CMP profile length {bytes}")
            }
            Self::InvalidData(s) => write!(f, "invalid CMP: {s}"),
            Self::InvalidPercentage(v) => write!(f, "CMP percentage {v} exceeds 100"),
            Self::IndexOutOfBounds => f.write_str("CMP parameter or palette index out of bounds"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sex {
    Male,
    Female,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
}
impl Range {
    /// Evaluate a percentage in `0..=100`, without customize fallback, clamping
    /// or sentinel interpretation. Endpoints must be finite and ordered.
    /// Weighted f64 arithmetic avoids f32 difference overflow and rounds once
    /// to f32; it does not promise the client's f32 instruction-level rounding.
    pub fn at_percent(self, percent: u8) -> Result<f32> {
        if percent > 100 {
            return Err(Error::InvalidPercentage(percent));
        }
        if !self.min.is_finite() || !self.max.is_finite() || self.min > self.max {
            return Err(Error::InvalidData("invalid interpolation endpoints"));
        }
        match percent {
            0 => Ok(self.min),
            100 => Ok(self.max),
            p => Ok(((f64::from(self.min) * f64::from(100 - p)
                + f64::from(self.max) * f64::from(p))
                / 100.) as f32),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorRange {
    pub min: [f32; 3],
    pub max: [f32; 3],
}
impl VectorRange {
    /// Component-wise [`Range::at_percent`], without bone-space application.
    pub fn at_percent(self, percent: u8) -> Result<[f32; 3]> {
        let mut output = [0.; 3];
        for (i, value) in output.iter_mut().enumerate() {
            *value = Range {
                min: self.min[i],
                max: self.max[i],
            }
            .at_percent(percent)?;
        }
        Ok(output)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub raw: [f32; 14],
}
impl Parameters {
    pub fn height(self, sex: Sex) -> Range {
        let p = match sex {
            Sex::Male => 0,
            Sex::Female => 4,
        };
        Range {
            min: self.raw[p],
            max: self.raw[p + 1],
        }
    }
    pub fn tail(self, sex: Sex) -> Range {
        let p = match sex {
            Sex::Male => 2,
            Sex::Female => 6,
        };
        Range {
            min: self.raw[p],
            max: self.raw[p + 1],
        }
    }
    pub fn bust(self) -> VectorRange {
        VectorRange {
            min: [self.raw[8], self.raw[9], self.raw[10]],
            max: [self.raw[11], self.raw[12], self.raw[13]],
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Cmp<'a> {
    colors: &'a [u8],
    parameters: &'a [u8],
}
impl<'a> Cmp<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() != FILE_SIZE {
            return Err(Error::UnsupportedLayout { bytes: bytes.len() });
        }
        let cmp = Self {
            colors: &bytes[..COLORS],
            parameters: &bytes[COLORS..],
        };
        for i in 0..RECORDS {
            let p = cmp.parameter_record(i)?;
            if p.raw.iter().any(|v| !v.is_finite()) {
                return Err(Error::InvalidData("nonfinite body parameter"));
            }
            for (low, high) in [(0, 1), (2, 3), (4, 5), (6, 7), (8, 11), (9, 12), (10, 13)] {
                if p.raw[low] > p.raw[high] {
                    return Err(Error::InvalidData("reversed parameter range"));
                }
            }
        }
        Ok(cmp)
    }
    pub fn color_count(self) -> usize {
        self.colors.len() / 4
    }
    pub fn palette_count(self) -> usize {
        self.colors.len() / 1024
    }
    pub fn raw_colors(self) -> &'a [u8] {
        self.colors
    }
    /// Stored RGBA bytes, without gamma conversion or alpha filtering.
    pub fn color(self, index: usize) -> Result<[u8; 4]> {
        if index >= self.color_count() {
            return Err(Error::IndexOutOfBounds);
        }
        Ok(record(self.colors, index, 4)?
            .try_into()
            .map_err(|_| Error::InvalidData("color width"))?)
    }
    pub fn palette_color(self, palette: usize, index: usize) -> Result<[u8; 4]> {
        if palette >= self.palette_count() || index >= 256 {
            return Err(Error::IndexOutOfBounds);
        }
        self.color(palette * 256 + index)
    }
    pub fn parameter_count(self) -> usize {
        RECORDS
    }
    pub fn parameter_record(self, index: usize) -> Result<Parameters> {
        if index >= RECORDS {
            return Err(Error::IndexOutOfBounds);
        }
        let bytes = record(self.parameters, index, STRIDE)?;
        let mut raw = [0.; 14];
        for (i, v) in raw.iter_mut().enumerate() {
            *v = f32::from_bits(u32_at(bytes, i * 4)?);
        }
        Ok(Parameters { raw })
    }
    pub fn parameters(
        self,
        race_index: usize,
        body_type: u8,
        clan_index: usize,
    ) -> Result<Parameters> {
        if race_index >= 8 || !(1..=5).contains(&body_type) || clan_index >= 2 {
            return Err(Error::IndexOutOfBounds);
        }
        self.parameter_record(race_index * 10 + usize::from(body_type - 1) * 2 + clan_index)
    }
}

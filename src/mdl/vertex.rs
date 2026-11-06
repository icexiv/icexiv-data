//! Declaration-driven scalar conversion; no coordinate or shader transformations.

use super::{Error, Result, f32_at, u16_at};

/// Raw MDL usage IDs are retained to avoid conflating shader inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Element {
    pub stream: u8,
    pub offset: u8,
    pub kind: u8,
    pub usage: u8,
    pub usage_index: u8,
}

impl Element {
    /// Stored byte width for the scalar layouts decoded by this module.
    pub fn byte_width(self) -> Result<usize> {
        Ok(match self.kind {
            0..=3 => 4 * (usize::from(self.kind) + 1),
            5 | 8 => 4,
            13 => 4,
            14 | 17 => 8,
            _ => return Err(Error::UnsupportedVertexType(self.kind)),
        })
    }

    /// Scalar conversion without usage-dependent reinterpretation. Float1 through
    /// Float4 (0..=3) preserve the little-endian binary32 bit patterns, including
    /// signed zero and NaN payloads, without arithmetic or normalization. UByte4 (5)
    /// remains 0..255; UByte4N (8) becomes 0..1. Unused lanes are zero.
    /// In particular, blend-weight scaling is a caller's usage interpretation.
    pub fn decode(self, bytes: &[u8]) -> Result<[f32; 8]> {
        let width = self.byte_width()?;
        let bytes = bytes
            .get(..width)
            .ok_or(Error::InvalidData("short vertex element"))?;
        let mut value = [0.; 8];
        match self.kind {
            0..=3 => {
                for (i, lane) in value
                    .iter_mut()
                    .enumerate()
                    .take(usize::from(self.kind) + 1)
                {
                    *lane = f32_at(bytes, i * 4)?;
                }
            }
            5 | 8 | 17 => {
                for i in 0..width {
                    value[i] = f32::from(bytes[i]) / if self.kind == 8 { 255. } else { 1. };
                }
            }
            13 | 14 => {
                for (i, lane) in value.iter_mut().enumerate().take(width / 2) {
                    *lane = crate::tex::half_to_f32(u16_at(bytes, i * 2)?);
                }
            }
            _ => return Err(Error::UnsupportedVertexType(self.kind)),
        }
        Ok(value)
    }
}

//! Item model variants, borrowing a complete little-endian IMC file.
//!
//! Subset zero is the default; `variant_count` excludes it. A part is a bit
//! number in the file's part mask, not an equipment slot enum. Records are
//! ordered by ascending enabled bit within each subset. No allocations or I/O
//! are required. Unknown mask bits and raw entry flags are preserved.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    SubsetOutOfBounds,
    PartNotPresent,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(reason) => write!(f, "invalid IMC data: {reason}"),
            Self::SubsetOutOfBounds => f.write_str("IMC subset out of bounds"),
            Self::PartNotPresent => f.write_str("IMC part is not in the part mask"),
        }
    }
}

impl std::error::Error for Error {}

/// One six-byte variant entry. IDs are raw file values, without path resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry {
    pub material_id: u8,
    pub decal_id: u8,
    pub raw_mask: u16,
    pub vfx_id: u8,
    pub raw_animation: u8,
}

impl Entry {
    /// Low ten bits of the stored mask; local MDL attribute bit numbers.
    pub fn attribute_mask(self) -> u16 {
        self.raw_mask & 0x03ff
    }

    /// High six bits of the stored mask, interpreted as the sound selector.
    pub fn sound_id(self) -> u8 {
        (self.raw_mask >> 10) as u8
    }

    /// Low nibble of the material animation byte. Upper bits are preserved in
    /// `raw_animation`; their meaning is not established.
    pub fn material_animation_id(self) -> u8 {
        self.raw_animation & 15
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Imc<'a> {
    variants: u16,
    part_mask: u16,
    entries: &'a [u8],
}

impl<'a> Imc<'a> {
    /// Validates exact file length, including all declared subsets. Trailing
    /// bytes are rejected. A zero part mask is rejected; all 16 bits otherwise
    /// participate in rank calculation, without assuming equipment semantics.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let header = bytes
            .get(..4)
            .ok_or(Error::InvalidData("truncated header"))?;
        let variants = u16::from_le_bytes([header[0], header[1]]);
        let part_mask = u16::from_le_bytes([header[2], header[3]]);
        if part_mask == 0 {
            return Err(Error::InvalidData("empty part mask"));
        }
        let length = (usize::from(variants) + 1)
            .checked_mul(part_mask.count_ones() as usize)
            .and_then(|count| count.checked_mul(6))
            .and_then(|length| length.checked_add(4))
            .ok_or(Error::InvalidData("entry table size overflow"))?;
        if length != bytes.len() {
            return Err(Error::InvalidData("entry table length mismatch"));
        }
        Ok(Self {
            variants,
            part_mask,
            entries: &bytes[4..],
        })
    }

    pub fn variant_count(self) -> u16 {
        self.variants
    }

    pub fn part_mask(self) -> u16 {
        self.part_mask
    }

    pub fn parts(self) -> impl Iterator<Item = u8> {
        (0..16).filter(move |bit| self.part_mask & (1u16 << bit) != 0)
    }

    /// Gets a record by subset (zero = default) and part bit (0..16).
    /// Does not fall back when the requested subset or part is absent.
    pub fn entry(self, subset: u16, part: u8) -> Result<Entry, Error> {
        if subset > self.variants {
            return Err(Error::SubsetOutOfBounds);
        }
        if part >= 16 || self.part_mask & (1u16 << part) == 0 {
            return Err(Error::PartNotPresent);
        }
        let rank = (self.part_mask & ((1u16 << part) - 1)).count_ones() as usize;
        let index = usize::from(subset) * self.part_mask.count_ones() as usize + rank;
        let start = index * 6;
        let b = &self.entries[start..start + 6];
        Ok(Entry {
            material_id: b[0],
            decal_id: b[1],
            raw_mask: u16::from_le_bytes([b[2], b[3]]),
            vfx_id: b[4],
            raw_animation: b[5],
        })
    }
}

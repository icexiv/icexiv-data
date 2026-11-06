//! Borrowed Win32 EQDP sparse equipment-deformer tables (version 0x0101).
//!
//! Entries expose five two-bit slot declarations and the complete raw word.
//! A missing block is a confirmed implicit zero entry, not a filesystem error.
//! Flags describe file declarations, not guaranteed path existence or fallback
//! routing. Race/sex and equipment/accessory context belong to the caller.
use crate::binary::{slice, u16_at};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion(u16),
    BudgetExceeded,
    EntryOutOfBounds { id: usize, count: usize },
}
impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::InvalidData(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(s) => write!(f, "invalid EQDP: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported EQDP version {v:#x}"),
            Self::BudgetExceeded => f.write_str("EQDP input or block budget exceeded"),
            Self::EntryOutOfBounds { id, count } => {
                write!(f, "EQDP entry {id} outside {count} entries")
            }
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_blocks: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 16 * 1024 * 1024,
            max_blocks: 65535,
        }
    }
}

/// Slot indices are file-relative; the table's path supplies its domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    First,
    Second,
    Third,
    Fourth,
    Fifth,
}

/// Slots in `equipmentdeformerparameter/c####.eqdp`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquipmentSlot {
    Head,
    Body,
    Hands,
    Legs,
    Feet,
}
impl From<EquipmentSlot> for Slot {
    fn from(value: EquipmentSlot) -> Self {
        match value {
            EquipmentSlot::Head => Self::First,
            EquipmentSlot::Body => Self::Second,
            EquipmentSlot::Hands => Self::Third,
            EquipmentSlot::Legs => Self::Fourth,
            EquipmentSlot::Feet => Self::Fifth,
        }
    }
}

/// Slots in `accessorydeformerparameter/c####.eqdp`. Left/right describe
/// the model slot, not a requirement for separately named materials.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessorySlot {
    Ears,
    Neck,
    Wrists,
    RightRing,
    LeftRing,
}
impl From<AccessorySlot> for Slot {
    fn from(value: AccessorySlot) -> Self {
        match value {
            AccessorySlot::Ears => Self::First,
            AccessorySlot::Neck => Self::Second,
            AccessorySlot::Wrists => Self::Third,
            AccessorySlot::RightRing => Self::Fourth,
            AccessorySlot::LeftRing => Self::Fifth,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotFlags(u8);
impl SlotFlags {
    pub fn raw(self) -> u8 {
        self.0
    }
    /// Low-bit material declaration (inferred meaning). This is not a path
    /// existence check; its complete fallback semantics remain unverified.
    pub fn material(self) -> bool {
        self.0 & 1 != 0
    }
    /// High-bit model declaration, supported by indexed-path correlation, not
    /// a guarantee that a directly named MDL exists.
    pub fn model(self) -> bool {
        self.0 & 2 != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entry {
    pub raw: u16,
    pub implicit_zero: bool,
}
impl Entry {
    /// Select using the equipment-domain slot interpretation.
    pub fn equipment_slot(self, slot: EquipmentSlot) -> SlotFlags {
        self.slot(slot.into())
    }
    /// Select using the accessory-domain slot interpretation.
    pub fn accessory_slot(self, slot: AccessorySlot) -> SlotFlags {
        self.slot(slot.into())
    }
    pub fn slot(self, slot: Slot) -> SlotFlags {
        SlotFlags(((self.raw >> (slot as u16 * 2)) & 3) as u8)
    }
    pub fn unknown_bits(self) -> u16 {
        self.raw & !0x03ff
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Eqdp<'a> {
    block_size: usize,
    pointers: &'a [u8],
    body: &'a [u8],
    used_end: usize,
    entry_count: usize,
}
impl<'a> Eqdp<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded);
        }
        let header = slice(bytes, 0, 6)?;
        let version = u16_at(header, 0)?;
        if version != 0x0101 {
            return Err(Error::UnsupportedVersion(version));
        }
        let block_size = usize::from(u16_at(header, 2)?);
        let count = usize::from(u16_at(header, 4)?);
        if block_size == 0 {
            return Err(Error::InvalidData("zero block size"));
        }
        if count > limits.max_blocks {
            return Err(Error::BudgetExceeded);
        }
        let entry_count = count
            .checked_mul(block_size)
            .ok_or(Error::InvalidData("entry count overflow"))?;
        let pointers = slice(bytes, 6, count * 2)?;
        let body = bytes
            .get(6 + count * 2..)
            .ok_or(Error::InvalidData("body offset"))?;
        let mut used_end = 0;
        for p in pointers.chunks_exact(2) {
            let pointer = u16::from_le_bytes([p[0], p[1]]);
            if pointer == u16::MAX {
                continue;
            }
            let end = usize::from(pointer)
                .checked_add(block_size)
                .and_then(|n| n.checked_mul(2))
                .ok_or(Error::InvalidData("block span overflow"))?;
            if end > body.len() {
                return Err(Error::InvalidData("block exceeds body"));
            }
            used_end = used_end.max(end);
        }
        Ok(Self {
            block_size,
            pointers,
            body,
            used_end,
            entry_count,
        })
    }
    pub fn block_size(self) -> usize {
        self.block_size
    }
    pub fn block_count(self) -> usize {
        self.pointers.len() / 2
    }
    pub fn entry_count(self) -> usize {
        self.entry_count
    }
    /// Returns None only for the 0xffff absent-block sentinel. Other invalid
    /// block indices return an error. Offsets count u16 entries, not bytes.
    pub fn block_entry_offset(self, block: usize) -> Result<Option<u16>> {
        if block >= self.block_count() {
            return Err(Error::InvalidData("block index out of bounds"));
        }
        let p = u16_at(self.pointers, block * 2)?;
        Ok((p != u16::MAX).then_some(p))
    }
    pub fn entry(self, id: usize) -> Result<Entry> {
        if id >= self.entry_count {
            return Err(Error::EntryOutOfBounds {
                id,
                count: self.entry_count,
            });
        }
        let Some(offset) = self.block_entry_offset(id / self.block_size)? else {
            return Ok(Entry {
                raw: 0,
                implicit_zero: true,
            });
        };
        let at = (usize::from(offset) + id % self.block_size) * 2;
        Ok(Entry {
            raw: u16_at(self.body, at)?,
            implicit_zero: false,
        })
    }
    pub fn trailing_bytes(self) -> &'a [u8] {
        &self.body[self.used_end..]
    }
}

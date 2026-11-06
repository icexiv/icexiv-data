//! Borrowed Win32 EQP compressed blocks and slot display-rule declarations.
//!
//! The first u64 is the 64-block presence bitmap, occupying reserved logical
//! entry zero. Each stored block has 160 u64 words. Missing blocks return None,
//! not invented zero/default flags. Exact compressed length is checked before
//! any lookup. Flags are declarations, not resolved character visibility;
//! enabling, cross-slot precedence and fallback belong to the consumer.
//! In the observed Win64 consumer, body/leg tail declarations are gated by
//! their own enabled fields. The shared tail/zear model slot has a separate
//! type-8 ear branch using head and character state. The raw `show_tail` and
//! ear/hat getters neither apply those gates nor decide model visibility.
use crate::binary::slice;
use std::fmt;
pub const BLOCK_SIZE: usize = 160;
pub const ENTRY_COUNT: usize = 64 * BLOCK_SIZE;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    ReservedEntry,
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
            Self::InvalidData(s) => write!(f, "invalid EQP: {s}"),
            Self::ReservedEntry => f.write_str("EQP entry zero stores the block bitmap"),
            Self::IndexOutOfBounds => f.write_str("EQP entry index out of bounds"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn word(bytes: &[u8], at: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        slice(bytes, at, 8)?
            .try_into()
            .map_err(|_| Error::InvalidData("word width"))?,
    ))
}

macro_rules! bits {
    ($name:ident,$raw:ty,{$($method:ident:$bit:literal),*$(,)?}) => {
        #[derive(Clone,Copy,Debug,Eq,PartialEq)]
        pub struct $name(pub $raw);
        impl $name {$(pub fn $method(self)->bool{self.0 & (1 << $bit)!=0})*}
    };
}
bits!(BodyFlags,u16,{
    enabled:0,hide_waist:1,hide_short_gloves:3,hide_mid_gloves:5,hide_long_gloves:6,hide_gorget:7,
    show_legs:8,show_hands:9,show_head:10,show_necklace:11,show_bracelet:12,show_tail:13,disable_breast_physics:14
});
bits!(LegFlags,u8,{enabled:0,hide_kneepads:1,hide_short_boot:2,hide_half_boot:3,show_feet:5,show_tail:6});
bits!(HandFlags,u8,{enabled:0,hide_elbow:1,hide_forearm:2,show_bracelet:4,show_left_ring:5,show_right_ring:6});
bits!(FootFlags,u8,{enabled:0,hide_knee:1,hide_calf:2,hide_ankle:3});
bits!(HeadFlags,u32,{
    enabled:0,hide_scalp:1,hide_hair:2,show_hair_override:3,hide_neck:4,show_necklace:5,show_earrings:7,
    show_human_earrings:8,show_aura_earrings:9,show_human_ears:10,show_miqote_ears:11,show_aura_ears:12,show_viera_ears:13,
    show_hrothgar_hat:16,show_viera_hat:17
});
impl HeadFlags {
    /// Coupled bits 1..3 (global bits 41..43), retained as codes 0..7.
    /// The individual public bit names are raw views, not independent hair
    /// visibility decisions. This does not evaluate race or character state.
    pub fn hair_coverage_code(self) -> u8 {
        ((self.0 >> 1) & 7) as u8
    }
}
impl BodyFlags {
    /// Select this record's head parameter region when combining equipment
    /// declarations. This bit alone is not final head visibility.
    pub fn uses_head_parameters(self) -> bool {
        self.0 & (1 << 10) != 0
    }
    pub fn uses_hand_parameters(self) -> bool {
        self.0 & (1 << 9) != 0
    }
    pub fn uses_leg_parameters(self) -> bool {
        self.0 & (1 << 8) != 0
    }
    /// The coupled two-bit field used by glove culling comparisons.
    pub fn glove_culling_code(self) -> u8 {
        ((self.0 >> 3) & 3) as u8
    }
}
impl LegFlags {
    pub fn uses_foot_parameters(self) -> bool {
        self.0 & (1 << 5) != 0
    }
    pub fn boot_culling_code(self) -> u8 {
        (self.0 >> 2) & 3
    }
}
impl HandFlags {
    /// Coupled bits 1..2. Retains codes 0..3 without resolving visibility.
    pub fn coverage_selector(self) -> u8 {
        (self.0 >> 1) & 3
    }
    pub fn coverage_comparison_bit(self) -> bool {
        self.0 & 8 != 0
    }
}
impl FootFlags {
    pub fn coverage_selector(self) -> u8 {
        (self.0 >> 1) & 3
    }
    pub fn coverage_comparison_bit(self) -> bool {
        self.0 & 8 != 0
    }
}
/// Raw combined declarations; field names follow public format observations,
/// without claiming that every rule was exercised in a running game.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Flags(pub u64);
impl Flags {
    pub fn body(self) -> BodyFlags {
        BodyFlags(self.0 as u16)
    }
    pub fn legs(self) -> LegFlags {
        LegFlags((self.0 >> 16) as u8)
    }
    pub fn hands(self) -> HandFlags {
        HandFlags((self.0 >> 24) as u8)
    }
    pub fn feet(self) -> FootFlags {
        FootFlags((self.0 >> 32) as u8)
    }
    pub fn head(self) -> HeadFlags {
        HeadFlags((self.0 >> 40) as u32)
    }
    pub fn bit(self, index: u8) -> Option<bool> {
        (index < 64).then(|| self.0 & (1u64 << index) != 0)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Eqp<'a> {
    bytes: &'a [u8],
    bitmap: u64,
}
impl<'a> Eqp<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let bitmap = word(bytes, 0)?;
        if bitmap & 1 == 0 {
            return Err(Error::InvalidData("first block missing"));
        }
        if bytes.len() != bitmap.count_ones() as usize * BLOCK_SIZE * 8 {
            return Err(Error::InvalidData("compressed block length mismatch"));
        }
        Ok(Self { bytes, bitmap })
    }
    pub fn block_bitmap(self) -> u64 {
        self.bitmap
    }
    pub fn stored_block_count(self) -> usize {
        self.bitmap.count_ones() as usize
    }
    /// Logical equipment set IDs are 1..10240, excluding reserved entry zero.
    /// A valid ID with an absent compressed block returns None.
    pub fn entry(self, id: usize) -> Result<Option<Flags>> {
        if id >= ENTRY_COUNT {
            return Err(Error::IndexOutOfBounds);
        }
        if id == 0 {
            return Err(Error::ReservedEntry);
        }
        let block = id / BLOCK_SIZE;
        if self.bitmap & (1u64 << block) == 0 {
            return Ok(None);
        }
        let before = self.bitmap & ((1u64 << block) - 1);
        let physical = before.count_ones() as usize * BLOCK_SIZE + id % BLOCK_SIZE;
        Ok(Some(Flags(word(self.bytes, physical * 8)?)))
    }
}

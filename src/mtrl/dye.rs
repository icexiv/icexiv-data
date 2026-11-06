//! Explicit STM overrides of a selected 32-half intermediate color row.
//!
//! The source is immutable and no float round trip is used. Callers choose the
//! destination profile independently of the MTRL storage layout and supply the
//! corresponding STM. [`super::ColorTable::to_canonical`] converts file rows;
//! [`apply_table`] evaluates both stain slots from that immutable base, including
//! reset and confirmed missing-template behavior. The strict single-row helpers
//! expose individual overrides without whole-material policy.

use super::{CanonicalColorTable, DyeOverrideProfile, DyeRow, color};
use crate::stm::{self, Stm};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    MissingTemplates,
    InvalidStainId(u8),
    TemplateLayoutMismatch,
    MissingTemplate(u16),
    ChannelNotEncoded,
    InvalidChannel(u8),
    Stm(stm::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTemplates => f.write_str("STM is required for the selected stain rows"),
            Self::InvalidStainId(id) => write!(f, "invalid material stain ID {id}"),
            Self::TemplateLayoutMismatch => {
                f.write_str("STM layout disagrees with override profile")
            }
            Self::MissingTemplate(id) => write!(f, "missing STM template {id}"),
            Self::ChannelNotEncoded => f.write_str("dye row does not encode a channel"),
            Self::InvalidChannel(id) => write!(f, "invalid two-bit dye channel {id}"),
            Self::Stm(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Stm(error) => Some(error),
            _ => None,
        }
    }
}
impl From<stm::Error> for Error {
    fn from(error: stm::Error) -> Self {
        Self::Stm(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppliedRow {
    pub half_bits: [u16; 32],
    /// Lanes written, including writes of a value identical to the source.
    /// This can be smaller than the declaration mask (all-zero specular RGB).
    pub written_lane_mask: u32,
}

/// Complete row-major color output from a pair of material stain requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedTable {
    pub half_bits: [[u16; 32]; 32],
    pub written_lane_masks: [u32; 32],
    /// Bit n marks a requested row whose template ID was absent from a valid STM.
    /// Its base colors are retained. Decode errors never set this bit instead of
    /// returning an error; an unavailable STM is [`Error::MissingTemplates`].
    pub missing_template_rows: u32,
}

impl AppliedTable {
    pub fn to_le_bytes(&self) -> [u8; 2048] {
        color::table_bytes(&self.half_bits)
    }

    pub fn values(&self) -> [[f32; 32]; 32] {
        color::table_values(&self.half_bits)
    }
}

/// Evaluate stain slots 0 and 1 from an immutable, converted base table.
///
/// Stain 0 retains the base row (reset). Legacy records address slot 0; encoded
/// channels 2/3 have no matching slot. Rows without flags or records are retained.
/// A confirmed missing template retains the row and is reported in the result.
/// No STM is needed when no row requests an override. Active requests require an
/// STM matching `profile`; malformed columns and invalid stains are errors.
/// IDs 254/255 are rejected rather than treated as the native invalid-ID sentinel.
/// All output is owned, so a failure cannot partly mutate the base or caller data.
pub fn apply_table(
    base: &CanonicalColorTable,
    profile: DyeOverrideProfile,
    templates: Option<Stm<'_>>,
    stains: [u8; 2],
) -> Result<AppliedTable, Error> {
    for id in stains {
        if id >= 254 {
            return Err(Error::InvalidStainId(id));
        }
    }
    let mut result = AppliedTable {
        half_bits: *base.half_bits(),
        written_lane_masks: [0; 32],
        missing_template_rows: 0,
    };
    for (index, dye) in base.dye_rows().iter().enumerate() {
        let Some(dye) = *dye else { continue };
        let Some(&stain) = stains.get(usize::from(dye.channel().unwrap_or(0))) else {
            continue;
        };
        if stain == 0 || dye.flags() == 0 {
            continue;
        }
        let templates = templates.ok_or(Error::MissingTemplates)?;
        match apply_selected_row(
            &result.half_bits[index],
            dye,
            profile,
            templates,
            u16::from(stain),
        ) {
            Ok(row) => {
                result.half_bits[index] = row.half_bits;
                result.written_lane_masks[index] = row.written_lane_mask;
            }
            Err(Error::MissingTemplate(_)) => result.missing_template_rows |= 1 << index,
            Err(error) => return Err(error),
        }
    }
    Ok(result)
}

/// Apply a caller-selected row, without testing its encoded channel.
///
/// The template ID comes from `dye`; missing IDs and invalid one-based stains
/// are errors, even when no override flag is set. All template columns must
/// admit the requested stain. There are no caller-buffer writes on failure.
/// Unselected lanes and selected specular RGB with three raw `0x0000` values
/// retain their source bits; `0x8000` is not the all-zero sentinel. Other selected
/// zero values are written. Half NaN payloads, signed zero and HDR bits survive.
pub fn apply_selected_row(
    base: &[u16; 32],
    dye: DyeRow,
    profile: DyeOverrideProfile,
    templates: Stm<'_>,
    stain_id: u16,
) -> Result<AppliedRow, Error> {
    let layout = match profile {
        DyeOverrideProfile::Legacy => stm::Layout::Legacy,
        DyeOverrideProfile::Dawntrail => stm::Layout::Dawntrail,
    };
    if templates.layout() != layout {
        return Err(Error::TemplateLayoutMismatch);
    }
    let template = templates
        .find_template(u32::from(dye.template()))?
        .ok_or(Error::MissingTemplate(dye.template()))?;
    let capacity = template.stain_capacity()?;
    if stain_id == 0 || usize::from(stain_id) > capacity {
        return Err(stm::Error::StainOutOfBounds {
            id: stain_id,
            capacity,
        }
        .into());
    }
    let mut result = AppliedRow {
        half_bits: *base,
        written_lane_mask: 0,
    };
    for (column, &(start, width)) in profile.column_spans().iter().enumerate() {
        if dye.flags() & (1 << column) == 0 {
            continue;
        }
        let bits = template.column(column)?.half_bits(stain_id)?;
        if column == 1 && bits == [0; 3] {
            continue;
        }
        result.half_bits[start..start + width].copy_from_slice(&bits[..width]);
        result.written_lane_mask |= ((1 << width) - 1) << start;
    }
    Ok(result)
}

/// Test a two-bit encoded channel, then apply the selected row.
///
/// A mismatch is `Ok(None)` and does not inspect STM/template/stain inputs.
/// Legacy dye words do not encode a channel: callers must select them explicitly
/// with `apply_selected_row`. Channel 2/3 are kept as wire values, not assigned
/// to application stain slots. This function does not select a shader/profile.
pub fn apply_channel_row(
    base: &[u16; 32],
    dye: DyeRow,
    profile: DyeOverrideProfile,
    templates: Stm<'_>,
    stain_id: u16,
    channel: u8,
) -> Result<Option<AppliedRow>, Error> {
    if channel > 3 {
        return Err(Error::InvalidChannel(channel));
    }
    let encoded = dye.channel().ok_or(Error::ChannelNotEncoded)?;
    if encoded != channel {
        return Ok(None);
    }
    apply_selected_row(base, dye, profile, templates, stain_id).map(Some)
}

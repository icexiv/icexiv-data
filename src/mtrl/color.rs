use super::{ColorLayout, ColorRow, ColorTable, DyeRow, Error, Mtrl, Result};

/// A 32-row, 32-binary16-lane base color table and its associated dye records.
///
/// Legacy rows are converted individually; rows 16..32 copy converted row zero
/// and have no dye record. Dawntrail colors retain every original bit. The base
/// is immutable: each [`super::dye::apply_table`] request starts here, including
/// requests that remove a stain. Values retain the half domain, without clamping,
/// sRGB conversion or shader evaluation. Storage is fixed-size and owns no I/O.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalColorTable {
    half_bits: [[u16; 32]; 32],
    dye_rows: [Option<DyeRow>; 32],
}

impl CanonicalColorTable {
    pub fn half_bits(&self) -> &[[u16; 32]; 32] {
        &self.half_bits
    }

    pub fn dye_rows(&self) -> &[Option<DyeRow>; 32] {
        &self.dye_rows
    }

    /// Row-major little-endian binary16 data (8 RGBA texels per row).
    pub fn to_le_bytes(&self) -> [u8; 2048] {
        table_bytes(&self.half_bits)
    }

    /// Floating-point values, preserving HDR, negatives and non-finite values.
    /// Use the half bits or bytes when NaN payload identity matters.
    pub fn values(&self) -> [[f32; 32]; 32] {
        table_values(&self.half_bits)
    }
}

impl ColorTable<'_> {
    /// Convert file-layout colors and retain their dye declarations.
    ///
    /// This explicit conversion accepts a parsed dataset independently of the
    /// material's inline flag. [`Mtrl::canonical_inline_color_table`] also checks
    /// the inline declaration. Dye flags do not alter the base colors.
    pub fn to_canonical(self) -> Result<CanonicalColorTable> {
        let mut result = CanonicalColorTable {
            half_bits: [[0; 32]; 32],
            dye_rows: [None; 32],
        };
        for index in 0..32 {
            let source = if index < self.row_count() { index } else { 0 };
            result.half_bits[index] = self.row(source)?.canonical_half_bits();
            if index < self.row_count() {
                result.dye_rows[index] = self.dye_row(index)?;
            }
        }
        Ok(result)
    }
}

impl ColorRow<'_> {
    /// Convert to the 32-half intermediate layout, preserving copied bits.
    ///
    /// Legacy lane 7 is `1 - source[3]`: binary32 round-to-nearest/ties-to-even,
    /// then binary16 truncation toward zero. Exact cancellation produces +0;
    /// infinities change sign, and NaNs become a same-sign quiet `0x7e00` NaN.
    /// The integer evaluation is independent of host floating-point controls.
    /// This specifies the nearest-even, masked-exception arithmetic profile;
    /// it does not reproduce arbitrary client MXCSR settings.
    pub fn canonical_half_bits(self) -> [u16; 32] {
        let mut source = [0; 32];
        for (half, bytes) in source.iter_mut().zip(self.raw().chunks_exact(2)) {
            *half = u16::from_le_bytes([bytes[0], bytes[1]]);
        }
        match self.layout() {
            ColorLayout::Dawntrail => source,
            ColorLayout::Legacy => [
                source[0],
                source[1],
                source[2],
                0x3c00,
                0x3c00,
                0x3c00,
                0x3c00,
                legacy_complement(source[3]),
                source[8],
                source[9],
                source[10],
                0x3c00,
                source[4],
                source[5],
                source[7],
                0,
                0x3800,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                source[11],
                0x3c00,
                0,
                source[12],
                source[13],
                source[14],
                source[15],
            ],
        }
    }
}

impl Mtrl<'_> {
    /// Convert a declared inline color table. `None` means no inline declaration;
    /// an external color texture may still be referenced by the material.
    /// A declaration without its dataset is an error, not absence.
    pub fn canonical_inline_color_table(&self) -> Result<Option<CanonicalColorTable>> {
        if self.table_flags().is_some_and(|flags| flags & 4 != 0)
            && self.additional_data().len() < 4
        {
            return Err(Error::InvalidData("truncated inline color declaration"));
        }
        if self.inline_color_table_declaration().is_none() {
            return Ok(None);
        }
        self.color_table()?
            .ok_or(Error::InvalidData(
                "inline color declaration without dataset",
            ))?
            .to_canonical()
            .map(Some)
    }
}

pub(super) fn table_bytes(rows: &[[u16; 32]; 32]) -> [u8; 2048] {
    let mut bytes = [0; 2048];
    for (out, half) in bytes.chunks_exact_mut(2).zip(rows.iter().flatten()) {
        out.copy_from_slice(&half.to_le_bytes());
    }
    bytes
}

pub(super) fn table_values(rows: &[[u16; 32]; 32]) -> [[f32; 32]; 32] {
    rows.map(|row| row.map(crate::tex::half_to_f32))
}

// Mathematical fixed-point evaluation, not a general f32 -> half converter.
// Every finite binary16 value is an integer multiple of 2^-24. The nonzero
// result of 1 - x has magnitude >= 2^-11 and <= 65505, so its half exponent
// is normal: no half-underflow or overflow branch is reachable here.
fn legacy_complement(bits: u16) -> u16 {
    let exponent = (bits >> 10) & 31;
    let fraction = bits & 1023;
    if exponent == 31 {
        return if fraction == 0 {
            bits ^ 0x8000
        } else {
            (bits & 0x8000) | 0x7e00
        };
    }
    let magnitude = if exponent == 0 {
        i64::from(fraction)
    } else {
        i64::from(1024 + fraction) << (exponent - 1)
    };
    let exact = (1i64 << 24)
        - if bits & 0x8000 == 0 {
            magnitude
        } else {
            -magnitude
        };
    if exact == 0 {
        return 0;
    }
    let sign = if exact < 0 { 0x8000 } else { 0 };
    let mut magnitude = exact.unsigned_abs();
    // Round the integer to 24 significant bits (binary32 nearest/even).
    let discarded = (64 - magnitude.leading_zeros()).saturating_sub(24);
    if discarded != 0 {
        let unit = 1u64 << discarded;
        let quotient = magnitude >> discarded;
        let remainder = magnitude & (unit - 1);
        let increment = remainder > unit / 2 || (remainder == unit / 2 && quotient & 1 != 0);
        magnitude = (quotient + u64::from(increment)) << discarded;
    }
    // Truncate to 11 significant bits and encode the normal binary16 result.
    let highest = 63 - magnitude.leading_zeros();
    let significand = (magnitude >> (highest - 10)) as u16;
    sign | (((highest - 9) as u16) << 10) | (significand - 1024)
}

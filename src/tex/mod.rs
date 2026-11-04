//! Checked parsing of extracted Win32 TEX bytes and CPU decoding to RGBA8.
//!
//! Supports linear, ordinary 2D textures in L8, A8, B4G4R4A4, B5G5R5A1,
//! B8G8R8A8, B8G8R8X8, BC1, BC2, BC3, BC4, BC5, BC7 and the R32F, R16G16F,
//! R32G32F, R16G16B16A16F and R32G32B32A32F float layouts. Other formats and
//! texture layouts return explicit errors. No I/O, GPU, image encoder or schema
//! is used. SqPack type-4 extraction is a separate operation, outside this
//! module's scope.
//!
//! All declared mip spans are validated at parse time without allocating pixel
//! storage. Decoding requires an explicit output byte budget per mip. Output is
//! tightly packed, row-major RGBA8 with straight alpha, no vertical flip, gamma
//! conversion or premultiplication. A8 uses white RGB and the stored alpha.
//! Packed channels expand by bit replication; BC interpolation rounds down.
//! BC4 repeats its single channel across RGB, BC5 stores red and green with blue
//! at zero. Float samples are clamped to `0..=1` and rounded to eight bits;
//! one-channel floats repeat, two-channel floats leave blue at zero.
//!
//! ```no_run
//! use icexiv_data::tex::Tex;
//!
//! # fn example(bytes: &[u8]) -> Result<(), icexiv_data::tex::Error> {
//! let texture = Tex::parse(bytes)?;
//! let mip = texture.mip(0)?;
//! let rgba = mip.decode_rgba8(16 * 1024 * 1024)?;
//! // Pass mip.width(), mip.height() and rgba to an encoder in the application.
//! # Ok(())
//! # }
//! ```

mod bc;
mod bc7;

use binrw::BinRead;
use std::{fmt, io::Cursor};

const HEADER_SIZE: usize = 80;
const MAX_MIPS: usize = 13;
const TYPE_MASK: u32 = 0x13c0_0000;
const TYPE_2D: u32 = 0x0080_0000;
const SWIZZLE: u32 = 0x0400_0000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedFormat(u32),
    UnsupportedLayout(u32),
    MipOutOfBounds { index: usize, count: usize },
    OutputTooLarge { requested: usize, limit: usize },
    AllocationFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(reason) => write!(f, "invalid TEX data: {reason}"),
            Self::UnsupportedFormat(value) => write!(f, "unsupported TEX format {value:#x}"),
            Self::UnsupportedLayout(value) => write!(f, "unsupported TEX layout {value:#x}"),
            Self::MipOutOfBounds { index, count } => {
                write!(f, "mip {index} exceeds mip count {count}")
            }
            Self::OutputTooLarge { requested, limit } => {
                write!(f, "TEX output of {requested} bytes exceeds limit {limit}")
            }
            Self::AllocationFailed => f.write_str("TEX output allocation failed"),
        }
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

/// TEX format identifiers; component names follow DXGI order (low bits first
/// for packed formats), not the little-endian integer's printed hex order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Format {
    L8 = 0x1130,
    A8 = 0x1131,
    B4G4R4A4 = 0x1440,
    B5G5R5A1 = 0x1441,
    B8G8R8A8 = 0x1450,
    B8G8R8X8 = 0x1451,
    Bc1 = 0x3420,
    Bc2 = 0x3430,
    Bc3 = 0x3431,
    Bc4 = 0x6120,
    Bc5 = 0x6230,
    Bc7 = 0x6432,
    R32F = 0x2150,
    R16G16F = 0x2250,
    R32G32F = 0x2260,
    R16G16B16A16F = 0x2460,
    R32G32B32A32F = 0x2470,
}

impl Format {
    fn from_id(id: u32) -> Result<Self> {
        Ok(match id {
            0x1130 => Self::L8,
            0x1131 => Self::A8,
            0x1440 => Self::B4G4R4A4,
            0x1441 => Self::B5G5R5A1,
            0x1450 => Self::B8G8R8A8,
            0x1451 => Self::B8G8R8X8,
            0x2150 => Self::R32F,
            0x2250 => Self::R16G16F,
            0x2260 => Self::R32G32F,
            0x2460 => Self::R16G16B16A16F,
            0x2470 => Self::R32G32B32A32F,
            0x3420 => Self::Bc1,
            0x3430 => Self::Bc2,
            0x3431 => Self::Bc3,
            0x6120 => Self::Bc4,
            0x6230 => Self::Bc5,
            0x6432 => Self::Bc7,
            _ => return Err(Error::UnsupportedFormat(id)),
        })
    }

    fn stored_size(self, width: usize, height: usize) -> Result<usize> {
        let (columns, rows, unit) = match self {
            Self::L8 | Self::A8 => (width, height, 1),
            Self::B4G4R4A4 | Self::B5G5R5A1 => (width, height, 2),
            Self::B8G8R8A8 | Self::B8G8R8X8 | Self::R32F | Self::R16G16F => (width, height, 4),
            Self::R32G32F | Self::R16G16B16A16F => (width, height, 8),
            Self::R32G32B32A32F => (width, height, 16),
            Self::Bc1 | Self::Bc4 => (width.div_ceil(4), height.div_ceil(4), 8),
            Self::Bc2 | Self::Bc3 | Self::Bc5 | Self::Bc7 => {
                (width.div_ceil(4), height.div_ceil(4), 16)
            }
        };
        columns
            .checked_mul(rows)
            .and_then(|size| size.checked_mul(unit))
            .ok_or(Error::InvalidData("mip byte size overflow"))
    }
}

#[derive(BinRead)]
#[br(little)]
struct HeaderRaw {
    attributes: u32,
    format: u32,
    width: u16,
    height: u16,
    depth: u16,
    mip_flags: u8,
    array_size: u8,
    _lod_indices: [u32; 3],
    offsets: [u32; MAX_MIPS],
}

/// Borrowed texture with every declared mip checked. TEX has no magic;
/// successful parsing validates the supported layout, not file identity.
#[derive(Debug)]
pub struct Tex<'data> {
    attributes: u32,
    mip_unknown_flag: bool,
    mip_count: usize,
    mips: [Mip<'data>; MAX_MIPS],
}

impl<'data> Tex<'data> {
    pub fn parse(bytes: &'data [u8]) -> Result<Self> {
        let header = HeaderRaw::read(&mut Cursor::new(bytes))
            .map_err(|_| Error::InvalidData("truncated header"))?;
        if header.attributes & TYPE_MASK != TYPE_2D || header.attributes & SWIZZLE != 0 {
            return Err(Error::UnsupportedLayout(header.attributes));
        }
        if header.depth != 1 || header.array_size != 0 {
            return Err(Error::InvalidData(
                "2D texture must have depth 1 and no array",
            ));
        }
        if header.width == 0 || header.height == 0 {
            return Err(Error::InvalidData("zero texture dimensions"));
        }
        let format = Format::from_id(header.format)?;
        let mip_count = usize::from(header.mip_flags & 0x7f);
        let dimension_mips = u16::BITS - header.width.max(header.height).leading_zeros();
        if mip_count == 0 || mip_count > MAX_MIPS || mip_count > dimension_mips as usize {
            return Err(Error::InvalidData("invalid mip count"));
        }
        let mut mips = [Mip {
            width: 0,
            height: 0,
            format,
            data: &[],
        }; MAX_MIPS];
        let mut previous_end = HEADER_SIZE;
        for (index, mip) in mips.iter_mut().enumerate().take(mip_count) {
            let width = (header.width >> index).max(1);
            let height = (header.height >> index).max(1);
            let size = format.stored_size(usize::from(width), usize::from(height))?;
            let start = usize::try_from(header.offsets[index])
                .map_err(|_| Error::InvalidData("mip offset overflow"))?;
            if start < previous_end {
                return Err(Error::InvalidData("mip overlaps header or previous mip"));
            }
            let end = start
                .checked_add(size)
                .ok_or(Error::InvalidData("mip range overflow"))?;
            let data = bytes
                .get(start..end)
                .ok_or(Error::InvalidData("truncated mip payload"))?;
            *mip = Mip {
                width,
                height,
                format,
                data,
            };
            previous_end = end;
        }
        Ok(Self {
            attributes: header.attributes,
            mip_unknown_flag: header.mip_flags & 0x80 != 0,
            mip_count,
            mips,
        })
    }

    pub fn attributes(&self) -> u32 {
        self.attributes
    }

    /// Preserve the unknown high flag separately from the low seven count bits.
    pub fn mip_unknown_flag(&self) -> bool {
        self.mip_unknown_flag
    }

    pub fn format(&self) -> Format {
        self.mips[0].format
    }

    pub fn width(&self) -> u16 {
        self.mips[0].width
    }

    pub fn height(&self) -> u16 {
        self.mips[0].height
    }

    pub fn mip_count(&self) -> usize {
        self.mip_count
    }

    pub fn mip(&self, index: usize) -> Result<Mip<'data>> {
        self.mips[..self.mip_count]
            .get(index)
            .copied()
            .ok_or(Error::MipOutOfBounds {
                index,
                count: self.mip_count,
            })
    }

    pub fn mips(&self) -> impl ExactSizeIterator<Item = Mip<'data>> + '_ {
        self.mips[..self.mip_count].iter().copied()
    }
}

/// One validated, tightly packed mip payload, borrowing the input file.
#[derive(Clone, Copy, Debug)]
pub struct Mip<'data> {
    width: u16,
    height: u16,
    format: Format,
    data: &'data [u8],
}

impl<'data> Mip<'data> {
    pub fn width(self) -> u16 {
        self.width
    }

    pub fn height(self) -> u16 {
        self.height
    }

    pub fn format(self) -> Format {
        self.format
    }

    /// Encoded bytes only; excludes gaps, padding and following mip levels.
    pub fn data(self) -> &'data [u8] {
        self.data
    }

    /// Decode with a mandatory per-output byte budget, checked before allocation.
    /// The caller controls aggregate memory when retaining multiple results.
    pub fn decode_rgba8(self, max_output_bytes: usize) -> Result<Vec<u8>> {
        let size = usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(Error::InvalidData("RGBA byte size overflow"))?;
        if size > max_output_bytes {
            return Err(Error::OutputTooLarge {
                requested: size,
                limit: max_output_bytes,
            });
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(size)
            .map_err(|_| Error::AllocationFailed)?;
        output.resize(size, 0);
        match self.format {
            Format::Bc1 | Format::Bc2 | Format::Bc3 | Format::Bc4 | Format::Bc5 => {
                bc::decode(self, &mut output)
            }
            Format::Bc7 => bc7::decode(self, &mut output).map_err(Error::InvalidData)?,
            Format::R32F
            | Format::R16G16F
            | Format::R32G32F
            | Format::R16G16B16A16F
            | Format::R32G32B32A32F => decode_float(self, &mut output),
            format => {
                let stride = match format {
                    Format::L8 | Format::A8 => 1,
                    Format::B4G4R4A4 | Format::B5G5R5A1 => 2,
                    _ => 4,
                };
                for (source, target) in self
                    .data
                    .chunks_exact(stride)
                    .zip(output.chunks_exact_mut(4))
                {
                    let rgba = match format {
                        Format::L8 => [source[0], source[0], source[0], 255],
                        Format::A8 => [255, 255, 255, source[0]],
                        Format::B4G4R4A4 => {
                            let v = u16::from_le_bytes([source[0], source[1]]);
                            [
                                ((v >> 8) & 15) as u8 * 17,
                                ((v >> 4) & 15) as u8 * 17,
                                (v & 15) as u8 * 17,
                                (v >> 12) as u8 * 17,
                            ]
                        }
                        Format::B5G5R5A1 => {
                            let v = u16::from_le_bytes([source[0], source[1]]);
                            [
                                expand5((v >> 10) & 31),
                                expand5((v >> 5) & 31),
                                expand5(v & 31),
                                if v & 0x8000 != 0 { 255 } else { 0 },
                            ]
                        }
                        Format::B8G8R8A8 => [source[2], source[1], source[0], source[3]],
                        Format::B8G8R8X8 => [source[2], source[1], source[0], 255],
                        _ => unreachable!("float and block formats are handled separately"),
                    };
                    target.copy_from_slice(&rgba);
                }
            }
        }
        Ok(output)
    }
}

fn expand5(value: u16) -> u8 {
    ((value << 3) | (value >> 2)) as u8
}

/// Converts float channels to tightly packed RGBA8: clamp to `0..=1`, multiply
/// by 255 and round half up. One channel replicates to RGB; two channels fill
/// RGB with the third channel at zero; four channels are used as stored.
fn decode_float(mip: Mip<'_>, output: &mut [u8]) {
    let (channels, floating) = match mip.format {
        Format::R32F => (1, false),
        Format::R16G16F => (2, true),
        Format::R32G32F => (2, false),
        Format::R16G16B16A16F => (4, true),
        Format::R32G32B32A32F => (4, false),
        _ => unreachable!("only float formats reach this decoder"),
    };
    let stride = channels * if floating { 2 } else { 4 };
    for (source, target) in mip
        .data
        .chunks_exact(stride)
        .zip(output.chunks_exact_mut(4))
    {
        let mut values = [0u8; 4];
        for (channel, value) in values[..channels].iter_mut().enumerate() {
            let sample = if floating {
                let offset = channel * 2;
                half_to_f32(u16::from_le_bytes([source[offset], source[offset + 1]]))
            } else {
                let offset = channel * 4;
                f32::from_bits(u32::from_le_bytes(
                    source[offset..offset + 4].try_into().unwrap(),
                ))
            };
            *value = quantize(sample);
        }
        let rgba = match channels {
            1 => [values[0], values[0], values[0], 255],
            2 => [values[0], values[1], 0, 255],
            _ => values,
        };
        target.copy_from_slice(&rgba);
    }
}

fn quantize(value: f32) -> u8 {
    if value.is_nan() {
        0
    } else {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

fn half_to_f32(half: u16) -> f32 {
    let exponent = (half >> 10) & 31;
    let mantissa = half & 1023;
    let magnitude = match exponent {
        0 => f32::from(mantissa) * 2f32.powi(-24),
        31 if mantissa == 0 => f32::INFINITY,
        31 => f32::NAN,
        _ => (1.0 + f32::from(mantissa) / 1024.0) * 2f32.powi(i32::from(exponent) - 15),
    };
    if half & 0x8000 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

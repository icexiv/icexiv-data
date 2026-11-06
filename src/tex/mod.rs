//! Checked parsing of extracted Win32 TEX/ATEX bytes and CPU texture decoding.
//!
//! AVFX-referenced `.atex` uses the same header and pixel representation. Pass
//! its extracted bytes directly to `Tex::parse`; extension or SqPack resource
//! type does not select the decoder. ATEX can be stored as SqPack type-2.
//!
//! Supports linear 2D, 2D-array, cube and volume textures in L8, A8, B4G4R4A4, B5G5R5A1,
//! B8G8R8A8, B8G8R8X8, BC1, BC2, BC3, BC4, BC5, BC7 and the R16F, R32F, R16G16F,
//! R32G32F, packed R11G11B10F, R16G16B16A16F and R32G32B32A32F float layouts, plus signed BC6H.
//! Other formats and
//! texture layouts return explicit errors. No I/O, GPU, image encoder or schema
//! is used. SqPack type-4 extraction is a separate operation, outside this
//! module's scope.
//!
//! `Tex::parse` validates every declared mip's payload without allocating pixel
//! storage. `Header` validates the header and mip ranges independently of the
//! payload; `Header::mip` binds one complete mip while preserving its source index
//! and mip count. Decoding requires an explicit output byte budget per mip. Output is
//! tightly packed, row-major RGBA8 with straight alpha, no vertical flip, gamma
//! conversion or premultiplication. A8 uses white RGB and the stored alpha.
//! Packed integer channels expand by bit replication; BC interpolation rounds down.
//! BC4 repeats its single channel across RGB, BC5 stores red and green with blue
//! at zero. Float samples are clamped to `0..=1` and rounded to eight bits;
//! one-channel floats repeat, two-channel floats leave blue at zero.
//! `decode_rgba32f` on float formats retains HDR, negative and non-finite values.
//! Mip output concatenates all images in layer/face/z order, never only the first
//! image. `Mip::image` provides an independently budgeted two-dimensional view.
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
mod bc6h;
mod bc7;

use binrw::BinRead;
use std::{fmt, io::Cursor, ops::Range};

const HEADER_SIZE: usize = 80;
const MAX_MIPS: usize = 13;
const TYPE_MASK: u32 = 0x13c0_0000;
const TYPE_2D: u32 = 0x0080_0000;
const TYPE_VOLUME: u32 = 0x0100_0000;
const TYPE_CUBE: u32 = 0x0200_0000;
const TYPE_ARRAY: u32 = 0x1000_0000;
const SWIZZLE: u32 = 0x0400_0000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedFormat(u32),
    UnsupportedLayout(u32),
    UnsupportedCubeImageCount(u8),
    MipOutOfBounds {
        index: usize,
        count: usize,
    },
    MipSizeMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
    ImageOutOfBounds {
        index: usize,
        count: usize,
    },
    SurfaceOutOfBounds {
        layer: usize,
        face: usize,
        z: usize,
    },
    NotCubeTexture,
    FloatOutputUnavailable(u32),
    OutputTooLarge {
        requested: usize,
        limit: usize,
    },
    AllocationFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(reason) => write!(f, "invalid TEX data: {reason}"),
            Self::UnsupportedFormat(value) => write!(f, "unsupported TEX format {value:#x}"),
            Self::UnsupportedLayout(value) => write!(f, "unsupported TEX layout {value:#x}"),
            Self::UnsupportedCubeImageCount(value) => {
                write!(f, "unsupported TEX cube image-count byte {value}")
            }
            Self::MipOutOfBounds { index, count } => {
                write!(f, "mip {index} exceeds mip count {count}")
            }
            Self::MipSizeMismatch {
                index,
                expected,
                actual,
            } => write!(
                f,
                "TEX mip {index} requires {expected} encoded bytes, got {actual}"
            ),
            Self::ImageOutOfBounds { index, count } => {
                write!(f, "image {index} exceeds image count {count}")
            }
            Self::SurfaceOutOfBounds { layer, face, z } => {
                write!(f, "TEX surface ({layer}, {face}, {z}) out of bounds")
            }
            Self::NotCubeTexture => f.write_str("TEX mip is not a cube texture"),
            Self::FloatOutputUnavailable(format) => {
                write!(f, "TEX format {format:#x} has no float-channel decoder")
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
    /// Win64 TEX 0x6330 maps to DXGI BC6H_SF16 (signed RGB half floats).
    Bc6hSf16 = 0x6330,
    Bc7 = 0x6432,
    /// Single IEEE binary16 channel (Win64 DXGI R16_FLOAT).
    R16F = 0x2140,
    R32F = 0x2150,
    R16G16F = 0x2250,
    R32G32F = 0x2260,
    /// Unsigned RGB mini-floats packed into a little-endian u32.
    R11G11B10F = 0x2350,
    R16G16B16A16F = 0x2460,
    R32G32B32A32F = 0x2470,
}

impl Format {
    fn float_layout(self) -> Option<(usize, bool)> {
        Some(match self {
            Self::R16F => (1, true),
            Self::R32F => (1, false),
            Self::R16G16F => (2, true),
            Self::R32G32F => (2, false),
            Self::R16G16B16A16F => (4, true),
            Self::R32G32B32A32F => (4, false),
            _ => return None,
        })
    }
    pub fn float_channels(self) -> Option<usize> {
        if matches!(self, Self::Bc6hSf16 | Self::R11G11B10F) {
            Some(3)
        } else {
            self.float_layout().map(|(channels, _)| channels)
        }
    }
    fn from_id(id: u32) -> Result<Self> {
        Ok(match id {
            0x1130 => Self::L8,
            0x1131 => Self::A8,
            0x1440 => Self::B4G4R4A4,
            0x1441 => Self::B5G5R5A1,
            0x1450 => Self::B8G8R8A8,
            0x1451 => Self::B8G8R8X8,
            0x2140 => Self::R16F,
            0x2150 => Self::R32F,
            0x2250 => Self::R16G16F,
            0x2260 => Self::R32G32F,
            0x2350 => Self::R11G11B10F,
            0x2460 => Self::R16G16B16A16F,
            0x2470 => Self::R32G32B32A32F,
            0x3420 => Self::Bc1,
            0x3430 => Self::Bc2,
            0x3431 => Self::Bc3,
            0x6120 => Self::Bc4,
            0x6230 => Self::Bc5,
            0x6330 => Self::Bc6hSf16,
            0x6432 => Self::Bc7,
            _ => return Err(Error::UnsupportedFormat(id)),
        })
    }

    fn stored_size(self, width: usize, height: usize) -> Result<usize> {
        let (columns, rows, unit) = match self {
            Self::L8 | Self::A8 => (width, height, 1),
            Self::B4G4R4A4 | Self::B5G5R5A1 | Self::R16F => (width, height, 2),
            Self::B8G8R8A8 | Self::B8G8R8X8 | Self::R32F | Self::R16G16F | Self::R11G11B10F => {
                (width, height, 4)
            }
            Self::R32G32F | Self::R16G16B16A16F => (width, height, 8),
            Self::R32G32B32A32F => (width, height, 16),
            Self::Bc1 | Self::Bc4 => (width.div_ceil(4), height.div_ceil(4), 8),
            Self::Bc2 | Self::Bc3 | Self::Bc5 | Self::Bc6hSf16 | Self::Bc7 => {
                (width.div_ceil(4), height.div_ceil(4), 16)
            }
        };
        columns
            .checked_mul(rows)
            .and_then(|size| size.checked_mul(unit))
            .ok_or(Error::InvalidData("mip byte size overflow"))
    }
}

#[derive(BinRead, Clone, Debug)]
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

/// Checked TEX header and the encoded range of every declared mip.
///
/// Parsing copies the original 80 header bytes and validates the layout,
/// dimensions and mip ranges without requiring or allocating pixel storage.
/// [`Self::mip`] borrows a complete mip supplied separately from the header.
#[derive(Clone, Debug)]
pub struct Header {
    raw_bytes: [u8; HEADER_SIZE],
    raw: HeaderRaw,
    layout: Layout,
    format: Format,
    layers: usize,
    faces: usize,
    mip_count: usize,
    packed_volume: bool,
    mips: [MipLayout; MAX_MIPS],
}

#[derive(Clone, Copy, Debug, Default)]
struct MipLayout {
    width: u16,
    height: u16,
    depth: u16,
    plane_size: usize,
    start: usize,
    end: usize,
}

/// Borrowed texture with every declared mip checked. TEX has no magic;
/// successful parsing validates the supported layout, not file identity.
#[derive(Debug)]
pub struct Tex<'data> {
    header: Header,
    data: &'data [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    D2,
    D2Array,
    Cube,
    Volume,
}

/// Cube face identity in the observed Win32 Direct3D convention.
/// This names the file's six images, without rotating/flipping pixels or
/// assigning world-space axes to a caller's environment map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CubeFace {
    PositiveX = 0,
    NegativeX = 1,
    PositiveY = 2,
    NegativeY = 3,
    PositiveZ = 4,
    NegativeZ = 5,
}
impl CubeFace {
    pub const ALL: [Self; 6] = [
        Self::PositiveX,
        Self::NegativeX,
        Self::PositiveY,
        Self::NegativeY,
        Self::PositiveZ,
        Self::NegativeZ,
    ];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

impl Header {
    /// Parse the first 80 bytes, requiring nonoverlapping declared mip ranges.
    /// Bytes after the header are ignored; their presence or length does not
    /// determine whether any mip payload is complete.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::parse_impl(bytes, false)
    }

    /// Explicit Win32 volume profile with every active offset word equal to 80.
    /// Mips occupy consecutive, dimension-derived spans after the header. This
    /// preserves the original offset words. Other offset patterns and non-volume
    /// layouts are rejected. Only the first 80 bytes are required and parsed.
    pub fn parse_packed_volume(bytes: &[u8]) -> Result<Self> {
        Self::parse_impl(bytes, true)
    }

    fn parse_impl(bytes: &[u8], packed_volume: bool) -> Result<Self> {
        let raw_bytes = *bytes
            .first_chunk::<HEADER_SIZE>()
            .ok_or(Error::InvalidData("truncated header"))?;
        let header = HeaderRaw::read(&mut Cursor::new(&raw_bytes))
            .map_err(|_| Error::InvalidData("truncated header"))?;
        if header.attributes & SWIZZLE != 0 {
            return Err(Error::UnsupportedLayout(header.attributes));
        }
        let (layout, layers, faces) = match header.attributes & TYPE_MASK {
            TYPE_2D if header.depth == 1 && header.array_size == 0 => (Layout::D2, 1, 1),
            TYPE_ARRAY if header.depth == 1 && header.array_size != 0 => {
                (Layout::D2Array, usize::from(header.array_size), 1)
            }
            TYPE_CUBE
                if header.depth == 1
                    && matches!(header.array_size, 0 | 6)
                    && header.width == header.height =>
            {
                (Layout::Cube, 1, 6)
            }
            TYPE_VOLUME if header.depth != 0 && header.array_size == 0 => (Layout::Volume, 1, 1),
            TYPE_CUBE if !matches!(header.array_size, 0 | 6) => {
                return Err(Error::UnsupportedCubeImageCount(header.array_size));
            }
            TYPE_2D | TYPE_ARRAY | TYPE_CUBE | TYPE_VOLUME => {
                return Err(Error::InvalidData(
                    "inconsistent texture dimensions or image count",
                ));
            }
            _ => return Err(Error::UnsupportedLayout(header.attributes)),
        };
        if header.width == 0 || header.height == 0 {
            return Err(Error::InvalidData("zero texture dimensions"));
        }
        let format = Format::from_id(header.format)?;
        let mip_count = usize::from(header.mip_flags & 0x7f);
        let dimension_mips = u16::BITS
            - header
                .width
                .max(header.height)
                .max(header.depth)
                .leading_zeros();
        if mip_count == 0 || mip_count > MAX_MIPS || mip_count > dimension_mips as usize {
            return Err(Error::InvalidData("invalid mip count"));
        }
        if packed_volume
            && (layout != Layout::Volume
                || header.offsets[..mip_count]
                    .iter()
                    .any(|&offset| offset != HEADER_SIZE as u32))
        {
            return Err(Error::InvalidData("not the packed volume header profile"));
        }
        let mut mips = [MipLayout::default(); MAX_MIPS];
        let mut previous_end = HEADER_SIZE;
        for (index, mip) in mips.iter_mut().enumerate().take(mip_count) {
            let width = (header.width >> index).max(1);
            let height = (header.height >> index).max(1);
            let depth = if layout == Layout::Volume {
                (header.depth >> index).max(1)
            } else {
                1
            };
            let plane_size = format.stored_size(usize::from(width), usize::from(height))?;
            let size = plane_size
                .checked_mul(layers)
                .and_then(|n| n.checked_mul(faces))
                .and_then(|n| n.checked_mul(usize::from(depth)))
                .ok_or(Error::InvalidData("mip image span overflow"))?;
            let start = if packed_volume {
                previous_end
            } else {
                usize::try_from(header.offsets[index])
                    .map_err(|_| Error::InvalidData("mip offset overflow"))?
            };
            if start < previous_end {
                return Err(Error::InvalidData("mip overlaps header or previous mip"));
            }
            let end = start
                .checked_add(size)
                .ok_or(Error::InvalidData("mip range overflow"))?;
            *mip = MipLayout {
                width,
                height,
                depth,
                plane_size,
                start,
                end,
            };
            previous_end = end;
        }
        Ok(Self {
            raw_bytes,
            raw: header,
            layout,
            format,
            layers,
            faces,
            mip_count,
            packed_volume,
            mips,
        })
    }

    /// Original header bytes, including flags, LOD indices and unused offsets.
    pub fn raw_bytes(&self) -> &[u8; HEADER_SIZE] {
        &self.raw_bytes
    }
    pub fn layout(&self) -> Layout {
        self.layout
    }
    pub fn format(&self) -> Format {
        self.format
    }
    pub fn mip_count(&self) -> usize {
        self.mip_count
    }
    /// Required logical file length through the last mip, including the header
    /// and any gaps between declared mip ranges. Trailing padding is excluded.
    pub fn encoded_len(&self) -> usize {
        self.mips[self.mip_count - 1].end
    }
    /// Encoded byte range relative to the start of the logical TEX file.
    /// Packed-volume ranges are derived from dimensions rather than the repeated
    /// offset words. The range includes every layer, face and depth slice.
    pub fn mip_range(&self, index: usize) -> Result<Range<usize>> {
        let mip = self.mip_layout(index)?;
        Ok(mip.start..mip.end)
    }
    /// Bind exactly one complete mip, excluding header bytes and inter-mip gaps.
    /// The returned view borrows only `encoded_mip_bytes`, not this header.
    /// Both short and oversized payloads return [`Error::MipSizeMismatch`].
    pub fn mip<'data>(&self, index: usize, encoded_mip_bytes: &'data [u8]) -> Result<Mip<'data>> {
        let mip = self.mip_layout(index)?;
        let expected = mip.end - mip.start;
        if encoded_mip_bytes.len() != expected {
            return Err(Error::MipSizeMismatch {
                index,
                expected,
                actual: encoded_mip_bytes.len(),
            });
        }
        Ok(self.bind_mip(index, mip, encoded_mip_bytes))
    }

    fn mip_layout(&self, index: usize) -> Result<&MipLayout> {
        self.mips[..self.mip_count]
            .get(index)
            .ok_or(Error::MipOutOfBounds {
                index,
                count: self.mip_count,
            })
    }

    fn bind_mip<'data>(&self, index: usize, mip: &MipLayout, data: &'data [u8]) -> Mip<'data> {
        Mip {
            width: mip.width,
            height: mip.height,
            depth: mip.depth,
            layers: self.layers,
            faces: self.faces,
            plane_size: mip.plane_size,
            format: self.format,
            source_index: index,
            source_mip_count: self.mip_count,
            data,
        }
    }
}

impl<'data> Tex<'data> {
    pub fn parse(bytes: &'data [u8]) -> Result<Self> {
        Self::parse_impl(bytes, false)
    }

    /// Explicit Win32 volume profile with every active offset word equal to 80.
    /// Mips occupy consecutive, dimension-derived spans after the header. This
    /// does not rewrite the header, synthesize pixels or fill truncated payloads.
    /// Other offset patterns and non-volume layouts are rejected. [`Self::parse`]
    /// continues to require nonoverlapping declared offsets.
    pub fn parse_packed_volume(bytes: &'data [u8]) -> Result<Self> {
        Self::parse_impl(bytes, true)
    }

    fn parse_impl(bytes: &'data [u8], packed_volume: bool) -> Result<Self> {
        let header = Header::parse_impl(bytes, packed_volume)?;
        // Header validation orders every range and checks its arithmetic, so
        // covering the last end guarantees that every declared mip is present.
        let data = bytes
            .get(..header.encoded_len())
            .ok_or(Error::InvalidData("truncated mip payload"))?;
        Ok(Self { header, data })
    }

    pub fn attributes(&self) -> u32 {
        self.header.raw.attributes
    }
    pub fn layout(&self) -> Layout {
        self.header.layout
    }
    /// Preserves the header byte. Cube samples use either 0 (legacy) or 6;
    /// use mip layer/face counts rather than treating this as a cube-array size.
    pub fn raw_array_size(&self) -> u8 {
        self.header.raw.array_size
    }
    pub fn lod_indices(&self) -> [u32; 3] {
        self.header.raw._lod_indices
    }
    pub fn depth(&self) -> u16 {
        self.header.raw.depth
    }

    /// Preserve the unknown high flag separately from the low seven count bits.
    pub fn mip_unknown_flag(&self) -> bool {
        self.header.raw.mip_flags & 0x80 != 0
    }

    pub fn format(&self) -> Format {
        self.header.format
    }

    pub fn width(&self) -> u16 {
        self.header.raw.width
    }

    pub fn height(&self) -> u16 {
        self.header.raw.height
    }

    pub fn mip_count(&self) -> usize {
        self.header.mip_count
    }
    /// Active offset words exactly as stored, including repeated 80 values in
    /// the explicit packed-volume profile.
    pub fn declared_mip_offsets(&self) -> &[u32] {
        &self.header.raw.offsets[..self.mip_count()]
    }
    pub fn uses_packed_volume_layout(&self) -> bool {
        self.header.packed_volume
    }

    pub fn mip(&self, index: usize) -> Result<Mip<'data>> {
        let mip = self.header.mip_layout(index)?;
        Ok(self
            .header
            .bind_mip(index, mip, &self.data[mip.start..mip.end]))
    }

    pub fn mips(&self) -> impl ExactSizeIterator<Item = Mip<'data>> + '_ {
        self.header.mips[..self.mip_count()]
            .iter()
            .enumerate()
            .map(|(index, mip)| {
                self.header
                    .bind_mip(index, mip, &self.data[mip.start..mip.end])
            })
    }
}

/// One mip containing every layer, cube face or volume slice. The encoded data
/// is mip-major, with contiguous two-dimensional images in file order.
#[derive(Clone, Copy, Debug)]
pub struct Mip<'data> {
    width: u16,
    height: u16,
    depth: u16,
    layers: usize,
    faces: usize,
    plane_size: usize,
    format: Format,
    source_index: usize,
    source_mip_count: usize,
    data: &'data [u8],
}
impl<'data> Mip<'data> {
    /// Mip index in the original TEX header, with zero denoting the base level.
    pub fn source_index(self) -> usize {
        self.source_index
    }
    /// Total mip count declared by the original TEX header.
    pub fn source_mip_count(self) -> usize {
        self.source_mip_count
    }
    pub fn width(self) -> u16 {
        self.width
    }
    pub fn height(self) -> u16 {
        self.height
    }
    pub fn depth(self) -> u16 {
        self.depth
    }
    pub fn layer_count(self) -> usize {
        self.layers
    }
    pub fn face_count(self) -> usize {
        self.faces
    }
    pub fn image_count(self) -> usize {
        self.layers * self.faces * usize::from(self.depth)
    }
    pub fn format(self) -> Format {
        self.format
    }
    /// Encoded bytes of all images of this mip, excluding gaps between mips.
    pub fn data(self) -> &'data [u8] {
        self.data
    }
    pub fn images(self) -> impl ExactSizeIterator<Item = Surface<'data>> {
        self.data
            .chunks_exact(self.plane_size)
            .map(move |data| Surface {
                width: self.width,
                height: self.height,
                format: self.format,
                data,
            })
    }
    pub fn image_by_index(self, index: usize) -> Result<Surface<'data>> {
        if index >= self.image_count() {
            return Err(Error::ImageOutOfBounds {
                index,
                count: self.image_count(),
            });
        }
        let start = index * self.plane_size;
        Ok(Surface {
            width: self.width,
            height: self.height,
            format: self.format,
            data: &self.data[start..start + self.plane_size],
        })
    }
    pub fn image(self, layer: usize, face: usize, z: usize) -> Result<Surface<'data>> {
        if layer >= self.layers || face >= self.faces || z >= usize::from(self.depth) {
            return Err(Error::SurfaceOutOfBounds { layer, face, z });
        }
        self.image_by_index((layer * self.faces + face) * usize::from(self.depth) + z)
    }
    /// Borrow a named face of a cube mip. Other layouts return
    /// [`Error::NotCubeTexture`], including a six-layer 2D array. The image stays
    /// in file row order; no coordinate-system or sampling conversion is made.
    pub fn cube_face(self, face: CubeFace) -> Result<Surface<'data>> {
        if self.faces != 6 {
            return Err(Error::NotCubeTexture);
        }
        self.image(0, face.index(), 0)
    }
    pub fn decode_rgba8(self, max_output_bytes: usize) -> Result<Vec<u8>> {
        let size = decoded_size(
            self.width,
            self.height,
            self.image_count(),
            4,
            max_output_bytes,
        )?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(size)
            .map_err(|_| Error::AllocationFailed)?;
        output.resize(size, 0);
        let image_size = size / self.image_count();
        for (surface, target) in self.images().zip(output.chunks_exact_mut(image_size)) {
            surface.decode_into(target)?;
        }
        Ok(output)
    }
    /// Four f32 channels per pixel, flattened across every image. Float formats
    /// only; one channel repeats across RGB, two channels use B=0, and A=1.
    /// Packed R11G11B10F and signed BC6H expand RGB with A=1. NaN classification
    /// is retained for half/packed formats; NaN payload and sign are unspecified.
    /// The mandatory budget is in bytes, not scalar count.
    pub fn decode_rgba32f(self, max_output_bytes: usize) -> Result<Vec<f32>> {
        if self.format == Format::Bc6hSf16 {
            let size = decoded_size(
                self.width,
                self.height,
                self.image_count(),
                16,
                max_output_bytes,
            )?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(size / 4)
                .map_err(|_| Error::AllocationFailed)?;
            output.resize(size / 4, 0.);
            let image_size = size / 4 / self.image_count();
            for (surface, target) in self.images().zip(output.chunks_exact_mut(image_size)) {
                bc6h::decode_float(surface, target);
            }
            return Ok(output);
        }
        let packed = self.format == Format::R11G11B10F;
        let (channels, half) = if packed {
            (3, false)
        } else {
            self.format
                .float_layout()
                .ok_or(Error::FloatOutputUnavailable(self.format as u32))?
        };
        let size = decoded_size(
            self.width,
            self.height,
            self.image_count(),
            16,
            max_output_bytes,
        )?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(size / 4)
            .map_err(|_| Error::AllocationFailed)?;
        let stride = if packed {
            4
        } else {
            channels * if half { 2 } else { 4 }
        };
        for source in self.data.chunks_exact(stride) {
            output.extend_from_slice(&if packed {
                packed_float_pixel(source)
            } else {
                float_pixel(source, channels, half)
            });
        }
        Ok(output)
    }
}

fn decoded_size(
    width: u16,
    height: u16,
    images: usize,
    stride: usize,
    limit: usize,
) -> Result<usize> {
    let size = usize::from(width)
        .checked_mul(usize::from(height))
        .and_then(|n| n.checked_mul(images))
        .and_then(|n| n.checked_mul(stride))
        .ok_or(Error::InvalidData("decoded image size overflow"))?;
    if size > limit {
        return Err(Error::OutputTooLarge {
            requested: size,
            limit,
        });
    }
    Ok(size)
}

/// One checked two-dimensional image, independent of array/cube/volume storage.
#[derive(Clone, Copy, Debug)]
pub struct Surface<'data> {
    width: u16,
    height: u16,
    format: Format,
    data: &'data [u8],
}

impl<'data> Surface<'data> {
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
        self.decode_into(&mut output)?;
        Ok(output)
    }
    pub fn decode_rgba32f(self, max_output_bytes: usize) -> Result<Vec<f32>> {
        Mip {
            width: self.width,
            height: self.height,
            depth: 1,
            layers: 1,
            faces: 1,
            plane_size: self.data.len(),
            format: self.format,
            source_index: 0,
            source_mip_count: 1,
            data: self.data,
        }
        .decode_rgba32f(max_output_bytes)
    }
    fn decode_into(self, output: &mut [u8]) -> Result<()> {
        match self.format {
            Format::Bc1 | Format::Bc2 | Format::Bc3 | Format::Bc4 | Format::Bc5 => {
                bc::decode(self, output)
            }
            Format::Bc7 => bc7::decode(self, output).map_err(Error::InvalidData)?,
            Format::Bc6hSf16 => bc6h::decode_bytes(self, output),
            Format::R16F
            | Format::R11G11B10F
            | Format::R32F
            | Format::R16G16F
            | Format::R32G32F
            | Format::R16G16B16A16F
            | Format::R32G32B32A32F => decode_float(self, output),
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
        Ok(())
    }
}

fn expand5(value: u16) -> u8 {
    ((value << 3) | (value >> 2)) as u8
}

/// Converts float channels to tightly packed RGBA8: clamp to `0..=1`, multiply
/// by 255 and round half up. One channel replicates to RGB; two channels fill
/// RGB with the third channel at zero; four channels are used as stored.
fn decode_float(mip: Surface<'_>, output: &mut [u8]) {
    if mip.format == Format::R11G11B10F {
        for (source, target) in mip.data.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
            target.copy_from_slice(&packed_float_pixel(source).map(quantize));
        }
        return;
    }
    let (channels, floating) = match mip.format {
        Format::R16F => (1, true),
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

// Unsigned 5-bit-exponent mini-floats with bias 15, expanded exactly into
// binary16's 10-bit mantissa before the shared numeric conversion. NaN payload
// bits are not promised by the float output API, as for other half formats.
fn packed_float_pixel(source: &[u8]) -> [f32; 4] {
    let word = u32::from_le_bytes(source.try_into().unwrap());
    [
        half_to_f32(((word & 0x7ff) << 4) as u16),
        half_to_f32((((word >> 11) & 0x7ff) << 4) as u16),
        half_to_f32(((word >> 22) << 5) as u16),
        1.,
    ]
}

fn float_pixel(source: &[u8], channels: usize, half: bool) -> [f32; 4] {
    let mut values = [0.; 4];
    for (channel, target) in values.iter_mut().enumerate().take(channels) {
        *target = if half {
            let offset = channel * 2;
            half_to_f32(u16::from_le_bytes([source[offset], source[offset + 1]]))
        } else {
            let offset = channel * 4;
            f32::from_le_bytes([
                source[offset],
                source[offset + 1],
                source[offset + 2],
                source[offset + 3],
            ])
        };
    }
    match channels {
        1 => [values[0], values[0], values[0], 1.],
        2 => [values[0], values[1], 0., 1.],
        _ => values,
    }
}

fn quantize(value: f32) -> u8 {
    if value.is_nan() {
        0
    } else {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

pub(crate) fn half_to_f32(half: u16) -> f32 {
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

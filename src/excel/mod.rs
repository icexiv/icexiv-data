//! Binary parsers for Excel headers (EXH) and pages (EXD).
//!
//! [`Exh`] describes column layouts, pagination, languages and the row variant.
//! [`Exd`] borrows page bytes and its header, and exposes typed fields
//! by numeric column index. Column names and game-specific semantics need an
//! external schema; strings remain raw bytes (potentially SeString payloads).
//! Parsing is synchronous and operates only on byte slices, with no I/O.
//!
//! # Parsing header and page bytes
//!
//! ```no_run
//! use icexiv_data::excel::{Exd, Exh};
//!
//! fn inspect_page(exh_bytes: &[u8], exd_bytes: &[u8]) -> Result<(), icexiv_data::excel::Error> {
//!     let header = Exh::parse(exh_bytes)?;
//!     let page = Exd::parse(exd_bytes, &header)?;
//!     for row in page.rows() {
//!         println!("row {} / {:?}", row.id(), row.subrow_id());
//!         for field in row.fields() {
//!             println!("{:?}", field?);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! Supports EXH version 3 and EXD version 2, ordinary rows and subrows, scalar
//! fields and packed booleans. Structural lengths/counts/offsets are validated
//! before allocation/access. String references are checked when read, within the
//! containing parent row, and returned without their terminating NUL. They are
//! not automatically decoded as UTF-8 or interpreted as SeString expressions.
//! Metadata allocation is bounded by validated input-table lengths, not a
//! separate policy limit; bound input sizes in the caller when data is untrusted.

pub mod exd;
pub mod exh;

pub use exd::{Exd, Row, Value};
pub use exh::{Column, ColumnType, Exh, Language, Page, Variant};

use binrw::{BinRead, Endian};
use std::fmt;
use std::io::Cursor;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion { format: &'static str, version: u16 },
    UnsupportedVariant(u8),
    UnsupportedColumnType(u16),
    UnsupportedLanguage(u16),
    LanguageUnavailable(Language),
    InvalidSheetName,
    ColumnOutOfBounds { index: usize, count: usize },
    VariantMismatch,
    AllocationFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(reason) => write!(f, "invalid Excel data: {reason}"),
            Self::UnsupportedVersion { format, version } => {
                write!(f, "unsupported {format} version {version}")
            }
            Self::UnsupportedVariant(value) => write!(f, "unsupported Excel variant {value}"),
            Self::UnsupportedColumnType(value) => {
                write!(f, "unsupported Excel column type {value:#x}")
            }
            Self::UnsupportedLanguage(value) => write!(f, "unsupported Excel language {value}"),
            Self::LanguageUnavailable(language) => {
                write!(f, "language {language:?} not available in sheet")
            }
            Self::InvalidSheetName => f.write_str("invalid relative sheet name"),
            Self::ColumnOutOfBounds { index, count } => {
                write!(f, "column {index} exceeds column count {count}")
            }
            Self::VariantMismatch => f.write_str("row lookup used with the wrong sheet variant"),
            Self::AllocationFailed => f.write_str("Excel metadata allocation failed"),
        }
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

fn read<T>(bytes: &[u8], endian: Endian) -> Result<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    T::read_options(&mut Cursor::new(bytes), endian, ())
        .map_err(|_| Error::InvalidData("bad magic or truncated binary structure"))
}

fn slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or(Error::InvalidData("range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidData("range exceeds input"))
}

fn table_end(bytes: &[u8], offset: usize, count: usize, stride: usize) -> Result<usize> {
    let length = count
        .checked_mul(stride)
        .ok_or(Error::InvalidData("table size overflow"))?;
    slice(bytes, offset, length)?;
    offset
        .checked_add(length)
        .ok_or(Error::InvalidData("table end overflow"))
}

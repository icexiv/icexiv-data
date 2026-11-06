//! Win32 SKLB 1200/1300 containers and embedded Havok binary tagfile v3.
//!
//! The 46/48-byte header, auxiliary section and unassigned bytes are retained;
//! the embedded graph is fully parsed with explicit input/allocation/work limits.
//! Other container versions return errors instead of guessing offset layouts.
use crate::{
    binary::{u16_at, u32_at},
    havok,
};
use std::fmt;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion([u8; 4]),
    Havok(havok::Error),
}
impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::InvalidData(s)
    }
}
impl From<havok::Error> for Error {
    fn from(e: havok::Error) -> Self {
        Self::Havok(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(s) => write!(f, "invalid SKLB: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported SKLB version bytes {v:02x?}"),
            Self::Havok(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
#[derive(Debug)]
pub struct Sklb<'a> {
    pub version: Version,
    pub raw_header: &'a [u8],
    pub auxiliary: &'a [u8],
    pub tagfile_offset: usize,
    pub tagfile: havok::Tagfile<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Version {
    V1200,
    V1300,
}
impl<'a> Sklb<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        Self::parse_with_limits(bytes, havok::Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: havok::Limits) -> Result<Self, Error> {
        if bytes.len() > limits.max_input_bytes {
            return Err(havok::Error::BudgetExceeded("SKLB input").into());
        }
        let prefix = bytes.get(..8).ok_or("truncated SKLB signature")?;
        if &prefix[..4] != b"blks" {
            return Err(Error::InvalidData("magic"));
        }
        let version: [u8; 4] = prefix[4..8]
            .try_into()
            .map_err(|_| Error::InvalidData("version"))?;
        let (version, size, auxiliary_offset, tagfile_offset) = match &version {
            b"0031" => (
                Version::V1300,
                48,
                usize::try_from(u32_at(bytes, 8)?)
                    .map_err(|_| Error::InvalidData("auxiliary offset overflow"))?,
                usize::try_from(u32_at(bytes, 12)?)
                    .map_err(|_| Error::InvalidData("tagfile offset overflow"))?,
            ),
            b"0021" => (
                Version::V1200,
                46,
                usize::from(u16_at(bytes, 8)?),
                usize::from(u16_at(bytes, 10)?),
            ),
            _ => return Err(Error::UnsupportedVersion(version)),
        };
        let raw_header = bytes.get(..size).ok_or("truncated SKLB header")?;
        if auxiliary_offset < size || tagfile_offset < auxiliary_offset {
            return Err(Error::InvalidData("section offsets"));
        }
        let auxiliary = bytes
            .get(auxiliary_offset..tagfile_offset)
            .ok_or("auxiliary range")?;
        let tagfile = havok::Tagfile::parse_with_limits(
            bytes.get(tagfile_offset..).ok_or("tagfile offset")?,
            limits,
        )?;
        Ok(Self {
            version,
            raw_header,
            auxiliary,
            tagfile_offset,
            tagfile,
        })
    }
    pub fn skeletons(&self, max_output_bytes: usize) -> havok::Result<Vec<havok::Skeleton<'a>>> {
        self.tagfile.skeletons(max_output_bytes)
    }
}

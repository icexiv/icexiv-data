//! Borrowed reader for version 1, variant 1 AMB direction coefficients.
//! See `docs/amb/README.md` and `docs/provenance/volume-environment.md`.
//! Selection, phase interpolation, color transforms and SH evaluation are
//! separate from parsing. Values are returned in on-disk channel/slot order.

use std::{collections::BTreeSet, fmt};

const HEADER: usize = 16;
const ENTRY: usize = 8;
const RECORD: usize = 108;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedHeader,
    InvalidLength,
    InvalidDirectory,
    NonFiniteCoefficient,
    InvalidPhase,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedHeader => "unsupported AMB header",
            Self::InvalidLength => "invalid AMB length",
            Self::InvalidDirectory => "invalid AMB group directory",
            Self::NonFiniteCoefficient => "non-finite AMB coefficient",
            Self::InvalidPhase => "AMB phase must be finite and in [0, 1)",
        })
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn u16(self, bytes: &[u8]) -> u16 {
        let bytes = [bytes[0], bytes[1]];
        match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        }
    }

    fn u32(self, bytes: &[u8]) -> u32 {
        let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
        match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        }
    }
}

pub struct Amb<'a> {
    directory: &'a [u8],
    records: &'a [u8],
    endian: Endian,
}

pub struct Group<'a> {
    pub id: u16,
    records: &'a [u8],
    endian: Endian,
}

impl<'a> Amb<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let header = bytes.get(..HEADER).ok_or(Error::InvalidLength)?;
        let endian = match (&header[..4], &header[4..6], header[7]) {
            (b"AMB\0", [1, 0], 1) => Endian::Little,
            (b"AMB\0", [0, 1], 1) => Endian::Big,
            _ => return Err(Error::UnsupportedHeader),
        };
        let groups = usize::from(endian.u16(&header[10..12]));
        let records =
            usize::try_from(endian.u32(&header[12..])).map_err(|_| Error::InvalidLength)?;
        let start = HEADER + groups * ENTRY;
        let end = records
            .checked_mul(RECORD)
            .and_then(|n| start.checked_add(n));
        if end != Some(bytes.len()) {
            return Err(Error::InvalidLength);
        }
        let directory = &bytes[HEADER..start];
        let mut ids = BTreeSet::new();
        for entry in directory.chunks_exact(ENTRY) {
            let id = endian.u16(entry);
            let count = usize::from(endian.u16(&entry[2..]));
            let first =
                usize::try_from(endian.u32(&entry[4..])).map_err(|_| Error::InvalidDirectory)?;
            if first > records || count > records - first || !ids.insert(id) {
                return Err(Error::InvalidDirectory);
            }
        }
        Ok(Self {
            directory,
            records: &bytes[start..],
            endian,
        })
    }

    pub fn group_count(&self) -> usize {
        self.directory.len() / ENTRY
    }

    pub fn record_count(&self) -> usize {
        self.records.len() / RECORD
    }

    /// Iterate in file order. Multiple IDs may reference the same records.
    pub fn groups(&self) -> impl ExactSizeIterator<Item = Group<'a>> + '_ {
        self.directory.chunks_exact(ENTRY).map(|entry| {
            let first = self.endian.u32(&entry[4..]) as usize;
            let count = usize::from(self.endian.u16(&entry[2..]));
            Group {
                id: self.endian.u16(entry),
                records: &self.records[first * RECORD..(first + count) * RECORD],
                endian: self.endian,
            }
        })
    }

    pub fn group(&self, id: u16) -> Option<Group<'a>> {
        self.groups().find(|group| group.id == id)
    }
}

impl Group<'_> {
    pub fn record_count(&self) -> usize {
        self.records.len() / RECORD
    }

    /// Return one record as RGB by nine slots. An absent index is not a decode
    /// error; malformed floating-point values in a present record are.
    pub fn coefficients(&self, index: usize) -> Result<Option<[[f32; 9]; 3]>, Error> {
        if index >= self.record_count() {
            return Ok(None);
        }
        let mut result = [[0.; 9]; 3];
        let bytes = &self.records[index * RECORD..(index + 1) * RECORD];
        for (dst, src) in result.iter_mut().flatten().zip(bytes.chunks_exact(4)) {
            *dst = f32::from_bits(self.endian.u32(src));
            if !dst.is_finite() {
                return Err(Error::NonFiniteCoefficient);
            }
        }
        Ok(Some(result))
    }

    /// Sample a variant 1 group at a normalized day phase. This returns the
    /// interpolated raw coefficients, not radiance or UE irradiance.
    pub fn sample_phase(&self, phase: f32) -> Result<Option<[[f32; 9]; 3]>, Error> {
        if !(0.0..1.0).contains(&phase) {
            return Err(Error::InvalidPhase);
        }
        let count = self.record_count();
        if count <= 1 {
            return self.coefficients(0);
        }
        let cursor = f64::from(phase) * count as f64;
        let lo = cursor as usize;
        let w = (cursor - lo as f64) as f32;
        let mut a = self.coefficients(lo)?.ok_or(Error::InvalidDirectory)?;
        let b = self
            .coefficients((lo + 1) % count)?
            .ok_or(Error::InvalidDirectory)?;
        for (a, b) in a.iter_mut().flatten().zip(b.into_iter().flatten()) {
            *a = (1. - w) * *a + w * b;
        }
        Ok(Some(a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(big: bool) -> Vec<u8> {
        let u16b = |v: u16| {
            if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let u32b = |v: u32| {
            if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let mut bytes = b"AMB\0".to_vec();
        bytes.extend(u16b(1));
        bytes.extend([0, 1]);
        bytes.extend(u16b(0));
        bytes.extend(u16b(2));
        bytes.extend(u32b(1));
        for id in [45, 3] {
            bytes.extend(u16b(id));
            bytes.extend(u16b(1));
            bytes.extend(u32b(0));
        }
        for i in 0..27 {
            bytes.extend(u32b((i as f32 - 5.).to_bits()));
        }
        bytes
    }

    #[test]
    fn parses_both_byte_orders_and_shared_records() {
        for big in [false, true] {
            let data = sample(big);
            let amb = Amb::parse(&data).unwrap();
            assert_eq!((amb.group_count(), amb.record_count()), (2, 1));
            let a = amb.group(45).unwrap();
            let b = amb.group(3).unwrap();
            let values = a.coefficients(0).unwrap().unwrap();
            assert_eq!((values[0][0], values[2][8]), (-5., 21.));
            assert_eq!(b.coefficients(0), Ok(Some(values)));
            assert_eq!(a.coefficients(1), Ok(None));
            assert!(amb.group(59).is_none());
        }
    }

    #[test]
    fn rejects_malformed_lengths_ranges_and_values() {
        let mut data = sample(false);
        assert!(matches!(
            Amb::parse(&data[..data.len() - 1]),
            Err(Error::InvalidLength)
        ));
        data[20] = 1;
        assert!(matches!(Amb::parse(&data), Err(Error::InvalidDirectory)));
        data[20] = 0;
        data[32..36].copy_from_slice(&f32::NAN.to_le_bytes());
        let amb = Amb::parse(&data).unwrap();
        assert_eq!(
            amb.group(45).unwrap().coefficients(0),
            Err(Error::NonFiniteCoefficient)
        );
    }

    #[test]
    fn variant_one_phase_wraps_at_midnight() {
        let mut bytes = sample(false);
        bytes[12..16].copy_from_slice(&2_u32.to_le_bytes());
        bytes[18..20].copy_from_slice(&2_u16.to_le_bytes());
        bytes.extend((0..27).flat_map(|_| 3_f32.to_le_bytes()));
        let amb = Amb::parse(&bytes).unwrap();
        let group = amb.group(45).unwrap();
        assert_eq!(group.sample_phase(0.).unwrap().unwrap()[0][0], -5.);
        assert_eq!(group.sample_phase(0.5).unwrap().unwrap()[0][0], 3.);
        assert_eq!(group.sample_phase(0.75).unwrap().unwrap()[0][0], -1.);
        assert_eq!(group.sample_phase(f32::NAN), Err(Error::InvalidPhase));
    }
}

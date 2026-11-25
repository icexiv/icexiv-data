//! Environment references in single-SCN1 little-endian LVB1 layouts.
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    UnsupportedProfile,
    InvalidOffset,
    InvalidString,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LVB: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvironmentRef<'a> {
    pub index: u32,
    pub location_instance_id: u32,
    pub envb: &'a str,
    pub essb: &'a str,
}

fn bytes_at(bytes: &[u8], off: usize, len: usize) -> Result<&[u8], Error> {
    bytes
        .get(off..off.checked_add(len).ok_or(Error::InvalidOffset)?)
        .ok_or(Error::InvalidOffset)
}
fn word(bytes: &[u8], off: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        bytes_at(bytes, off, 4)?.try_into().unwrap(),
    ))
}
fn relative(bytes: &[u8], base: usize, field: usize) -> Result<usize, Error> {
    base.checked_add_signed(word(bytes, field)? as i32 as isize)
        .ok_or(Error::InvalidOffset)
}
fn string(bytes: &[u8], base: usize, field: usize) -> Result<&str, Error> {
    if word(bytes, field)? == 0 {
        return Ok("");
    }
    let at = relative(bytes, base, field)?;
    let remaining = bytes.get(at..).ok_or(Error::InvalidOffset)?;
    let end = remaining
        .iter()
        .take(1024)
        .position(|&c| c == 0)
        .ok_or(Error::InvalidString)?;
    std::str::from_utf8(&remaining[..end]).map_err(|_| Error::InvalidString)
}

/// Resolve layout pointers without enumerating strings or selecting an active environment.
pub fn environment_refs(
    bytes: &[u8],
) -> Result<impl ExactSizeIterator<Item = Result<EnvironmentRef<'_>, Error>>, Error> {
    if !bytes.starts_with(b"LVB1")
        || word(bytes, 4)? as usize != bytes.len()
        || word(bytes, 8)? != 1
        || bytes_at(bytes, 12, 4)? != b"SCN1"
    {
        return Err(Error::UnsupportedProfile);
    }
    let general = relative(bytes, 20, 20)?;
    bytes_at(bytes, general, 16)?;
    let count = word(bytes, general + 12)? as usize;
    let list = relative(bytes, general, general + 8)?;
    bytes_at(
        bytes,
        list,
        count.checked_mul(16).ok_or(Error::InvalidOffset)?,
    )?;
    Ok((0..count).map(move |i| {
        let at = list + i * 16;
        Ok(EnvironmentRef {
            envb: string(bytes, at, at)?,
            index: word(bytes, at + 4)?,
            location_instance_id: word(bytes, at + 8)?,
            essb: string(bytes, at, at + 12)?,
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_explicit_layout_pointer_and_rejects_bad_pointer() {
        let mut bytes = [0_u8; 80];
        bytes[..4].copy_from_slice(b"LVB1");
        bytes[12..16].copy_from_slice(b"SCN1");
        for (at, v) in [
            (4, 80_u32),
            (8, 1),
            (20, 16),
            (44, 16),
            (48, 1),
            (52, 16),
            (56, 2),
            (60, 123),
        ] {
            bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[68..78].copy_from_slice(b"a/b_c.envb");
        let found = environment_refs(&bytes).unwrap().next().unwrap().unwrap();
        assert_eq!(
            found,
            EnvironmentRef {
                index: 2,
                location_instance_id: 123,
                envb: "a/b_c.envb",
                essb: ""
            }
        );
        bytes[52..56].copy_from_slice(&1000_u32.to_le_bytes());
        assert_eq!(
            environment_refs(&bytes).unwrap().next().unwrap(),
            Err(Error::InvalidOffset)
        );
    }
}

//! Minimal checked FlatBuffers wire access for EPBD; no generated code/runtime.
use super::{Error, Result};
use crate::binary::{slice, u16_at, u32_at};

#[derive(Clone, Copy, Debug)]
pub(super) struct Table<'a> {
    pub data: &'a [u8],
    pos: usize,
    vtable: &'a [u8],
    size: usize,
}
impl<'a> Table<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> Result<Self> {
        let back = u32_at(data, pos)? as i32;
        let vpos = i64::try_from(pos)
            .map_err(|_| Error::InvalidData("FlatBuffers table offset"))?
            .checked_sub(i64::from(back))
            .ok_or("FlatBuffers vtable arithmetic overflow")?;
        let vpos =
            usize::try_from(vpos).map_err(|_| Error::InvalidData("FlatBuffers vtable offset"))?;
        let length = usize::from(u16_at(data, vpos)?);
        let size = usize::from(u16_at(data, vpos.checked_add(2).ok_or("vtable overflow")?)?);
        if length < 4 || length % 2 != 0 || size < 4 {
            return Err(Error::InvalidData("FlatBuffers table size"));
        }
        let vtable = slice(data, vpos, length)?;
        slice(data, pos, size)?;
        Ok(Self {
            data,
            pos,
            vtable,
            size,
        })
    }
    pub fn field(self, index: usize, width: usize) -> Result<Option<usize>> {
        let at = index
            .checked_mul(2)
            .and_then(|n| n.checked_add(4))
            .ok_or("field overflow")?;
        if at >= self.vtable.len() {
            return Ok(None);
        }
        let offset = usize::from(u16_at(self.vtable, at)?);
        if offset == 0 {
            return Ok(None);
        }
        if offset < 4 || offset.checked_add(width).is_none_or(|n| n > self.size) {
            return Err(Error::InvalidData("FlatBuffers field outside table"));
        }
        Ok(Some(self.pos.checked_add(offset).ok_or("field overflow")?))
    }
    pub fn uint(self, index: usize) -> Result<u32> {
        self.field(index, 4)?
            .map_or(Ok(0), |p| Ok(u32_at(self.data, p)?))
    }
    pub fn optional_ushort(self, index: usize) -> Result<Option<u16>> {
        Ok(self
            .field(index, 2)?
            .map(|p| u16_at(self.data, p))
            .transpose()?)
    }
    pub fn indirect(self, index: usize) -> Result<Option<usize>> {
        self.field(index, 4)?
            .map(|p| indirect(self.data, p))
            .transpose()
    }
    pub fn vector(self, index: usize, stride: usize) -> Result<&'a [u8]> {
        let Some(p) = self.indirect(index)? else {
            return Ok(&[]);
        };
        let count = usize::try_from(u32_at(self.data, p)?)
            .map_err(|_| Error::InvalidData("vector count overflow"))?;
        Ok(slice(
            self.data,
            p.checked_add(4).ok_or("vector overflow")?,
            count.checked_mul(stride).ok_or("vector size overflow")?,
        )?)
    }
}
pub(super) fn indirect(data: &[u8], p: usize) -> Result<usize> {
    let delta =
        usize::try_from(u32_at(data, p)?).map_err(|_| Error::InvalidData("offset overflow"))?;
    if delta < 4 {
        return Err(Error::InvalidData("FlatBuffers invalid forward offset"));
    }
    let at = p.checked_add(delta).ok_or("offset overflow")?;
    slice(data, at, 4)?;
    Ok(at)
}

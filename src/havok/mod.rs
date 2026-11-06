//! Checked Havok binary tagfile v3 metadata/object graph and skeleton decoding.
//! No Havok runtime, filesystem, SDK, external parser or target execution.
mod skeleton;
mod spline;
pub use spline::{Sample, SplineAnimation, SplineBlock, SplineLimits};
mod tagfile;
pub use skeleton::{Bone, Skeleton, SkeletonLimits, Transform};
use std::fmt;
pub use tagfile::{Class, Field, FloatData, Limits, Member, Object, Root, Tagfile, Value};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedVersion(i64),
    UnsupportedType(u32),
    BudgetExceeded(&'static str),
    AllocationFailed,
    MissingField(&'static str),
}
impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::InvalidData(s)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(s) => write!(f, "invalid Havok data: {s}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported Havok version {v}"),
            Self::UnsupportedType(t) => write!(f, "unsupported Havok member type {t:#x}"),
            Self::BudgetExceeded(s) => write!(f, "Havok {s} budget exceeded"),
            Self::AllocationFailed => f.write_str("Havok allocation failed"),
            Self::MissingField(s) => write!(f, "Havok required field missing: {s}"),
        }
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub(super) fn spend(value: &mut usize, n: usize, name: &'static str) -> Result<()> {
    *value = value.checked_sub(n).ok_or(Error::BudgetExceeded(name))?;
    Ok(())
}
pub(super) fn reserve<T>(n: usize, bytes: &mut usize) -> Result<Vec<T>> {
    spend(
        bytes,
        n.checked_mul(std::mem::size_of::<T>())
            .ok_or("allocation size overflow")?,
        "allocation",
    )?;
    let mut v = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| Error::AllocationFailed)?;
    Ok(v)
}

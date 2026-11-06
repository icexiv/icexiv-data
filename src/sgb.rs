//! Win32 SGB1/SCN1 furniture scene structure and explicit asset pointers.
//!
//! The verified profile has one scene and one layer group. Layers/instances are
//! resolved by their relative offset tables; strings are never scanned to guess
//! dependencies. BG model/collision and SharedGroup paths retain occurrences and
//! owner transforms. Other instance kinds retain their common header/type.
//! No filesystem access, dependency loading, transform composition or behavior.
use crate::binary::{slice, u32_at};
use std::fmt;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    UnsupportedProfile(&'static str),
    BudgetExceeded(&'static str),
    AllocationFailed,
}
impl From<&'static str> for Error {
    fn from(v: &'static str) -> Self {
        Self::InvalidData(v)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(v) => write!(f, "invalid SGB: {v}"),
            Self::UnsupportedProfile(v) => write!(f, "unsupported SGB profile: {v}"),
            Self::BudgetExceeded(v) => write!(f, "SGB {v} budget exceeded"),
            Self::AllocationFailed => f.write_str("SGB allocation failed"),
        }
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_allocation_bytes: usize,
    pub max_layers: usize,
    pub max_instances: usize,
    pub max_string_bytes: usize,
    pub max_work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 << 20,
            max_allocation_bytes: 64 << 20,
            max_layers: 65536,
            max_instances: 1_000_000,
            max_string_bytes: 1 << 20,
            max_work: 16_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Asset<'a> {
    Background {
        model_path: &'a [u8],
        collision_path: &'a [u8],
    },
    SharedGroup {
        path: &'a [u8],
    },
    Other(u32),
}
#[derive(Clone, Copy, Debug)]
pub struct Instance<'a> {
    pub offset: usize,
    pub id: u32,
    pub name: &'a [u8],
    pub transform: Transform,
    pub asset: Asset<'a>,
    pub raw_header: &'a [u8],
}
#[derive(Debug)]
pub struct Layer<'a> {
    pub offset: usize,
    pub id: u32,
    pub name: &'a [u8],
    pub raw_header: &'a [u8],
    pub instances: Vec<Instance<'a>>,
}
#[derive(Debug)]
pub struct Group<'a> {
    pub offset: usize,
    pub id: u32,
    pub name: &'a [u8],
    pub raw_header: &'a [u8],
    pub layers: Vec<Layer<'a>>,
}
#[derive(Debug)]
pub struct Sgb<'a> {
    bytes: &'a [u8],
    pub scene_header: &'a [u8],
    pub group: Group<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyKind {
    Model,
    Collision,
    SharedGroup,
}
#[derive(Clone, Copy, Debug)]
pub struct Dependency<'a> {
    pub kind: DependencyKind,
    pub path: &'a [u8],
    pub layer_index: usize,
    pub instance_index: usize,
    pub instance_id: u32,
    pub instance_offset: usize,
}
struct Reader<'a> {
    bytes: &'a [u8],
    allocation: usize,
    work: usize,
    instances: usize,
    limits: Limits,
}
fn spend(b: &mut usize, n: usize, name: &'static str) -> Result<()> {
    *b = b.checked_sub(n).ok_or(Error::BudgetExceeded(name))?;
    Ok(())
}
impl<'a> Reader<'a> {
    fn word(&mut self, at: usize) -> Result<u32> {
        spend(&mut self.work, 1, "work")?;
        Ok(u32_at(self.bytes, at)?)
    }
    fn target(&mut self, origin: usize, field: usize) -> Result<usize> {
        let v = self.word(field)? as i32;
        let offset = if v < 0 {
            origin.checked_sub(v.unsigned_abs() as usize)
        } else {
            origin.checked_add(v as usize)
        }
        .ok_or("relative offset overflow")?;
        if offset > self.bytes.len() {
            return Err("relative offset beyond input".into());
        }
        Ok(offset)
    }
    fn table(&mut self, origin: usize, field: usize, count: usize) -> Result<usize> {
        let p = self.target(origin, field)?;
        slice(
            self.bytes,
            p,
            count.checked_mul(4).ok_or("offset table size overflow")?,
        )?;
        spend(&mut self.work, count, "offset table")?;
        Ok(p)
    }
    fn string(&mut self, origin: usize, field: usize) -> Result<&'a [u8]> {
        let relative = usize::try_from(self.word(field)?).map_err(|_| "string offset overflow")?;
        let p = origin
            .checked_add(relative)
            .ok_or("string pointer overflow")?;
        let remaining = self.bytes.get(p..).ok_or("string pointer")?;
        let cap = remaining
            .len()
            .min(self.limits.max_string_bytes.saturating_add(1));
        for (i, b) in remaining[..cap].iter().enumerate() {
            spend(&mut self.work, 1, "string work")?;
            if *b == 0 {
                return Ok(&remaining[..i]);
            }
        }
        if remaining.len() > self.limits.max_string_bytes {
            Err(Error::BudgetExceeded("string length"))
        } else {
            Err("unterminated string".into())
        }
    }
    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>> {
        spend(
            &mut self.allocation,
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or("allocation size overflow")?,
            "allocation",
        )?;
        let mut v = Vec::new();
        v.try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        Ok(v)
    }
    fn instance(&mut self, p: usize) -> Result<Instance<'a>> {
        let header = slice(self.bytes, p, 48)?;
        let typ = self.word(p)?;
        let id = self.word(p + 4)?;
        let name = self.string(p, p + 8)?;
        let mut lanes = [0f32; 9];
        for (i, v) in lanes.iter_mut().enumerate() {
            *v = f32::from_bits(self.word(p + 12 + i * 4)?);
            if !v.is_finite() {
                return Err("nonfinite instance transform".into());
            }
        }
        let asset = match typ {
            1 => {
                slice(self.bytes, p, 56)?;
                Asset::Background {
                    model_path: self.string(p, p + 48)?,
                    collision_path: self.string(p, p + 52)?,
                }
            }
            6 => {
                slice(self.bytes, p, 52)?;
                Asset::SharedGroup {
                    path: self.string(p, p + 48)?,
                }
            }
            other => Asset::Other(other),
        };
        Ok(Instance {
            offset: p,
            id,
            name,
            raw_header: header,
            transform: Transform {
                translation: [lanes[0], lanes[1], lanes[2]],
                rotation: [lanes[3], lanes[4], lanes[5]],
                scale: [lanes[6], lanes[7], lanes[8]],
            },
            asset,
        })
    }
    fn layer(&mut self, p: usize) -> Result<Layer<'a>> {
        let raw_header = slice(self.bytes, p, 52)?;
        let id = self.word(p)?;
        let name = self.string(p, p + 4)?;
        let count = usize::try_from(self.word(p + 12)?).map_err(|_| "instance count overflow")?;
        spend(&mut self.instances, count, "instances")?;
        let table = self.table(p, p + 8, count)?;
        let mut instances = self.reserve(count)?;
        for i in 0..count {
            let at = self.target(table, table + i * 4)?;
            instances.push(self.instance(at)?);
        }
        Ok(Layer {
            offset: p,
            id,
            name,
            raw_header,
            instances,
        })
    }
}
impl<'a> Sgb<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("input"));
        }
        let header = slice(bytes, 0, 12)?;
        if &header[..4] != b"SGB1" {
            return Err(Error::UnsupportedProfile("file signature"));
        }
        if usize::try_from(u32_at(bytes, 4)?).ok() != Some(bytes.len()) {
            return Err("declared file length".into());
        }
        if u32_at(bytes, 8)? != 1 {
            return Err(Error::UnsupportedProfile("scene chunk count"));
        }
        let scene_header = slice(bytes, 12, 72)?;
        if &scene_header[..4] != b"SCN1" || u32_at(scene_header, 4)? != 72 {
            return Err(Error::UnsupportedProfile("SCN1 fixed header"));
        }
        if u32_at(scene_header, 12)? != 1 {
            return Err(Error::UnsupportedProfile("layer group count"));
        }
        let mut r = Reader {
            bytes,
            allocation: limits.max_allocation_bytes,
            work: limits.max_work,
            instances: limits.max_instances,
            limits,
        };
        // SCN1 group offset is relative to its payload (absolute byte 20),
        // whereas the layer and instance offsets are relative to table starts.
        let group = r.target(20, 20)?;
        let raw_header = slice(bytes, group, 16)?;
        let id = r.word(group)?;
        let name = r.string(group, group + 4)?;
        let count = usize::try_from(r.word(group + 12)?).map_err(|_| "layer count overflow")?;
        if count > limits.max_layers {
            return Err(Error::BudgetExceeded("layers"));
        }
        let table = r.table(group, group + 8, count)?;
        let mut layers = r.reserve(count)?;
        for i in 0..count {
            let p = r.target(table, table + i * 4)?;
            layers.push(r.layer(p)?);
        }
        Ok(Self {
            bytes,
            scene_header,
            group: Group {
                offset: group,
                id,
                name,
                raw_header,
                layers,
            },
        })
    }
    pub fn raw_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Explicit nonempty BG / SharedGroup pointers, in layer/instance order.
    /// No deduplication: repeated paths can have different owner transforms.
    pub fn dependencies(&self) -> impl Iterator<Item = Dependency<'a>> + '_ {
        self.group.layers.iter().enumerate().flat_map(|(li, l)| {
            l.instances.iter().enumerate().flat_map(move |(ii, i)| {
                let paths = match i.asset {
                    Asset::Background {
                        model_path,
                        collision_path,
                    } => [
                        Some((DependencyKind::Model, model_path)),
                        Some((DependencyKind::Collision, collision_path)),
                    ],
                    Asset::SharedGroup { path } => {
                        [Some((DependencyKind::SharedGroup, path)), None]
                    }
                    Asset::Other(_) => [None, None],
                };
                paths
                    .into_iter()
                    .flatten()
                    .filter(|(_, p)| !p.is_empty())
                    .map(move |(kind, path)| Dependency {
                        kind,
                        path,
                        layer_index: li,
                        instance_index: ii,
                        instance_id: i.id,
                        instance_offset: i.offset,
                    })
            })
        })
    }
    /// Common headers whose type-specific payload is not interpreted. Their
    /// presence does not establish absence of every possible external asset.
    pub fn other_instances(&self) -> impl Iterator<Item = &Instance<'a>> {
        self.group
            .layers
            .iter()
            .flat_map(|l| &l.instances)
            .filter(|i| matches!(i.asset, Asset::Other(_)))
    }
}

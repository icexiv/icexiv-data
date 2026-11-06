use super::{Error, Result, Tagfile, Value, reserve, spend};
#[derive(Clone, Copy, Debug)]
pub struct SkeletonLimits {
    pub max_output_bytes: usize,
    pub max_bones: usize,
    pub max_validation_steps: usize,
}
impl Default for SkeletonLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: 64 * 1024 * 1024,
            max_bones: 65536,
            max_validation_steps: 16 * 1024 * 1024,
        }
    }
}
/// Original Havok QsTransform lanes: translation xyzw, rotation xyzw and scale
/// xyzw. Translation/scale fourth lanes remain available, not discarded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: [f32; 4],
    pub rotation: [f32; 4],
    pub scale: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Bone<'a> {
    pub name: &'a [u8],
    pub parent: Option<usize>,
    pub lock_translation: bool,
    pub rest: Transform,
}
#[derive(Debug)]
pub struct Skeleton<'a> {
    pub name: Option<&'a [u8]>,
    pub object_id: usize,
    pub bones: Vec<Bone<'a>>,
}
impl<'a> Tagfile<'a> {
    /// IDs explicitly referenced by remembered hkaAnimationContainer objects,
    /// in container/array order. Mapper-side skeletons are not implicitly
    /// selected. IDs can be matched against `Skeleton::object_id`.
    pub fn animation_skeleton_ids(&self, max_output_bytes: usize) -> Result<Vec<usize>> {
        let mut allocation = max_output_bytes;
        let mut out = Vec::new();
        for obj in &self.objects {
            if self.class(obj.class_index)?.name != b"hkaAnimationContainer" {
                continue;
            }
            let Some(value) = obj.field(b"skeletons") else {
                continue;
            };
            let Value::Array(refs) = value else {
                return Err(Error::InvalidData("animation skeleton array"));
            };
            let bytes = refs
                .len()
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or("skeleton ID size overflow")?;
            spend(&mut allocation, bytes, "skeleton IDs")?;
            out.try_reserve_exact(refs.len())
                .map_err(|_| Error::AllocationFailed)?;
            for value in refs {
                let Value::Reference(id) = value else {
                    return Err(Error::InvalidData("skeleton pointer type"));
                };
                let Some(target) = self.object(*id)? else {
                    continue;
                };
                if self.class(target.class_index)?.name != b"hkaSkeleton" {
                    return Err(Error::InvalidData("animation skeleton target class"));
                }
                out.push(*id);
            }
        }
        Ok(out)
    }
    /// Decode every remembered hkaSkeleton v3 object. Does not normalize
    /// quaternions, synthesize missing poses, change axes or construct bind/world
    /// matrices. Both graph and output budgets are required for untrusted data.
    pub fn skeletons(&self, max_output_bytes: usize) -> Result<Vec<Skeleton<'a>>> {
        self.skeletons_with_limits(SkeletonLimits {
            max_output_bytes,
            ..SkeletonLimits::default()
        })
    }
    pub fn skeletons_with_limits(&self, limits: SkeletonLimits) -> Result<Vec<Skeleton<'a>>> {
        let mut allocation = limits.max_output_bytes;
        let mut work = limits.max_validation_steps;
        let mut bones_left = limits.max_bones;
        let mut count = 0;
        for obj in &self.objects {
            spend(&mut work, 1, "skeleton objects")?;
            if self.class(obj.class_index)?.name == b"hkaSkeleton" {
                count += 1;
            }
        }
        let mut out = reserve(count, &mut allocation)?;
        for (object_index, obj) in self.objects.iter().enumerate() {
            let class = self.class(obj.class_index)?;
            if class.name != b"hkaSkeleton" {
                continue;
            }
            if class.version != 3 {
                return Err(Error::UnsupportedVersion(class.version));
            }
            let required =
                |name: &'static [u8], label| obj.field(name).ok_or(Error::MissingField(label));
            let name = match obj.field(b"name") {
                Some(Value::String(v)) => *v,
                None => None,
                _ => return Err(Error::InvalidData("skeleton name type")),
            };
            let Value::Integers {
                values: parents, ..
            } = required(b"parentIndices", "parentIndices")?
            else {
                return Err(Error::InvalidData("parent indices type"));
            };
            let Value::Array(entries) = required(b"bones", "bones")? else {
                return Err(Error::InvalidData("bones type"));
            };
            let Value::Vectors { components, data } = required(b"referencePose", "referencePose")?
            else {
                return Err(Error::InvalidData("reference pose type"));
            };
            let n = entries.len();
            spend(&mut bones_left, n, "skeleton bones")?;
            if *components != 12
                || data.len() != n.checked_mul(12).ok_or("pose size overflow")?
                || parents.len() != n
            {
                return Err(Error::InvalidData("skeleton parallel array lengths"));
            }
            let mut bones = reserve(n, &mut allocation)?;
            for (i, entry) in entries.iter().enumerate() {
                spend(&mut work, 1, "bone records")?;
                let Value::Struct(b) = entry else {
                    return Err(Error::InvalidData("bone struct type"));
                };
                if self.class(b.class_index)?.name != b"hkaBone" {
                    return Err(Error::InvalidData("bone class"));
                }
                let Some(Value::String(Some(name))) = b.field(b"name") else {
                    return Err(Error::MissingField("bone name"));
                };
                let parent = match parents[i] {
                    -1 => None,
                    x if x >= 0 && (x as u64) < n as u64 => Some(x as usize),
                    _ => return Err(Error::InvalidData("bone parent index")),
                };
                let lock_translation = match b.field(b"lockTranslation") {
                    None => false,
                    Some(Value::Byte(0)) => false,
                    Some(Value::Byte(1)) => true,
                    _ => return Err(Error::InvalidData("lockTranslation boolean")),
                };
                let mut lanes = [0.; 12];
                for (j, v) in lanes.iter_mut().enumerate() {
                    *v = data.get(i * 12 + j).ok_or("pose component")?;
                    if !v.is_finite() {
                        return Err(Error::InvalidData("nonfinite reference pose"));
                    }
                }
                bones.push(Bone {
                    name,
                    parent,
                    lock_translation,
                    rest: Transform {
                        translation: lanes[..4]
                            .try_into()
                            .map_err(|_| Error::InvalidData("translation lanes"))?,
                        rotation: lanes[4..8]
                            .try_into()
                            .map_err(|_| Error::InvalidData("rotation lanes"))?,
                        scale: lanes[8..12]
                            .try_into()
                            .map_err(|_| Error::InvalidData("scale lanes"))?,
                    },
                });
            }
            for i in 0..n {
                let mut at = Some(i);
                for _ in 0..n {
                    spend(&mut work, 1, "bone parent chains")?;
                    let Some(index) = at else {
                        break;
                    };
                    at = bones[index].parent;
                }
                if at.is_some() {
                    return Err(Error::InvalidData("bone parent cycle"));
                }
            }
            out.push(Skeleton {
                name,
                object_id: object_index + 1,
                bones,
            });
        }
        Ok(out)
    }
}

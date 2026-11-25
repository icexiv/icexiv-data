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
impl Skeleton<'_> {
    /// Unique match for a raw MDL parent bone name in this selected skeleton.
    /// Work bounds include comparisons of potentially long same-length names.
    pub fn bone_by_name(&self, name: &[u8], mut max_work: usize) -> Result<Option<usize>> {
        let mut found = None;
        for (i, b) in self.bones.iter().enumerate() {
            spend(&mut max_work, 1, "bone name lookup")?;
            if b.name.len() != name.len() {
                continue;
            }
            spend(&mut max_work, name.len(), "bone name comparison")?;
            if b.name == name && found.replace(i).is_some() {
                return Err("ambiguous bone name".into());
            }
        }
        Ok(found)
    }
    /// Transform a bone-local point (e.g. an MDL ElementId translation) into
    /// skeleton space. `pose` is a finalized sparse *absolute local* pose in
    /// bone order, not an additive layer. None uses that bone's reference TRS.
    /// Apply scale, quaternion rotation, then translation at each ancestor;
    /// this retains the effect of rotated non-uniform parent scale.
    pub fn posed_point(
        &self,
        bone: usize,
        point: [f32; 3],
        pose: &[Option<super::Sample>],
        max_steps: usize,
    ) -> Result<[f32; 3]> {
        Ok(self.transform_frame(bone, [point], pose, max_steps)?[0])
    }

    /// Skeleton-space X/Y/Z basis vectors followed by the position of a
    /// bone-local anchor. Directions never include translation; retains shear.
    pub fn posed_frame(
        &self,
        bone: usize,
        point: [f32; 3],
        pose: &[Option<super::Sample>],
        max_steps: usize,
    ) -> Result<[[f32; 3]; 4]> {
        self.transform_frame(
            bone,
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], point],
            pose,
            max_steps,
        )
    }

    fn transform_frame<const N: usize>(
        &self,
        bone: usize,
        frame: [[f32; 3]; N],
        pose: &[Option<super::Sample>],
        max_steps: usize,
    ) -> Result<[[f32; 3]; N]> {
        if pose.len() != self.bones.len() {
            return Err("pose/skeleton size".into());
        }
        let mut at = Some(bone);
        let mut remaining = max_steps;
        let mut count = 0;
        let mut frame = frame.map(|v| v.map(f64::from));
        while let Some(i) = at {
            let b = self.bones.get(i).ok_or("pose bone index")?;
            if count == self.bones.len() {
                return Err("pose parent cycle".into());
            }
            spend(&mut remaining, 1, "pose parent chain")?;
            count += 1;
            let local = pose[i].unwrap_or_else(|| super::Sample {
                translation: std::array::from_fn(|j| b.rest.translation[j]),
                rotation: b.rest.rotation,
                scale: std::array::from_fn(|j| b.rest.scale[j]),
            });
            let [x, y, z, w] = local.rotation.map(f64::from);
            let norm2 = x * x + y * y + z * z + w * w;
            if norm2 == 0. || !norm2.is_finite() {
                return Err("pose quaternion".into());
            }
            let cross = |a: [f64; 3], b: [f64; 3]| {
                [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ]
            };
            for (k, p) in frame.iter_mut().enumerate() {
                let v = std::array::from_fn(|j| p[j] * f64::from(local.scale[j]));
                let t = cross([x, y, z], v);
                let u = cross([x, y, z], t);
                *p = std::array::from_fn(|j| {
                    v[j] + 2. * (w * t[j] + u[j]) / norm2
                        + if k + 1 == N {
                            f64::from(local.translation[j])
                        } else {
                            0.
                        }
                });
            }
            at = b.parent;
        }
        let frame = frame.map(|v| v.map(|x| x as f32));
        if !frame.iter().flatten().all(|v| v.is_finite()) {
            return Err("nonfinite posed frame".into());
        }
        Ok(frame)
    }
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

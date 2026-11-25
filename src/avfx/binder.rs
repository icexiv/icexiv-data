use super::{AssetLimits, Block, Error, Fields, Kind, Node, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinderSlot {
    Start,
    First,
    Second,
    Goal,
}
#[derive(Clone, Copy, Debug)]
pub struct RingParameters {
    pub enabled: Option<i32>,
    pub progress_time: Option<i32>,
    pub position: [Option<f32>; 3],
    pub radius: Option<f32>,
}
#[derive(Clone, Copy, Debug)]
pub struct BinderProperty<'a> {
    pub slot: BinderSlot,
    pub block: Block<'a>,
    pub fields: Fields<'a>,
    pub bind_point: Option<i32>,
    pub target_point: Option<i32>,
    pub bind_point_id: Option<i32>,
    /// Raw NUL-terminated name, without padding. The token `null` is retained
    /// literally, not treated as absence. An encoded empty block is Some(empty).
    pub name: Option<&'a [u8]>,
    pub generate_delay: Option<i32>,
    pub coordinate_update_frame: Option<i32>,
    pub coordinate_type: Option<i32>,
    pub ring: RingParameters,
    pub position: Option<Fields<'a>>,
}
impl BinderProperty<'_> {
    /// CoUF gate on a coordinate update that the player is attempting, using
    /// that update's local frame. A negative signed low word leaves the gate
    /// open; a finite cutoff is inclusive. None preserves an omitted field.
    /// This does not schedule updates or assert that the host query succeeded.
    pub fn coordinate_refresh_allowed(self, local_frame: f32) -> Result<Option<bool>> {
        if !local_frame.is_finite() {
            return Err("nonfinite binder coordinate frame".into());
        }
        Ok(self.coordinate_update_frame.map(|v| {
            let cutoff = v as i16;
            cutoff < 0 || local_frame <= f32::from(cutoff)
        }))
    }
}
impl<'a> Node<'a> {
    /// Prepare the SpS curve for a point binder with no additional motion
    /// channels. An SpSR channel or a different binder variant is not folded
    /// into the deterministic result. `sample` takes authored frame units.
    pub fn point_sps_curve(self, limits: AssetLimits) -> Result<Option<super::ScalarCurve>> {
        if self.kind != Kind::Binder {
            return Err("not a binder".into());
        }
        let remaining = limits
            .max_work
            .checked_sub(self.fields.len().saturating_mul(2))
            .ok_or(Error::BudgetExceeded)?;
        if self.variant()? != Some(0) {
            return Ok(None);
        }
        let Some(data) = self.fields.one(b"Data")? else {
            return Ok(None);
        };
        let fields = data.fields(limits.max_blocks.min(remaining))?;
        if fields.len() != 1 {
            return Ok(None);
        }
        let Some(sps) = fields.one(b"SpS")? else {
            return Ok(None);
        };
        let remaining = remaining.checked_sub(2).ok_or(Error::BudgetExceeded)?;
        let curve = super::Curve::parse(sps, (limits.max_blocks - 1).min(remaining / 6))?;
        if curve
            .fields
            .len()
            .saturating_mul(6)
            .saturating_add(curve.keys().len())
            > remaining
        {
            return Err(Error::BudgetExceeded);
        }
        curve
            .scalar(limits.max_items, limits.max_output_bytes)
            .map(Some)
    }
    /// The single-key SpS value for an unrandomized point binder with the
    /// constant profile observed on w5004/b0008. None means this profile
    /// cannot supply a value; it never substitutes 1 for an unknown curve.
    pub fn constant_point_sps(self, max_blocks: usize) -> Result<Option<f32>> {
        if self.kind != Kind::Binder {
            return Err("not a binder".into());
        }
        if self.variant()? != Some(0) {
            return Ok(None);
        }
        let Some(data) = self.fields.one(b"Data")? else {
            return Ok(None);
        };
        let fields = data.fields(max_blocks)?;
        let Some(sps) = fields.one(b"SpS")? else {
            return Ok(None);
        };
        if fields.len() != 1 {
            return Ok(None);
        }
        let curve = super::Curve::parse(sps, max_blocks - fields.len())?;
        if curve.fields.len() != 5 || curve.fields.integer(b"KeyC")? != Some(1) {
            return Ok(None);
        }
        for name in [b"BvPr", b"BvPo", b"RanT"] {
            if curve.fields.integer(name)? != Some(0) {
                return Ok(None);
            }
        }
        let key = curve.keys().next().ok_or("missing point binder key")?;
        let [a, b, c] = key.values;
        Ok((key.time == 0
            && key.kind == 1
            && a.is_finite()
            && a.to_bits() == b.to_bits()
            && a.to_bits() == c.to_bits())
        .then_some(a))
    }
    /// Encoded bFTO and bTSc bytes, in that order. Preserve 0xff, 1, 0
    /// and absence separately; these are not four-byte property-slot flags.
    pub fn binder_transform_flags(self) -> Result<BinderTransformFlags> {
        if self.kind != Kind::Binder {
            return Err("not a binder".into());
        }
        let byte = |name| self.fields.one(name)?.map(Block::byte).transpose();
        Ok(BinderTransformFlags {
            follow: byte(b"bFTO")?,
            scale: byte(b"bTSc")?,
        })
    }
    /// Decode property slots as asset metadata. Does not choose a caster/target,
    /// supply missing-slot defaults, resolve BPID globally or assemble a pose.
    pub fn binder_properties(self, limits: AssetLimits) -> Result<Vec<BinderProperty<'a>>> {
        if self.kind != Kind::Binder {
            return Err("not a binder".into());
        }
        let mut work = limits.max_work;
        let mut blocks = limits.max_blocks;
        let mut bytes = limits.max_output_bytes;
        let spend = |budget: &mut usize, n: usize| -> Result<()> {
            *budget = budget.checked_sub(n).ok_or(Error::BudgetExceeded)?;
            Ok(())
        };
        spend(&mut work, self.fields.len())?;
        let classify = |b: Block<'_>| {
            if b.is(b"PrpS") {
                Some((0, BinderSlot::Start))
            } else if b.is(b"Prp1") {
                Some((1, BinderSlot::First))
            } else if b.is(b"Prp2") {
                Some((2, BinderSlot::Second))
            } else if b.is(b"PrpG") {
                Some((3, BinderSlot::Goal))
            } else {
                None
            }
        };
        let count = self
            .fields
            .iter()
            .filter(|b| classify(*b).is_some())
            .count();
        if count > limits.max_items {
            return Err(Error::BudgetExceeded);
        }
        spend(
            &mut bytes,
            count
                .checked_mul(std::mem::size_of::<BinderProperty<'_>>())
                .ok_or("binder allocation overflow")?,
        )?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut seen = 0u8;
        for block in self.fields.iter() {
            let Some((index, slot)) = classify(block) else {
                continue;
            };
            if seen & (1 << index) != 0 {
                return Err("duplicate binder property slot".into());
            }
            seen |= 1 << index;
            let fields = block.fields(blocks)?;
            spend(&mut blocks, fields.len())?;
            spend(
                &mut work,
                fields.len().checked_mul(18).ok_or("binder work overflow")?,
            )?;
            let name = if let Some(b) = fields.one(b"Name")? {
                spend(
                    &mut work,
                    b.payload
                        .len()
                        .checked_mul(2)
                        .ok_or("binder name work overflow")?,
                )?;
                Some(if b.payload.is_empty() {
                    &[][..]
                } else {
                    b.string()?
                })
            } else {
                None
            };
            let position = if let Some(b) = fields.one(b"Pos")? {
                let f = b.fields(blocks)?;
                spend(&mut blocks, f.len())?;
                spend(&mut work, f.len())?;
                Some(f)
            } else {
                None
            };
            output.push(BinderProperty {
                slot,
                block,
                fields,
                bind_point: fields.integer(b"BPT")?,
                target_point: fields.integer(b"BPTP")?,
                bind_point_id: fields.integer(b"BPID")?,
                name,
                generate_delay: fields.integer(b"GenD")?,
                coordinate_update_frame: fields.integer(b"CoUF")?,
                coordinate_type: fields.integer(b"BCT")?,
                position,
                ring: RingParameters {
                    enabled: fields.integer(b"bRng")?,
                    progress_time: fields.integer(b"RnPT")?,
                    position: [
                        fields.real(b"RnPX")?,
                        fields.real(b"RnPY")?,
                        fields.real(b"RnPZ")?,
                    ],
                    radius: fields.real(b"RnRd")?,
                },
            });
        }
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BinderTransformFlags {
    pub follow: Option<u8>,
    pub scale: Option<u8>,
}

/// Host-coordinate basis (three XYZ axes), translation and separate axis
/// lengths at the successful binder coordinate lookup. The caller supplies
/// the frame for the selected slot/time; distance/ring updates are separate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BinderHostFrame {
    pub axes: [[f32; 3]; 3],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

impl BinderTransformFlags {
    /// Split the supplied affine into the orientation and scale used by the
    /// Win32 host-frame path. `axes[i]` is the image of local unit axis i.
    /// Keeps reflections and shear directions, without orthogonalizing them.
    pub fn host_frame(self, axes: [[f32; 3]; 3], translation: [f32; 3]) -> Result<BinderHostFrame> {
        if !axes
            .iter()
            .flatten()
            .chain(&translation)
            .all(|v| v.is_finite())
        {
            return Err("nonfinite binder host input".into());
        }
        let follow = self.follow.unwrap_or(1) != 0;
        let mut scale = [1.; 3];
        let mut basis = [[0.; 3]; 3];
        for (i, [x, y, z]) in axes.into_iter().enumerate() {
            let len = ((y * y + x * x) + z * z).sqrt();
            if !len.is_finite() {
                return Err("binder axis length overflow".into());
            }
            let denom = if follow { len + f32::EPSILON } else { len };
            if self.scale.unwrap_or(0) != 0 {
                scale[i] = denom;
            }
            if follow {
                let inv = 1. / denom;
                basis[i] = [x * inv, y * inv, z * inv];
            } else {
                basis[i][i] = 1.;
            }
        }
        Ok(BinderHostFrame {
            axes: basis,
            translation,
            scale,
        })
    }
}

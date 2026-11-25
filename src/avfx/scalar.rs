//! Scalar AVFX curve evaluation in authored frame units.
use super::{Curve, Error, Result};

#[derive(Clone, Copy, Debug)]
struct Knot {
    time: f32,
    value: f32,
    shape: [f32; 2],
    kind: u16,
}

#[derive(Debug)]
pub struct ScalarCurve {
    knots: Vec<Knot>,
    pre: i32,
    post: i32,
}

impl Curve<'_> {
    /// Prepare a deterministic scalar curve (Z value, X/Y handle coefficients).
    /// Call only for scalar parameters: RGB keys have different lane semantics.
    /// Requires explicit pre/post behavior and RanT=0. No RNG is synthesized.
    pub fn scalar(self, max_keys: usize, max_output_bytes: usize) -> Result<ScalarCurve> {
        if self.fields.integer(b"RanT")? != Some(0) {
            return Err("scalar random mode".into());
        }
        self.scalar_shape(max_keys, max_output_bytes)
    }
    pub(super) fn scalar_shape(
        self,
        max_keys: usize,
        max_output_bytes: usize,
    ) -> Result<ScalarCurve> {
        let keys = self.keys();
        if keys.len() > max_keys
            || keys
                .len()
                .checked_mul(std::mem::size_of::<Knot>())
                .is_none_or(|n| n > max_output_bytes)
        {
            return Err(Error::BudgetExceeded);
        }
        if keys.len() == 0 || keys.len() > 255 {
            return Err("scalar key count".into());
        }
        let behavior = |name| match self.fields.integer(name)? {
            Some(v @ 0..=2) => Ok(v),
            _ => Err(Error::from("unknown scalar boundary behavior")),
        };
        let pre = behavior(b"BvPr")?;
        let post = behavior(b"BvPo")?;
        let mut knots = Vec::new();
        knots
            .try_reserve_exact(keys.len())
            .map_err(|_| Error::AllocationFailed)?;
        let mut last = -1;
        for key in keys {
            let [x, y, value] = key.values;
            if key.time <= last || key.time > 0x3fff || key.kind > 2 || !value.is_finite() {
                return Err("scalar key range/order".into());
            }
            let shape = |v: f32| -> Result<f32> {
                let code = (v * 15.).trunc();
                if !(-128. ..=127.).contains(&code) {
                    return Err("scalar handle out of range".into());
                }
                Ok(code * (1. / 15.))
            };
            knots.push(Knot {
                time: f32::from(key.time),
                value,
                shape: [shape(x)?, shape(y)?],
                kind: key.kind,
            });
            last = key.time;
        }
        Ok(ScalarCurve { knots, pre, post })
    }
}

impl ScalarCurve {
    /// Evaluate spline, linear and step segments with constant, repeated or
    /// accumulated boundaries. No allocation after preparation.
    pub fn sample(&self, mut frame: f32) -> Result<f32> {
        if !frame.is_finite() {
            return Err("nonfinite curve time".into());
        }
        let k = &self.knots;
        let a = k[0];
        let b = k[k.len() - 1];
        if k.len() == 1 {
            return Ok(a.value);
        }
        let mut offset = 0.;
        if frame < a.time || frame > b.time {
            let before = frame < a.time;
            let mode = if before { self.pre } else { self.post };
            if mode == 0 {
                return Ok(if before { a.value } else { b.value });
            }
            let span = b.time - a.time;
            let distance = if before {
                a.time - frame
            } else {
                frame - b.time
            };
            // Native integer period counts, with the opposite endpoint at an
            // exact exterior multiple. Keep conversion and increment in range.
            if distance >= i32::MAX as f32 {
                return Err("curve period count overflow".into());
            }
            let n = (distance as i32 / span as i32 + 1) as f32 * if before { -1. } else { 1. };
            frame -= n * span;
            if !(a.time..=b.time).contains(&frame) {
                return Err("curve period precision".into());
            }
            if mode == 2 {
                offset = n * (b.value - a.value);
            }
        }
        let j = k.partition_point(|p| p.time < frame).clamp(1, k.len() - 1);
        let l = k[j - 1];
        let r = k[j];
        let span = r.time - l.time;
        let t = (frame - l.time) / span;
        let delta = r.value - l.value;
        let value = match r.kind {
            1 => l.value + t * delta,
            2 => {
                if frame < r.time {
                    l.value
                } else {
                    r.value
                }
            }
            _ => {
                let mut m0 = l.shape[0] * delta;
                if j >= 2 {
                    let p = k[j - 2];
                    m0 = (l.shape[1] * (l.value - p.value) + m0) * (span / (r.time - p.time));
                }
                let mut m1 = r.shape[1] * delta;
                if let Some(n) = k.get(j + 1) {
                    m1 = (r.shape[0] * (n.value - r.value) + m1) * (span / (n.time - l.time));
                }
                let t2 = t * t;
                let t3 = t2 * t;
                ((2. * t3 - 3. * t2 + 1.) * l.value + (3. * t2 - 2. * t3) * r.value)
                    + (t3 - 2. * t2 + t) * m0
                    + (t3 - t2) * m1
            }
        } + offset;
        if value.is_finite() {
            Ok(value)
        } else {
            Err("scalar curve overflow".into())
        }
    }
}

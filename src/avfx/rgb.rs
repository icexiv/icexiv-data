use super::{Curve, Error, Result};

/// Validated RGB keys; sampling borrows the AVFX bytes and allocates nothing.
#[derive(Clone, Copy, Debug)]
pub struct RgbCurve<'a> {
    keys: &'a [[u8; 16]],
    repeat_before: bool,
    repeat_after: bool,
}

fn time(k: &[u8; 16]) -> f32 {
    u16::from_le_bytes([k[0], k[1]]).into()
}
fn values(k: &[u8; 16]) -> [f32; 3] {
    std::array::from_fn(|i| {
        let j = 4 + 4 * i;
        f32::from_le_bytes([k[j], k[j + 1], k[j + 2], k[j + 3]])
    })
}

impl<'a> Curve<'a> {
    /// Interpret all three lanes as colors. Unlike scalar curves, mode 0 and
    /// mode 1 use the same linear segment and boundary mode 2 repeats without
    /// accumulating a value delta.
    pub fn rgb(self, max_keys: usize) -> Result<RgbCurve<'a>> {
        let (keys, _) = self.keys.as_chunks::<16>();
        if keys.len() > max_keys {
            return Err(Error::BudgetExceeded);
        }
        if keys.is_empty() || keys.len() > 127 {
            return Err("RGB key count".into());
        }
        let boundary = |name| match self.fields.integer(name)? {
            None | Some(0) => Ok(false),
            Some(1 | 2) => Ok(true),
            _ => Err(Error::from("unknown RGB boundary behavior")),
        };
        let mut last = -1;
        for k in keys {
            let t = i16::from_le_bytes([k[0], k[1]]);
            if t <= last
                || u16::from_le_bytes([k[2], k[3]]) > 2
                || !values(k).iter().all(|v| v.is_finite())
            {
                return Err("RGB key range/order".into());
            }
            last = t;
        }
        Ok(RgbCurve {
            keys,
            repeat_before: boundary(b"BvPr")?,
            repeat_after: boundary(b"BvPo")?,
        })
    }
}

impl RgbCurve<'_> {
    /// RGB in authored frame units, without alpha, color-space conversion or
    /// clamping to display range.
    pub fn sample(self, mut frame: f32) -> Result<[f32; 3]> {
        if !frame.is_finite() {
            return Err("nonfinite RGB frame".into());
        }
        let first = &self.keys[0];
        if self.keys.len() == 1 {
            return Ok(values(first));
        }
        let last = &self.keys[self.keys.len() - 1];
        let (lo, hi) = (time(first), time(last));
        if frame <= lo || frame >= hi {
            let before = frame <= lo;
            if !(if before {
                self.repeat_before
            } else {
                self.repeat_after
            }) {
                return Ok(values(if before { first } else { last }));
            }
            let dist = if before { lo - frame } else { frame - hi };
            if dist >= i32::MAX as f32 {
                return Err("RGB repeat range".into());
            }
            let span = hi - lo;
            let n = dist as i32 / span as i32 + 1;
            let step = n as f32 * span;
            frame = if before { frame + step } else { frame - step };
            if !(lo..=hi).contains(&frame) {
                return Err("RGB repeat precision".into());
            }
        }
        let j = self
            .keys
            .partition_point(|k| time(k) < frame)
            .clamp(1, self.keys.len() - 1);
        let (a, b) = (&self.keys[j - 1], &self.keys[j]);
        if b[2] == 2 {
            return Ok(values(if frame < time(b) { a } else { b }));
        }
        let t = (frame - time(a)) / (time(b) - time(a));
        let (a, b) = (values(a), values(b));
        let rgb = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
        if rgb.iter().all(|v| v.is_finite()) {
            Ok(rgb)
        } else {
            Err("RGB sample overflow".into())
        }
    }
}

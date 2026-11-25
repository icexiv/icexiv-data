use super::{Curve, Result, ScalarCurve};

/// Explicit stream state for AVFX curve variation. The caller determines
/// which instances share a stream and in what order they consume it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurveRng(pub [u32; 4]);
impl CurveRng {
    pub fn next_u16(&mut self) -> u16 {
        let [a, b, c, d] = self.0;
        let t = a ^ (a << 11);
        let n = d ^ (d >> 19) ^ t ^ (t >> 8);
        self.0 = [b, c, d, n];
        n as u16
    }
}

/// Scalar amplitude for a random channel chosen by its enclosing parameter.
/// A trailing R alone does not identify a random channel (e.g. SclR is red).
#[derive(Debug)]
pub struct RandomAmplitude {
    curve: ScalarCurve,
    mode: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct RandomAmplitudeInstance<'a> {
    amp: &'a RandomAmplitude,
    percent: i8,
}
impl Curve<'_> {
    pub fn random_amplitude(
        self,
        max_keys: usize,
        max_output_bytes: usize,
    ) -> Result<RandomAmplitude> {
        let mode = match self.fields.integer(b"RanT")? {
            Some(v @ 0..=5) => v as u8,
            _ => return Err("unknown scalar random mode".into()),
        };
        Ok(RandomAmplitude {
            curve: self.scalar_shape(max_keys, max_output_bytes)?,
            mode,
        })
    }
}
impl RandomAmplitude {
    /// Modes 0–2 consume one draw here and retain a signed integer percent.
    /// Modes 3–5 consume their draw at evaluation time.
    pub fn instantiate(&self, rng: &mut CurveRng) -> RandomAmplitudeInstance<'_> {
        let percent = if self.mode < 3 {
            let n = f32::from(rng.next_u16());
            let u = match self.mode {
                0 => (n + n) / 65535. - 1.,
                1 => n / 65535.,
                _ => n / 65535. - 1.,
            };
            (u * 100.) as i8
        } else {
            0
        };
        RandomAmplitudeInstance { amp: self, percent }
    }
}
impl RandomAmplitudeInstance<'_> {
    pub fn sample(self, frame: f32, base: f32, rng: &mut CurveRng) -> Result<f32> {
        if !base.is_finite() {
            return Err("nonfinite scalar base".into());
        }
        let a = self.amp.curve.sample(frame)?;
        let offset = if self.amp.mode < 3 {
            (f32::from(self.percent) * 0.01) * a
        } else {
            let (lo, hi) = match self.amp.mode {
                3 => (-a, a),
                4 => (0., a),
                _ => (-a, 0.),
            };
            (hi - lo) * f32::from(rng.next_u16()) / 65535. + lo
        };
        let out = base + offset;
        if out.is_finite() {
            Ok(out)
        } else {
            Err("random scalar overflow".into())
        }
    }
}

//! Spline-compressed transform tracks. Sampling returns track-space TRS;
//! binding, additive composition and skeleton assembly remain separate.
use super::{Error, Object, Result, Tagfile, Value, reserve, spend};

#[derive(Clone, Copy, Debug)]
pub struct SplineLimits {
    pub max_allocation_bytes: usize,
    pub max_tracks: usize,
    pub max_blocks: usize,
    pub max_control_points: usize,
    pub max_work: usize,
}
impl Default for SplineLimits {
    fn default() -> Self {
        Self {
            max_allocation_bytes: 128 << 20,
            max_tracks: 65536,
            max_blocks: 65536,
            max_control_points: 2_000_000,
            max_work: 32_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}
#[derive(Debug)]
struct Curve<'a> {
    knots: &'a [u8],
    degree: usize,
    controls: Vec<[f64; 4]>,
}
#[derive(Debug)]
struct Track<'a> {
    position: Curve<'a>,
    rotation: Curve<'a>,
    scale: Curve<'a>,
}
#[derive(Debug)]
pub struct SplineBlock<'a> {
    tracks: Vec<Track<'a>>,
}
#[derive(Debug)]
pub struct SplineAnimation<'a> {
    pub object_id: usize,
    pub duration_seconds: f32,
    pub frame_duration_seconds: f32,
    pub num_frames: usize,
    pub max_frames_per_block: usize,
    pub blocks: Vec<SplineBlock<'a>>,
}
pub(super) fn integer(o: &Object<'_>, name: &'static [u8]) -> Result<usize> {
    match o.field(name) {
        Some(Value::Integer(v)) => {
            usize::try_from(*v).map_err(|_| Error::InvalidData("animation integer"))
        }
        _ => Err(Error::InvalidData("missing animation integer")),
    }
}
fn real(o: &Object<'_>, name: &[u8]) -> Result<f32> {
    match o.field(name) {
        Some(Value::Real(v)) if v.is_finite() => Ok(*v),
        _ => Err(Error::InvalidData("animation real")),
    }
}
fn integers<'a>(o: &'a Object<'_>, name: &[u8]) -> Result<&'a [i64]> {
    match o.field(name) {
        Some(Value::Integers { values, .. }) => Ok(values),
        _ => Err(Error::InvalidData("animation offset array")),
    }
}
struct Budget {
    bytes: usize,
    controls: usize,
    work: usize,
}
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("spline span overflow")?;
        let v = self.data.get(self.pos..end).ok_or("truncated spline")?;
        self.pos = end;
        Ok(v)
    }
    fn align(&mut self, n: usize) -> Result<()> {
        self.take(self.pos.wrapping_neg() & (n - 1))?;
        Ok(())
    }
    fn scalar(&mut self) -> Result<f64> {
        let b = self.take(4)?;
        let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if !v.is_finite() {
            return Err(Error::InvalidData("nonfinite spline scalar"));
        }
        Ok(f64::from(v))
    }
    fn shape(&mut self, b: &mut Budget) -> Result<(usize, usize, &'a [u8])> {
        let h = self.take(3)?;
        let n = usize::from(u16::from_le_bytes([h[0], h[1]])) + 1;
        let degree = usize::from(h[2]);
        if degree > 8 || n <= degree {
            return Err(Error::InvalidData("spline degree/control count"));
        }
        let knots = self.take(n + degree + 1)?;
        spend(&mut b.work, knots.len(), "spline knots")?;
        if knots.windows(2).any(|p| p[0] > p[1]) || knots[degree] >= knots[n] {
            return Err(Error::InvalidData("spline knot domain"));
        }
        Ok((n, degree, knots))
    }
}
fn points(n: usize, b: &mut Budget) -> Result<Vec<[f64; 4]>> {
    spend(&mut b.controls, n, "spline controls")?;
    spend(&mut b.work, n, "spline control decoding")?;
    reserve(n, &mut b.bytes)
}
fn vector<'a>(
    c: &mut Cursor<'a>,
    flags: u8,
    q: u8,
    identity: f64,
    b: &mut Budget,
) -> Result<Curve<'a>> {
    if flags & 0x88 != 0 || q > 1 {
        return Err(Error::InvalidData("vector spline encoding"));
    }
    let states = std::array::from_fn::<_, 3, _>(|i| {
        if flags & (1 << i) != 0 {
            1
        } else if flags & (16 << i) != 0 {
            2
        } else {
            0
        }
    });
    let (n, degree, knots) = if states.contains(&2) {
        let shape = c.shape(b)?;
        c.align(4)?;
        shape
    } else {
        (1, 0, &[][..])
    };
    let mut bounds = [[identity; 2]; 3];
    for i in 0..3 {
        if states[i] == 1 {
            let v = c.scalar()?;
            bounds[i] = [v, v];
        } else if states[i] == 2 {
            bounds[i] = [c.scalar()?, c.scalar()?];
        }
    }
    let mut controls = points(n, b)?;
    for _ in 0..n {
        let mut v = [0.; 4];
        for i in 0..3 {
            v[i] = if states[i] == 2 {
                let raw = c.take(usize::from(q) + 1)?;
                let code = if q == 0 {
                    u16::from(raw[0])
                } else {
                    u16::from_le_bytes([raw[0], raw[1]])
                };
                bounds[i][0]
                    + (bounds[i][1] - bounds[i][0]) * f64::from(code)
                        / if q == 0 { 255. } else { 65535. }
            } else {
                bounds[i][0]
            };
        }
        controls.push(v);
    }
    if !knots.is_empty() {
        c.align(4)?;
    }
    Ok(Curve {
        knots,
        degree,
        controls,
    })
}
fn quaternion(c: &mut Cursor<'_>, encoding: u8) -> Result<[f64; 4]> {
    if encoding == 5 {
        return Ok([c.scalar()?, c.scalar()?, c.scalar()?, c.scalar()?]);
    }
    if encoding == 0 {
        let raw = c.take(4)?;
        let code = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let r = f64::from((code >> 18) & 1023) / 1023.;
        let w = 1. - r * r;
        let index = code & 262143;
        let band = f64::from(index).sqrt().floor();
        let polar = band * std::f64::consts::PI / 1022.;
        let azimuth = if band == 0. {
            0.
        } else {
            (f64::from(index) - band * band) * std::f64::consts::PI / (4. * band)
        };
        let radius = (1. - w * w).max(0.).sqrt();
        let mut out = [
            radius * polar.sin() * azimuth.cos(),
            radius * polar.sin() * azimuth.sin(),
            radius * polar.cos(),
            w,
        ];
        for (i, v) in out.iter_mut().enumerate() {
            if code & (1 << (28 + i)) != 0 {
                *v = -*v;
            }
        }
        return Ok(out);
    }
    let (values, missing, negative) = match encoding {
        1 => {
            let raw = c.take(5)?;
            let mut bytes = [0; 8];
            bytes[..5].copy_from_slice(raw);
            let code = u64::from_le_bytes(bytes);
            (
                std::array::from_fn::<_, 3, _>(|i| {
                    (((code >> (12 * i)) & 4095) as f64 - 2047.) / (2047. * 2f64.sqrt())
                }),
                ((code >> 36) & 3) as usize,
                code & (1 << 38) != 0,
            )
        }
        2 => {
            let raw = c.take(6)?;
            let words = std::array::from_fn::<_, 3, _>(|i| {
                u16::from_le_bytes([raw[2 * i], raw[2 * i + 1]])
            });
            (
                words.map(|v| (f64::from(v & 32767) - 16383.) / (16383. * 2f64.sqrt())),
                usize::from((words[0] >> 15) | ((words[1] >> 15) << 1)),
                words[2] & 32768 != 0,
            )
        }
        _ => return Err(Error::UnsupportedType(u32::from(encoding))),
    };
    let residual = 1. - values.iter().map(|v| v * v).sum::<f64>();
    if residual < 0. {
        return Err(Error::InvalidData("quaternion residual"));
    }
    let mut out = [0.; 4];
    let mut at = 0;
    for (i, v) in out.iter_mut().enumerate() {
        if i == missing {
            *v = residual.sqrt() * if negative { -1. } else { 1. };
        } else {
            *v = values[at];
            at += 1;
        }
    }
    Ok(out)
}
fn rotation<'a>(c: &mut Cursor<'a>, flags: u8, q: u8, b: &mut Budget) -> Result<Curve<'a>> {
    if !matches!(q, 0 | 1 | 2 | 5) {
        return Err(Error::UnsupportedType(u32::from(q)));
    }
    let (n, degree, knots) = if flags & 240 != 0 {
        let shape = c.shape(b)?;
        c.align(match q {
            0 | 5 => 4,
            2 => 2,
            _ => 1,
        })?;
        shape
    } else {
        (1, 0, &[][..])
    };
    let mut controls = points(n, b)?;
    for _ in 0..n {
        let v = if flags == 0 {
            [0., 0., 0., 1.]
        } else {
            quaternion(c, q)?
        };
        if v.iter().map(|v| v * v).sum::<f64>() == 0. {
            return Err(Error::InvalidData("zero quaternion control"));
        }
        controls.push(v);
    }
    Ok(Curve {
        knots,
        degree,
        controls,
    })
}
impl<'a> Tagfile<'a> {
    pub fn spline_animation(
        &self,
        object_id: usize,
        limits: SplineLimits,
    ) -> Result<SplineAnimation<'a>> {
        let o = self.object(object_id)?.ok_or("null animation")?;
        let class = self.class(o.class_index)?;
        if class.name != b"hkaSplineCompressedAnimation" {
            return Err(Error::InvalidData("animation class"));
        }
        if class.version != 0 {
            return Err(Error::UnsupportedVersion(class.version));
        }
        if integer(o, b"type")? != 3 {
            return Err(Error::InvalidData("spline animation type"));
        }
        if o.field(b"numberOfFloatTracks").is_some() && integer(o, b"numberOfFloatTracks")? != 0 {
            return Err(Error::InvalidData(
                "float animation tracks require decoding",
            ));
        }
        let n = integer(o, b"numberOfTransformTracks")?;
        let count = integer(o, b"numBlocks")?;
        if n > limits.max_tracks || count > limits.max_blocks {
            return Err(Error::BudgetExceeded("spline tracks/blocks"));
        }
        let frames = integer(o, b"numFrames")?;
        let max_frames = integer(o, b"maxFramesPerBlock")?;
        if frames == 0
            || max_frames < 2
            || max_frames > 256
            || count == 0
            || count != frames.saturating_sub(1).div_ceil(max_frames - 1).max(1)
        {
            return Err(Error::InvalidData("animation frame/block counts"));
        }
        let duration = real(o, b"duration")?;
        let frame_duration = real(o, b"frameDuration")?;
        if duration < 0. || frame_duration <= 0. {
            return Err(Error::InvalidData("animation durations"));
        }
        let offsets = integers(o, b"blockOffsets")?;
        let floats = integers(o, b"floatBlockOffsets")?;
        if offsets.len() != count
            || floats.len() != count
            || offsets[0] != 0
            || integer(o, b"maskAndQuantizationSize")?
                != n.checked_mul(4).ok_or("mask size overflow")?
        {
            return Err(Error::InvalidData("spline offset lengths"));
        }
        let Some(Value::Bytes(data)) = o.field(b"data") else {
            return Err(Error::InvalidData("spline data bytes"));
        };
        let mut b = Budget {
            bytes: limits.max_allocation_bytes,
            controls: limits.max_control_points,
            work: limits.max_work,
        };
        let mut blocks = reserve(count, &mut b.bytes)?;
        for i in 0..count {
            let start =
                usize::try_from(offsets[i]).map_err(|_| Error::InvalidData("block offset"))?;
            if start % 4 != 0 {
                return Err(Error::InvalidData("unaligned spline block"));
            }
            let end = if i + 1 < count {
                usize::try_from(offsets[i + 1]).map_err(|_| Error::InvalidData("block offset"))?
            } else {
                data.len()
            };
            let bytes = data.get(start..end).ok_or("block range")?;
            let mut c = Cursor {
                data: bytes,
                pos: 0,
            };
            let masks = c.take(n.checked_mul(4).ok_or("mask overflow")?)?;
            c.align(4)?;
            let mut tracks = reserve(n, &mut b.bytes)?;
            spend(&mut b.work, n, "spline tracks")?;
            for mask in masks.chunks_exact(4) {
                let position = vector(&mut c, mask[1], mask[0] & 3, 0., &mut b)?;
                let rotation = rotation(&mut c, mask[2], (mask[0] >> 2) & 15, &mut b)?;
                c.align(4)?;
                let scale = vector(&mut c, mask[3], mask[0] >> 6, 1., &mut b)?;
                tracks.push(Track {
                    position,
                    rotation,
                    scale,
                });
            }
            if usize::try_from(floats[i]).ok() != Some(c.pos) {
                return Err(Error::InvalidData("float block boundary"));
            }
            spend(&mut b.work, bytes.len() - c.pos, "spline padding")?;
            if bytes[c.pos..].iter().any(|v| *v != 0) {
                return Err(Error::InvalidData("nonzero spline block tail"));
            }
            blocks.push(SplineBlock { tracks });
        }
        Ok(SplineAnimation {
            object_id,
            duration_seconds: duration,
            frame_duration_seconds: frame_duration,
            num_frames: frames,
            max_frames_per_block: max_frames,
            blocks,
        })
    }
}
impl Curve<'_> {
    fn sample(&self, time: f64, work: &mut usize) -> Result<[f64; 4]> {
        let n = self.controls.len();
        if self.knots.is_empty() {
            return Ok(self.controls[0]);
        }
        let p = self.degree;
        let t = time.clamp(f64::from(self.knots[p]), f64::from(self.knots[n]));
        spend(work, self.knots.len(), "sample knot search")?;
        let span = if t == f64::from(self.knots[n]) {
            n - 1
        } else {
            self.knots
                .partition_point(|v| f64::from(*v) <= t)
                .saturating_sub(1)
                .min(n - 1)
        };
        let mut d = [[0.; 4]; 9];
        d[..=p].copy_from_slice(&self.controls[span - p..=span]);
        for r in 1..=p {
            for j in (r..=p).rev() {
                spend(work, 4, "spline evaluation")?;
                let i = span - p + j;
                let lo = f64::from(self.knots[i]);
                let hi = f64::from(self.knots[i + p - r + 1]);
                let alpha = if hi == lo { 0. } else { (t - lo) / (hi - lo) };
                for k in 0..4 {
                    d[j][k] = (1. - alpha) * d[j - 1][k] + alpha * d[j][k];
                }
            }
        }
        Ok(d[p])
    }
}
impl SplineBlock<'_> {
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
    /// Local block frame coordinate; curve domains clamp independently.
    pub fn sample(
        &self,
        frame: f64,
        max_output_bytes: usize,
        max_work: usize,
    ) -> Result<Vec<Sample>> {
        if !frame.is_finite() {
            return Err(Error::InvalidData("nonfinite sample frame"));
        }
        let mut bytes = max_output_bytes;
        let mut work = max_work;
        let mut out = reserve(self.tracks.len(), &mut bytes)?;
        for t in &self.tracks {
            spend(&mut work, 1, "sample tracks")?;
            let p = t.position.sample(frame, &mut work)?;
            let mut q = t.rotation.sample(frame, &mut work)?;
            let s = t.scale.sample(frame, &mut work)?;
            let norm = q.iter().map(|v| v * v).sum::<f64>().sqrt();
            if !norm.is_finite() || norm == 0. {
                return Err(Error::InvalidData("sample quaternion norm"));
            }
            for v in &mut q {
                *v /= norm;
            }
            let translation = [p[0] as f32, p[1] as f32, p[2] as f32];
            let scale = [s[0] as f32, s[1] as f32, s[2] as f32];
            if translation.iter().chain(&scale).any(|v| !v.is_finite()) {
                return Err(Error::InvalidData("sample scalar overflow"));
            }
            out.push(Sample {
                translation,
                rotation: q.map(|v| v as f32),
                scale,
            });
        }
        Ok(out)
    }
}
impl SplineAnimation<'_> {
    /// Finite milliseconds, clamped to [0,duration]. Does not apply blendHint.
    pub fn sample(
        &self,
        milliseconds: f64,
        max_output_bytes: usize,
        max_work: usize,
    ) -> Result<Vec<Sample>> {
        if !milliseconds.is_finite() {
            return Err(Error::InvalidData("nonfinite sample time"));
        }
        if self.blocks.is_empty()
            || !(2..=256).contains(&self.max_frames_per_block)
            || !self.duration_seconds.is_finite()
            || self.duration_seconds < 0.
            || !self.frame_duration_seconds.is_finite()
            || self.frame_duration_seconds <= 0.
        {
            return Err(Error::InvalidData("sample animation metadata"));
        }
        let frame = (milliseconds / 1000.).clamp(0., f64::from(self.duration_seconds))
            / f64::from(self.frame_duration_seconds);
        let stride = self.max_frames_per_block - 1;
        let block = ((frame / (stride as f64)).floor() as usize).min(self.blocks.len() - 1);
        let start = block
            .checked_mul(stride)
            .ok_or("sample block frame overflow")?;
        self.blocks[block].sample(frame - start as f64, max_output_bytes, max_work)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_quaternion_encoding_landmarks() {
        let q40 = 2047u64 | (2047 << 12) | (2047 << 24) | (3 << 36);
        let q48 = [16383u16 | 32768, 16383 | 32768, 16383];
        let bytes48 = q48
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        for (encoding, raw) in [
            (0, 0u32.to_le_bytes().to_vec()),
            (1, q40.to_le_bytes()[..5].to_vec()),
            (2, bytes48),
        ] {
            let mut c = Cursor { data: &raw, pos: 0 };
            assert_eq!(quaternion(&mut c, encoding).unwrap(), [0., 0., 0., 1.]);
            assert_eq!(c.pos, raw.len());
        }
        let raw = (1023u32 << 18).to_le_bytes();
        let mut c = Cursor { data: &raw, pos: 0 };
        assert_eq!(quaternion(&mut c, 0).unwrap(), [0., 0., 1., 0.]);
        for encoding in [3, 4, 6, 7, 15] {
            assert!(quaternion(&mut Cursor { data: &[], pos: 0 }, encoding).is_err());
        }
    }
    #[test]
    fn quadratic_curve_uses_local_de_boor_with_repeated_knots() {
        let curve = Curve {
            knots: &[0, 0, 0, 2, 2, 2],
            degree: 2,
            controls: vec![[0.; 4], [4.; 4], [0.; 4]],
        };
        for (t, expected) in [
            (-1., 0.),
            (0., 0.),
            (0.5, 1.5),
            (1., 2.),
            (1.5, 1.5),
            (2., 0.),
            (3., 0.),
        ] {
            assert_eq!(curve.sample(t, &mut 1000).unwrap(), [expected; 4]);
        }
    }
}

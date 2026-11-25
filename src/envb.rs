//! Bounded reader for little-endian ENVB/ENVS v6 tables. An unknown track
//! retains its numeric ID and key offsets; a key offset does not imply a
//! known record size. The reader does not select weather or interpolate.

use std::fmt;

const MAX_FILE: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Header,
    Range,
    NonFinite,
    TrackType,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ENVB: {self:?}")
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy)]
pub struct Envb<'a> {
    bytes: &'a [u8],
    count: usize,
}

#[derive(Clone, Copy)]
pub struct Section<'a> {
    bytes: &'a [u8],
    tracks: usize,
    count: usize,
    pub owner_id: u32,
    pub cycle_seconds: f32,
    pub parameter: u32,
    pub weight: f32,
}

#[derive(Clone, Copy)]
pub struct Track<'a> {
    bytes: &'a [u8],
    offsets: usize,
    count: usize,
    pub id: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct ColorIntensity {
    pub rgba: [u8; 4],
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalLighting {
    pub seconds: f32,
    pub ambient_scale: f32,
    pub ambient_saturation: f32,
    pub ambient_attenuation: f32,
    pub extra_ambient_weight: f32,
    pub sunlight: ColorIntensity,
    pub extra_ambient: ColorIntensity,
    pub moonlight: ColorIntensity,
}

#[derive(Clone, Copy, Debug)]
pub struct Cloud {
    pub seconds: f32,
    pub primary: u32,
    pub secondary: u32,
    pub primary_intensity: f32,
    pub secondary_intensity: f32,
    pub diffuse: ColorIntensity,
    pub ambient: ColorIntensity,
}

#[derive(Clone, Copy, Debug)]
pub struct ToneMapping {
    pub seconds: f32,
    pub adaptation_rate: f32,
    /// Raw (W, X, Y) fields. These are not UE exposure values.
    pub adapted_luminance_wxy: [f32; 3],
    /// Raw (Y, X) fields.
    pub tone_map_yx: [f32; 2],
}

/// ENVS v6 track 12: three star layer weights at the key start.
#[derive(Clone, Copy, Debug)]
pub struct Starfield {
    pub seconds: f32,
    pub layer_intensity: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct HeightFog {
    pub seconds: f32,
    pub color: ColorIntensity,
    pub density: f32,
    pub height: f32,
    pub height_falloff: f32,
    pub start_distance: f32,
    pub second_density_percent: f32,
    pub second_height_delta: f32,
    pub second_height_falloff: f32,
    pub directional_color: ColorIntensity,
    pub directional_start_distance: f32,
    pub directional_intensity: f32,
    pub directional_exponent: f32,
    pub update_height_fog: bool,
}

fn range(bytes: &[u8], at: usize, size: usize) -> Result<&[u8], Error> {
    let end = at.checked_add(size).ok_or(Error::Range)?;
    bytes.get(at..end).ok_or(Error::Range)
}

fn word(bytes: &[u8], at: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(range(bytes, at, 4)?.try_into().unwrap()))
}

fn float(bytes: &[u8], at: usize) -> Result<f32, Error> {
    let f = f32::from_bits(word(bytes, at)?);
    if f.is_finite() {
        Ok(f)
    } else {
        Err(Error::NonFinite)
    }
}

fn table(bytes: &[u8], start: usize, count: usize, stride: usize) -> Result<(), Error> {
    range(bytes, start, count.checked_mul(stride).ok_or(Error::Range)?)?;
    Ok(())
}

fn color(bytes: &[u8], base: usize, offset: u32) -> Result<ColorIntensity, Error> {
    let raw = range(bytes, relative(base, offset)?, 8)?;
    Ok(ColorIntensity {
        rgba: raw[..4].try_into().unwrap(),
        intensity: float(raw, 4)?,
    })
}

fn relative(base: usize, bits: u32) -> Result<usize, Error> {
    base.checked_add_signed(bits as i32 as isize)
        .ok_or(Error::Range)
}

impl<'a> Envb<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        if !bytes.starts_with(b"ENVB")
            || bytes.len() > MAX_FILE
            || word(bytes, 4)? as usize != bytes.len()
        {
            return Err(Error::Header);
        }
        let count = word(bytes, 8)? as usize;
        table(bytes, 12, count, 24)?;
        Ok(Self { bytes, count })
    }

    pub fn environment_count(&self) -> usize {
        self.count
    }

    /// Environment headers are followed by section data addressed relative to
    /// the header's offset field. Values are checked when traversed.
    pub fn sections(
        &self,
        environment: usize,
    ) -> Result<impl ExactSizeIterator<Item = Result<Section<'a>, Error>>, Error> {
        if environment >= self.count {
            return Err(Error::Range);
        }
        let bytes = self.bytes;
        let at = 12 + environment * 24;
        if range(bytes, at, 4)? != b"ENVS"
            || word(bytes, at + 4)? != 24
            || word(bytes, at + 8)? != 6
        {
            return Err(Error::Header);
        }
        let start = relative(at + 8, word(bytes, at + 12)?)?;
        let count = word(bytes, at + 16)? as usize;
        table(bytes, start, count, 16)?;
        Ok((0..count).map(move |index| {
            let at = start + 16 * index;
            let tracks = relative(at, word(bytes, at)?)?;
            let count = word(bytes, at + 4)? as usize;
            let footer = relative(at, word(bytes, at + 12)?)?;
            table(bytes, tracks, count, 12)?;
            range(bytes, footer, 20)?;
            Ok(Section {
                bytes,
                tracks,
                count,
                owner_id: word(bytes, at + 8)?,
                cycle_seconds: float(bytes, footer)?,
                parameter: word(bytes, footer + 4)?,
                weight: float(bytes, footer + 8)?,
            })
        }))
    }
}

impl<'a> Section<'a> {
    pub fn tracks(&self) -> impl ExactSizeIterator<Item = Result<Track<'a>, Error>> + '_ {
        (0..self.count).map(|index| {
            let at = self.tracks + index * 12;
            let offsets = relative(at, word(self.bytes, at)?)?;
            let count = word(self.bytes, at + 4)? as usize;
            table(self.bytes, offsets, count, 4)?;
            Ok(Track {
                bytes: self.bytes,
                offsets,
                count,
                id: word(self.bytes, at + 8)?,
            })
        })
    }
}

impl Track<'_> {
    pub fn key_count(&self) -> usize {
        self.count
    }

    pub fn key_offset(&self, index: usize) -> Result<usize, Error> {
        if index >= self.count {
            return Err(Error::Range);
        }
        let at = relative(self.offsets, word(self.bytes, self.offsets + index * 4)?)?;
        range(self.bytes, at, 4)?;
        Ok(at)
    }

    pub fn global_lighting(&self, index: usize) -> Result<GlobalLighting, Error> {
        if self.id != 0 {
            return Err(Error::TrackType);
        }
        let at = self.key_offset(index)?;
        let b = range(self.bytes, at, 32)?;
        // These are offsets from the key start. The 007V marker and
        // extension at +44/+48 precede the referenced colors in the sample.
        let base = at;
        Ok(GlobalLighting {
            seconds: float(b, 0)?,
            ambient_scale: float(b, 8)?,
            ambient_saturation: float(b, 12)?,
            ambient_attenuation: float(b, 16)?,
            extra_ambient_weight: float(b, 28)?,
            sunlight: color(self.bytes, base, word(b, 4)?)?,
            extra_ambient: color(self.bytes, base, word(b, 20)?)?,
            moonlight: color(self.bytes, base, word(b, 24)?)?,
        })
    }

    pub fn cloud(&self, index: usize) -> Result<Cloud, Error> {
        if self.id != 2 {
            return Err(Error::TrackType);
        }
        let at = self.key_offset(index)?;
        let b = range(self.bytes, at, 28)?;
        Ok(Cloud {
            seconds: float(b, 0)?,
            primary: word(b, 4)?,
            secondary: word(b, 8)?,
            primary_intensity: float(b, 12)?,
            secondary_intensity: float(b, 16)?,
            diffuse: color(self.bytes, at, word(b, 20)?)?,
            ambient: color(self.bytes, at, word(b, 24)?)?,
        })
    }

    pub fn tone_mapping(&self, index: usize) -> Result<ToneMapping, Error> {
        if self.id != 9 {
            return Err(Error::TrackType);
        }
        let b = range(self.bytes, self.key_offset(index)?, 28)?;
        Ok(ToneMapping {
            seconds: float(b, 0)?,
            adaptation_rate: float(b, 4)?,
            adapted_luminance_wxy: [float(b, 8)?, float(b, 12)?, float(b, 16)?],
            tone_map_yx: [float(b, 20)?, float(b, 24)?],
        })
    }

    pub fn starfield(&self, index: usize) -> Result<Starfield, Error> {
        if self.id != 12 {
            return Err(Error::TrackType);
        }
        let b = range(self.bytes, self.key_offset(index)?, 16)?;
        Ok(Starfield {
            seconds: float(b, 0)?,
            layer_intensity: [float(b, 4)?, float(b, 8)?, float(b, 12)?],
        })
    }

    /// Read the 007V height-fog extension of track 13.
    pub fn height_fog(&self, index: usize) -> Result<HeightFog, Error> {
        if self.id != 13 {
            return Err(Error::TrackType);
        }
        let at = self.key_offset(index)?;
        let b = range(self.bytes, at, 88)?;
        if &b[32..36] != b"007V" {
            return Err(Error::Header);
        }
        Ok(HeightFog {
            seconds: float(b, 0)?,
            color: color(self.bytes, at, word(b, 4)?)?,
            density: float(b, 36)?,
            height: float(b, 40)?,
            height_falloff: float(b, 44)?,
            start_distance: float(b, 48)?,
            second_density_percent: float(b, 56)?,
            second_height_delta: float(b, 60)?,
            second_height_falloff: float(b, 64)?,
            directional_start_distance: float(b, 68)?,
            directional_intensity: float(b, 72)?,
            directional_exponent: float(b, 76)?,
            directional_color: color(self.bytes, at, word(b, 80)?)?,
            update_height_fog: word(b, 84)? != 0,
        })
    }
}

/// Bracket day time in a nondecreasing track without changing key order.
/// A track can begin after midnight; the last and first keys then bracket
/// midnight across the cycle boundary. Returned weight is linear only;
/// callers decide which fields interpolate and which are discrete.
pub fn periodic_key_span(times: &[f32], cycle: f32, seconds: f32) -> Option<(usize, usize, f32)> {
    if !cycle.is_finite()
        || cycle <= 0.
        || !seconds.is_finite()
        || times.is_empty()
        || times.iter().any(|&t| !(0.0..=cycle).contains(&t))
        || times.windows(2).any(|t| t[0] > t[1])
    {
        return None;
    }
    if times.len() == 1 {
        return Some((0, 0, 0.));
    }
    let t = f64::from(seconds).rem_euclid(f64::from(cycle));
    let n = times.partition_point(|&time| f64::from(time) <= t);
    let lo = if n == 0 { times.len() - 1 } else { n - 1 };
    let hi = n % times.len();
    let a = f64::from(times[lo]) - if n == 0 { f64::from(cycle) } else { 0. };
    let b = f64::from(times[hi])
        + if n == times.len() {
            f64::from(cycle)
        } else {
            0.
        };
    Some((
        lo,
        hi,
        if b > a {
            ((t - a) / (b - a)) as f32
        } else {
            0.
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &mut [u8], at: usize, v: u32) {
        bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; 184];
        bytes[..4].copy_from_slice(b"ENVB");
        bytes[12..16].copy_from_slice(b"ENVS");
        for (at, val) in [
            (4, 184),
            (8, 1),
            (16, 24),
            (20, 6),
            (24, 16),
            (28, 1),
            (36, 16),
            (40, 2),
            (44, 7),
            (48, 128), // section / footer
            (52, 24),
            (56, 1),
            (60, 0),
            (64, 16),
            (68, 1),
            (72, 900), // tracks
            (76, 8),
            (80, 82), // offsets relative to each key directory
            (84, 43_200_f32.to_bits()),
            (88, 52),
            (92, 0.8_f32.to_bits()),
            (104, 60),
            (108, 68),
            (140, 1_f32.to_bits()),
            (148, 2_f32.to_bits()),
            (156, 3_f32.to_bits()),
        ] {
            put(&mut bytes, at, val);
        }
        bytes[128..132].copy_from_slice(b"007V");
        bytes[136..140].copy_from_slice(&[10, 20, 30, 255]);
        bytes.resize(188, 0); // footer extends to byte 184, unknown record at 162.
        put(&mut bytes, 4, 188);
        put(&mut bytes, 164, 86_400_f32.to_bits());
        put(&mut bytes, 168, 45);
        put(&mut bytes, 172, 1_f32.to_bits());
        bytes
    }

    #[test]
    fn follows_distinct_offset_bases_without_treating_unknown_tracks_as_lighting() {
        let data = fixture();
        let env = Envb::parse(&data).unwrap();
        let section = env.sections(0).unwrap().next().unwrap().unwrap();
        assert_eq!((section.owner_id, section.parameter), (7, 45));
        let mut tracks = section.tracks();
        let lighting = tracks.next().unwrap().unwrap().global_lighting(0).unwrap();
        assert_eq!(lighting.seconds, 43_200.);
        assert_eq!(lighting.sunlight.rgba, [10, 20, 30, 255]);
        assert_eq!(lighting.extra_ambient.intensity, 2.);
        assert_eq!(lighting.moonlight.intensity, 3.);
        let unknown = tracks.next().unwrap().unwrap();
        assert_eq!((unknown.id, unknown.key_offset(0)), (900, Ok(162)));
        assert!(matches!(unknown.global_lighting(0), Err(Error::TrackType)));
    }

    #[test]
    fn reads_star_layer_intensities_without_consuming_the_moon_record() {
        let mut data = fixture();
        put(&mut data, 60, 12);
        for (offset, value) in [(88, 2.5_f32), (92, 5.0_f32), (96, 1.3_f32)] {
            put(&mut data, offset, value.to_bits());
        }
        let env = Envb::parse(&data).unwrap();
        let section = env.sections(0).unwrap().next().unwrap().unwrap();
        let track = section.tracks().next().unwrap().unwrap();
        let stars = track.starfield(0).unwrap();
        assert_eq!(stars.seconds, 43_200.);
        assert_eq!(stars.layer_intensity, [2.5, 5., 1.3]);
        assert!(matches!(track.global_lighting(0), Err(Error::TrackType)));
    }

    #[test]
    fn rejects_truncated_files_and_out_of_range_keys() {
        let mut data = fixture();
        assert!(matches!(Envb::parse(&data[..160]), Err(Error::Header)));
        put(&mut data, 76, u32::MAX >> 1);
        let env = Envb::parse(&data).unwrap();
        let section = env.sections(0).unwrap().next().unwrap().unwrap();
        let track = section.tracks().next().unwrap().unwrap();
        assert_eq!(track.key_offset(0), Err(Error::Range));
    }

    #[test]
    fn brackets_keys_across_midnight_and_skips_earlier_duplicate_on_exact_time() {
        assert_eq!(periodic_key_span(&[10., 20.], 30., 0.), Some((1, 0, 0.5)));
        assert_eq!(
            periodic_key_span(&[10., 10., 20.], 30., 10.),
            Some((1, 2, 0.))
        );
        assert_eq!(periodic_key_span(&[20., 10.], 30., 0.), None);
    }
}

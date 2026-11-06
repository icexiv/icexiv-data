//! Win32 PAP 0x00020001 animation records and Havok binding relationships.
//! Timeline bytes are retained; no timeline execution or character assembly.
use crate::{
    binary::{u16_at, u32_at},
    havok::{self, Tagfile, Value},
};
pub type Result<T> = std::result::Result<T, havok::Error>;
#[derive(Clone, Copy, Debug)]
pub struct Record<'a> {
    pub name: &'a [u8],
    pub havok_index: u16,
    pub raw: &'a [u8],
}
#[derive(Debug)]
pub struct Pap<'a> {
    pub header: &'a [u8],
    pub skeleton_id: u16,
    records: &'a [u8],
    /// Absent for a zero-record package with only container alignment bytes.
    pub tagfile: Option<Tagfile<'a>>,
    /// Zero alignment bytes between the Havok stream end and timeline.
    pub havok_padding: &'a [u8],
    pub timeline: &'a [u8],
}
#[derive(Clone, Copy, Debug)]
pub struct Binding<'a> {
    pub object_id: usize,
    pub animation_object_id: usize,
    pub original_skeleton_name: Option<&'a [u8]>,
    pub transform_track_to_bone: &'a [i64],
    pub float_track_to_slot: &'a [i64],
    pub blend_hint: i64,
}
fn refs<'b>(o: &'b havok::Object<'_>, name: &[u8]) -> Result<&'b [Value<'b>]> {
    match o.field(name) {
        Some(Value::Array(a)) => Ok(a),
        _ => Err("animation container reference array".into()),
    }
}
fn reference(v: &Value<'_>) -> Result<usize> {
    match v {
        Value::Reference(id) if *id != 0 => Ok(*id),
        _ => Err("animation null/reference type".into()),
    }
}
impl<'a> Pap<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, havok::Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: havok::Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(havok::Error::BudgetExceeded("PAP input"));
        }
        let header = bytes.get(..26).ok_or("PAP header")?;
        if &header[..4] != b"pap " {
            return Err("PAP magic".into());
        }
        let version = u32_at(bytes, 4)?;
        if version != 0x00020001 {
            return Err(havok::Error::UnsupportedVersion(i64::from(version)));
        }
        let count = usize::from(u16_at(bytes, 8)?);
        let start = usize::try_from(u32_at(bytes, 14)?).map_err(|_| "PAP offset overflow")?;
        let havok = usize::try_from(u32_at(bytes, 18)?).map_err(|_| "PAP offset overflow")?;
        let timeline = usize::try_from(u32_at(bytes, 22)?).map_err(|_| "PAP offset overflow")?;
        let end = start
            .checked_add(count.checked_mul(40).ok_or("PAP record size")?)
            .ok_or("PAP record span")?;
        if start < 26 || end > havok || havok > timeline {
            return Err("PAP section order".into());
        }
        let records = bytes.get(start..end).ok_or("PAP records")?;
        let havok_bytes = bytes.get(havok..timeline).ok_or("PAP Havok range")?;
        let (tagfile, consumed) =
            if count == 0 && havok_bytes.len() <= 15 && havok_bytes.iter().all(|b| *b == 0) {
                (None, 0)
            } else {
                let (graph, consumed) = Tagfile::parse_prefix_with_limits(havok_bytes, limits)?;
                (Some(graph), consumed)
            };
        let havok_padding = &havok_bytes[consumed..];
        if !havok_padding.is_empty()
            && (havok_padding.len() > 15
                || timeline % 16 != 0
                || havok_padding.iter().any(|b| *b != 0))
        {
            return Err("PAP Havok alignment padding".into());
        }
        let file = Self {
            header,
            skeleton_id: u16_at(bytes, 10)?,
            records,
            tagfile,
            havok_padding,
            timeline: bytes.get(timeline..).ok_or("PAP timeline")?,
        };
        let mut work = limits.max_validation_steps;
        for record in file.records() {
            file.binding_with_work(record.havok_index, &mut work)?;
        }
        Ok(file)
    }
    pub fn records(&self) -> impl ExactSizeIterator<Item = Record<'a>> + 'a {
        self.records.chunks_exact(40).map(|raw| {
            let name = &raw[..32];
            Record {
                name: &name[..name.iter().position(|v| *v == 0).unwrap_or(32)],
                havok_index: u16::from_le_bytes([raw[34], raw[35]]),
                raw,
            }
        })
    }
    /// Container array order, not remembered-object stream order.
    pub fn binding(&self, index: u16) -> Result<Binding<'_>> {
        self.binding_with_work(index, &mut havok::Limits::default().max_validation_steps)
    }
    fn binding_with_work(&self, index: u16, work: &mut usize) -> Result<Binding<'_>> {
        let tagfile = self.tagfile.as_ref().ok_or("PAP has no Havok stream")?;
        *work = work
            .checked_sub(tagfile.objects.len())
            .ok_or(havok::Error::BudgetExceeded("PAP container lookup"))?;
        let mut containers = tagfile.objects.iter().filter(|o| {
            tagfile
                .class(o.class_index)
                .is_ok_and(|c| c.name == b"hkaAnimationContainer")
        });
        let container = containers.next().ok_or("missing animation container")?;
        if containers.next().is_some() {
            return Err("ambiguous animation containers".into());
        }
        let animations = refs(container, b"animations")?;
        let bindings = refs(container, b"bindings")?;
        if animations.len() != bindings.len() {
            return Err("animation/binding counts".into());
        }
        let index = usize::from(index);
        let animation = reference(animations.get(index).ok_or("PAP animation index")?)?;
        let id = reference(bindings.get(index).ok_or("PAP binding index")?)?;
        let o = tagfile.object(id)?.ok_or("null binding")?;
        let class = tagfile.class(o.class_index)?;
        if class.name != b"hkaAnimationBinding" || class.version != 1 {
            return Err("binding class/version".into());
        }
        if reference(o.field(b"animation").ok_or("binding animation")?)? != animation {
            return Err("binding animation mismatch".into());
        }
        let target = tagfile.object(animation)?.ok_or("null binding animation")?;
        let original_skeleton_name = match o.field(b"originalSkeletonName") {
            None => None,
            Some(Value::String(v)) => *v,
            _ => return Err("binding skeleton name".into()),
        };
        let indices = |name: &[u8]| -> Result<&[i64]> {
            match o.field(name) {
                None => Ok(&[]),
                Some(Value::Integers { values, .. }) => Ok(values),
                _ => Err("binding track map".into()),
            }
        };
        let transform_track_to_bone = indices(b"transformTrackToBoneIndices")?;
        let float_track_to_slot = indices(b"floatTrackToFloatSlotIndices")?;
        let cost = transform_track_to_bone
            .len()
            .checked_add(float_track_to_slot.len())
            .ok_or("PAP map work overflow")?;
        *work = work
            .checked_sub(cost)
            .ok_or(havok::Error::BudgetExceeded("PAP binding indices"))?;
        if let Some(Value::Integer(n)) = target.field(b"numberOfTransformTracks") {
            if !transform_track_to_bone.is_empty()
                && usize::try_from(*n).ok() != Some(transform_track_to_bone.len())
            {
                return Err("binding transform map length".into());
            }
        }
        if transform_track_to_bone
            .iter()
            .chain(float_track_to_slot)
            .any(|v| *v < -1 || *v > i64::from(i16::MAX))
        {
            return Err("binding track index range".into());
        }
        let blend_hint = match o.field(b"blendHint") {
            None => 0,
            Some(Value::Integer(v)) => *v,
            _ => return Err("binding blend hint".into()),
        };
        Ok(Binding {
            object_id: id,
            animation_object_id: animation,
            original_skeleton_name,
            transform_track_to_bone,
            float_track_to_slot,
            blend_hint,
        })
    }
}

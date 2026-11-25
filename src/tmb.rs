//! Win32 TMLB envelopes in standalone action TMB files and PAP timeline sections.
use crate::havok::Result;
use crate::{
    binary::{slice, u16_at, u32_at},
    havok::{Error, reserve, spend},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Timeline,
    Animation,
    Effect,
    Sound,
}

/// A directly encoded command reference. Times are raw timeline units;
/// actors/tracks, loops and activation conditions belong to the player.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandResource<'a> {
    pub id: u16,
    pub start: i16,
    /// Encoded duration (loop/duration for C063).
    pub duration: i32,
    pub kind: ResourceKind,
    pub name: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindTarget {
    pub origin: i8,
    pub kind: i8,
    pub id: i16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectCommand<'a> {
    pub resource: CommandResource<'a>,
    pub targets: [BindTarget; 2],
    pub scale: [f32; 3],
    pub rotation_radians: [f32; 3],
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub visibility: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandLink {
    /// Indices in Timeline::entries, not serialized IDs.
    pub actor: usize,
    pub track: usize,
    pub command: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimelineActor {
    pub id: u16,
    pub start: i16,
    pub ability_delay: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct TimelineTrack<'a> {
    pub id: u16,
    pub start: i16,
    /// None: no condition pointer; Some(empty): encoded zero-entry condition.
    condition_bytes: Option<&'a [u8]>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackCondition {
    pub operation: u32,
    pub value: u32,
    pub float_value: f32,
}
impl<'a> TimelineTrack<'a> {
    pub fn conditions(self) -> Option<impl ExactSizeIterator<Item = TrackCondition> + 'a> {
        self.condition_bytes.map(|b| {
            b.chunks_exact(12).map(|r| {
                let word = |i| u32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
                TrackCondition {
                    operation: word(0),
                    value: word(4),
                    float_value: f32::from_bits(word(8)),
                }
            })
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TimelineLimits {
    pub max_input_bytes: usize,
    pub max_timelines: usize,
    /// Cumulative across the whole PAP sequence.
    pub max_entries: usize,
    pub max_output_bytes: usize,
}

impl Default for TimelineLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 << 20,
            max_timelines: 65536,
            max_entries: 1_000_000,
            max_output_bytes: 64 << 20,
        }
    }
}

#[derive(Debug)]
pub struct Timeline<'a> {
    /// Byte offset relative to the start of the PAP timeline section.
    pub offset: usize,
    pub entries: Vec<TimelineEntry<'a>>,
    /// Out-of-line storage following the counted entries.
    pub tail: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct TimelineEntry<'a> {
    /// Entry header position relative to the enclosing TMLB start.
    pub offset: usize,
    pub tag: [u8; 4],
    pub payload: &'a [u8],
}

impl<'a> Timeline<'a> {
    pub fn actor(&self, index: usize) -> Result<TimelineActor> {
        let e = self.entries.get(index).ok_or("TMLB entry index")?;
        if e.tag != *b"TMAC" || e.payload.len() != 20 {
            return Err("TMAC layout".into());
        }
        Ok(TimelineActor {
            id: u16_at(e.payload, 0)?,
            start: u16_at(e.payload, 2)? as i16,
            ability_delay: u32_at(e.payload, 4)? as i32,
        })
    }
    pub fn track(&self, index: usize) -> Result<TimelineTrack<'a>> {
        let e = self.entries.get(index).ok_or("TMLB entry index")?;
        if e.tag != *b"TMTR" || e.payload.len() != 16 {
            return Err("TMTR layout".into());
        }
        let conditions = if u32_at(e.payload, 12)? == 0 {
            None
        } else {
            let (_, at) = self.location(index, 12)?;
            let b = self.tail.get(at..).ok_or("TMTR condition offset")?;
            if u32_at(b, 0)? != 8 {
                return Err("TMTR condition header".into());
            }
            let count = u32_at(b, 4)? as usize;
            Some(slice(
                b,
                8,
                count.checked_mul(12).ok_or("TMTR condition count")?,
            )?)
        };
        Ok(TimelineTrack {
            id: u16_at(e.payload, 0)?,
            start: u16_at(e.payload, 2)? as i16,
            condition_bytes: conditions,
        })
    }
    /// Reachable commands in TMAL/actor/track/list order. Orphan tracks are
    /// not scheduled. Duplicated references retain multiplicity. No assumption
    /// that a record ID equals its index; unresolved IDs are errors.
    pub fn command_links(
        &self,
        max_output_bytes: usize,
        max_work: usize,
    ) -> Result<Vec<CommandLink>> {
        let mut work = max_work;
        let mut bytes = max_output_bytes;
        let mut ids = reserve(self.entries.len(), &mut bytes)?;
        let mut root = None;
        for (i, e) in self.entries.iter().enumerate() {
            spend(&mut work, 1, "TMLB link inventory")?;
            if e.tag == *b"TMAL" {
                if root.replace(i).is_some() {
                    return Err("multiple TMAL lists".into());
                }
            } else if e.tag == *b"TMAC"
                || e.tag == *b"TMTR"
                || e.tag[0] == b'C' && e.tag[1..].iter().all(u8::is_ascii_digit)
            {
                ids.push((u16_at(e.payload, 0)?, i));
            }
        }
        let lookup_cost = (usize::BITS - ids.len().leading_zeros()) as usize + 1;
        spend(
            &mut work,
            ids.len().checked_mul(lookup_cost).ok_or("TMLB sort work")?,
            "TMLB ID sort",
        )?;
        ids.sort_unstable();
        if ids.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("duplicate TMLB ID".into());
        }
        let find = |id, work: &mut usize| -> Result<usize> {
            spend(work, lookup_cost, "TMLB ID lookup")?;
            ids.binary_search_by_key(&id, |&(x, _)| x)
                .map(|p| ids[p].1)
                .map_err(|_| "unresolved TMLB ID".into())
        };
        let links = |i| {
            self.linked_ids(i)?
                .ok_or(Error::InvalidData("TMLB link owner"))
        };
        // Count before allocating; traversal work is bounded on both passes.
        let mut output = Vec::new();
        let mut count = 0usize;
        let root = root.ok_or("missing TMAL list")?;
        for emit in [false, true] {
            if emit {
                output = reserve(count, &mut bytes)?;
            }
            for id in links(root)? {
                let actor = find(id, &mut work)?;
                if self.entries[actor].tag != *b"TMAC" {
                    return Err("TMLB actor link".into());
                }
                for id in links(actor)? {
                    let track = find(id, &mut work)?;
                    if self.entries[track].tag != *b"TMTR" {
                        return Err("TMLB track link".into());
                    }
                    for id in links(track)? {
                        let command = find(id, &mut work)?;
                        if self.entries[command].tag[0] != b'C' {
                            return Err("TMLB command link".into());
                        }
                        if emit {
                            output.push(CommandLink {
                                actor,
                                track,
                                command,
                            });
                        } else {
                            count = count.checked_add(1).ok_or("TMLB link count")?;
                        }
                    }
                }
            }
        }
        Ok(output)
    }

    /// Direct C012 effect inputs. Offsets start at the entry payload; vector
    /// counts and bounds are validated before reading any components.
    pub fn effect_command(&self, index: usize) -> Result<Option<EffectCommand<'a>>> {
        let entry = self.entries.get(index).ok_or("TMLB entry index")?;
        if entry.tag != *b"C012" {
            return Ok(None);
        }
        let resource = self
            .command_resource(index)?
            .ok_or("missing effect command")?;
        let target = |at: usize| -> Result<BindTarget> {
            Ok(BindTarget {
                origin: entry.payload[at] as i8,
                kind: entry.payload[at + 1] as i8,
                id: u16_at(entry.payload, at + 2)? as i16,
            })
        };
        fn vector<const N: usize>(t: &Timeline<'_>, i: usize, at: usize) -> Result<[f32; N]> {
            let (e, loc) = t.location(i, at)?;
            if u32_at(e.payload, at + 4)? as usize != N {
                return Err("TMLB vector count".into());
            }
            let b = slice(t.tail, loc, N * 4)?;
            let mut v = [0.; N];
            for (j, x) in v.iter_mut().enumerate() {
                *x = f32::from_bits(u32_at(b, j * 4)?);
            }
            Ok(v)
        }
        Ok(Some(EffectCommand {
            resource,
            targets: [target(16)?, target(20)?],
            scale: vector(self, index, 24)?,
            rotation_radians: vector(self, index, 32)?,
            position: vector(self, index, 40)?,
            color: vector(self, index, 48)?,
            visibility: u32_at(entry.payload, 56)?,
        }))
    }
    /// References read from known command layouts in PAP and standalone TMB.
    /// An unknown opcode is not classified by searching its bytes for paths.
    pub fn command_resource(&self, index: usize) -> Result<Option<CommandResource<'a>>> {
        let entry = self.entries.get(index).ok_or("TMLB entry index")?;
        let (kind, len, pointer) = match &entry.tag {
            b"C002" => (ResourceKind::Timeline, 20, 16),
            b"C009" => (ResourceKind::Animation, 16, 12),
            b"C010" => (ResourceKind::Animation, 32, 24),
            b"C012" => (ResourceKind::Effect, 64, 12),
            b"C063" => (ResourceKind::Sound, 24, 12),
            _ => return Ok(None),
        };
        if entry.payload.len() != len {
            return Err("TMLB resource command layout".into());
        }
        let (_, at) = self.location(index, pointer)?;
        let name = self.tail.get(at..).ok_or("TMLB command name offset")?;
        let end = name
            .iter()
            .position(|v| *v == 0)
            .ok_or("TMLB unterminated command name")?;
        Ok(Some(CommandResource {
            id: u16_at(entry.payload, 0)?,
            start: u16_at(entry.payload, 2)? as i16,
            duration: u32_at(entry.payload, 4)? as i32,
            kind,
            name: &name[..end],
        }))
    }
    fn location(&self, index: usize, field: usize) -> Result<(&TimelineEntry<'a>, usize)> {
        let entry = self.entries.get(index).ok_or("TMLB entry index")?;
        let relative = u32_at(entry.payload, field)? as usize;
        let target = entry
            .offset
            .checked_add(8)
            .and_then(|v| v.checked_add(relative))
            .ok_or("TMLB relative offset")?;
        let last = self.entries.last().ok_or("TMLB empty")?;
        let tail_start = last
            .offset
            .checked_add(8)
            .and_then(|v| v.checked_add(last.payload.len()))
            .ok_or("TMLB tail offset")?;
        Ok((
            entry,
            target
                .checked_sub(tail_start)
                .ok_or("TMLB offset before tail")?,
        ))
    }

    /// TMAL, TMAC and TMTR lists contain little-endian u16 IDs. An empty
    /// list is distinct from an entry without this kind of field.
    pub fn linked_ids(
        &self,
        index: usize,
    ) -> Result<Option<impl ExactSizeIterator<Item = u16> + 'a>> {
        let entry = self.entries.get(index).ok_or("TMLB entry index")?;
        let field = match (&entry.tag, entry.payload.len()) {
            (b"TMAL", 8) => 0,
            (b"TMAC", 20) => 12,
            (b"TMTR", 16) => 4,
            (b"TMAL" | b"TMAC" | b"TMTR", _) => return Err("TMLB link layout".into()),
            _ => return Ok(None),
        };
        let count = u32_at(entry.payload, field + 4)? as usize;
        let bytes = if count == 0 {
            &[]
        } else {
            let (_, at) = self.location(index, field)?;
            slice(
                self.tail,
                at,
                count.checked_mul(2).ok_or("TMLB link count")?,
            )?
        };
        Ok(Some(
            bytes
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]])),
        ))
    }

    /// Name referenced by the 24-byte C009 record, without the NUL.
    pub fn c009_name(&self, index: usize) -> Result<Option<&'a [u8]>> {
        let entry = self.entries.get(index).ok_or("TMLB entry index")?;
        if entry.tag != *b"C009" {
            return Ok(None);
        }
        Ok(self.command_resource(index)?.map(|c| c.name))
    }

    /// Parse counted TMLB envelopes in a four-byte-aligned PAP section.
    pub fn parse_sequence(data: &'a [u8], limits: TimelineLimits) -> Result<Vec<Self>> {
        if data.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("TMLB input"));
        }
        // First count bounded envelopes so all output allocations are charged
        // at their exact requested capacity, without reserving from a hint.
        fn envelope(data: &[u8], start: usize) -> Result<(&[u8], usize)> {
            let bytes = &data[start..];
            if bytes.get(..4) != Some(b"TMLB") {
                return Err("TMLB magic".into());
            }
            let len = u32_at(bytes, 4)? as usize;
            if len < 12 {
                return Err("TMLB size".into());
            }
            let block = slice(bytes, 0, len)?;
            let end = start + len;
            let pad = if end == data.len() {
                0
            } else {
                end.wrapping_neg() % 4
            };
            if !slice(data, end, pad)?.iter().all(|&v| v == 0) {
                return Err("TMLB alignment".into());
            }
            Ok((block, end + pad))
        }
        let mut pos = 0;
        let mut count = 0;
        while pos < data.len() {
            if count == limits.max_timelines {
                return Err(Error::BudgetExceeded("TMLB count"));
            }
            pos = envelope(data, pos)?.1;
            count += 1;
        }
        let mut bytes = limits.max_output_bytes;
        let mut remaining = limits.max_entries;
        let mut out = reserve(count, &mut bytes)?;
        pos = 0;
        while pos < data.len() {
            let (block, next) = envelope(data, pos)?;
            let n = u32_at(block, 8)? as usize;
            if n > (block.len() - 12) / 8 {
                return Err("TMLB entry count".into());
            }
            spend(&mut remaining, n, "TMLB entries")?;
            let mut entries = reserve(n, &mut bytes)?;
            let mut at = 12;
            for _ in 0..n {
                let hdr = slice(block, at, 8)?;
                let len = u32_at(hdr, 4)? as usize;
                let raw = slice(block, at, len)?;
                entries.push(TimelineEntry {
                    offset: at,
                    tag: hdr[..4].try_into().unwrap(),
                    payload: raw.get(8..).ok_or("TMLB entry size")?,
                });
                at += len;
            }
            out.push(Self {
                offset: pos,
                entries,
                tail: &block[at..],
            });
            pos = next;
        }
        Ok(out)
    }
}

//! Byte-only type-3 metadata and logical MDL header reconstruction.

type Result<T> = std::result::Result<T, &'static str>;
pub(super) const PREFIX: usize = 208;
pub(super) const MDL_HEADER: usize = 68;

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn half(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Segment {
    pub capacity: u32,
    pub stored_size: u32,
    pub offset: u32,
    pub first_block: usize,
    pub blocks: usize,
}

pub(super) struct Header {
    pub version: u32,
    pub declarations: u16,
    pub materials: u16,
    pub lods: u8,
    pub streaming: u8,
    pub edge: u8,
    pub segments: [Segment; 11],
    pub span_count: usize,
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < PREFIX {
            return Err("truncated model entry header");
        }
        let mut segments = [Segment::default(); 11];
        let mut span_count = 0;
        for (index, segment) in segments.iter_mut().enumerate() {
            *segment = Segment {
                capacity: word(bytes, 24 + index * 4),
                stored_size: word(bytes, 68 + index * 4),
                offset: word(bytes, 112 + index * 4),
                first_block: usize::from(half(bytes, 156 + index * 2)),
                blocks: usize::from(half(bytes, 178 + index * 2)),
            };
            if segment.blocks == 0 {
                if segment.capacity != 0 || segment.stored_size != 0 {
                    return Err("empty model segment has nonzero size");
                }
            } else {
                if segment.capacity == 0 || !segment.capacity.is_multiple_of(128) {
                    return Err("invalid model segment capacity");
                }
                span_count += segment.blocks;
            }
        }
        // A disjoint cover of the span table: no duplicate or orphan blocks.
        // Check only eleven ranges, without a file-sized bitmap allocation.
        for (i, a) in segments.iter().enumerate().filter(|(_, s)| s.blocks != 0) {
            if a.first_block + a.blocks > span_count {
                return Err("model block range exceeds span table");
            }
            for b in segments.iter().skip(i + 1).filter(|s| s.blocks != 0) {
                if a.first_block < b.first_block + b.blocks
                    && b.first_block < a.first_block + a.blocks
                {
                    return Err("overlapping model block ranges");
                }
                let a_end = u64::from(a.offset) + u64::from(a.stored_size);
                let b_end = u64::from(b.offset) + u64::from(b.stored_size);
                if u64::from(a.offset) < b_end && u64::from(b.offset) < a_end {
                    return Err("overlapping stored model segments");
                }
            }
        }
        let lods = bytes[204];
        if !(1..=3).contains(&lods) || bytes[205] > 1 || bytes[206] > 1 {
            return Err("invalid model LOD count or boolean flag");
        }
        Ok(Self {
            version: word(bytes, 20),
            declarations: half(bytes, 200),
            materials: half(bytes, 202),
            lods,
            streaming: bytes[205],
            edge: bytes[206],
            segments,
            span_count,
        })
    }

    pub fn validate_spans(&self, spans: &[u8]) -> Result<()> {
        if spans.len() != self.span_count * 2 {
            return Err("truncated model block spans");
        }
        for segment in &self.segments {
            if segment.blocks == 0 {
                continue;
            }
            let mut total = 0u64;
            for block in segment.first_block..segment.first_block + segment.blocks {
                let span = half(spans, block * 2);
                if span < 16 {
                    return Err("model block span too short");
                }
                total += u64::from(span);
            }
            if total != u64::from(segment.stored_size) {
                return Err("model segment stored size disagrees with block spans");
            }
        }
        Ok(())
    }

    /// Order in the logical file, unlike the eleven parallel metadata arrays.
    pub fn logical_order(&self) -> [usize; 11] {
        [0, 1, 2, 5, 8, 3, 6, 9, 4, 7, 10]
    }

    pub fn write_mdl_header(&self, output: &mut [u8], sizes: &[u32; 11]) -> Result<()> {
        let mut header = [0u8; MDL_HEADER];
        let mut put = |offset: usize, value: u32| {
            header[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(0, self.version);
        put(4, sizes[0]);
        put(8, sizes[1]);
        let mut cursor = MDL_HEADER as u32;
        for index in self.logical_order() {
            if (2..5).contains(&index) {
                let lod = index - 2;
                put(16 + lod * 4, if sizes[index] == 0 { 0 } else { cursor });
                put(40 + lod * 4, sizes[index]);
            } else if index >= 8 {
                let lod = index - 8;
                put(28 + lod * 4, if sizes[index] == 0 { 0 } else { cursor });
                put(52 + lod * 4, sizes[index]);
            }
            cursor = cursor
                .checked_add(sizes[index])
                .ok_or("MDL size overflow")?;
        }
        if cursor as usize != output.len() {
            return Err("decoded model size disagrees with entry size");
        }
        header[12..14].copy_from_slice(&self.declarations.to_le_bytes());
        header[14..16].copy_from_slice(&self.materials.to_le_bytes());
        header[64..67].copy_from_slice(&[self.lods, self.streaming, self.edge]);
        output
            .get_mut(..MDL_HEADER)
            .ok_or("truncated MDL output")?
            .copy_from_slice(&header);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_arbitrary_model_metadata_never_panics() {
        for length in 0..PREFIX {
            assert!(Header::parse(&vec![0; length]).is_err());
        }
        let mut state = 0x37u32;
        for _ in 0..4096 {
            let mut bytes = [0; PREFIX];
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            let _ = Header::parse(&bytes);
        }
    }
}

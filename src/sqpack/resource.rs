//! Resource lookup orchestrates byte acquisition; format.rs owns binary parsing.

use super::index::{self, Index, IndexHash, IndexKind};
use super::{Error, PackReader, PackSource, SqPack, format, is_valid_file_name};
use std::sync::Arc;

struct ResourcePath {
    path: String,
    repository: String,
    repository_id: u8,
    category: u8,
    hash: u64,
    hash2: u32,
}

impl ResourcePath {
    fn parse(path: &str) -> Option<Self> {
        if !is_valid_file_name(path) {
            return None;
        }
        let path = path.to_ascii_lowercase();
        let (directory, filename) = path.rsplit_once('/')?;
        let mut components = path.split('/');
        let category = match components.next()? {
            "common" => 0,
            "bgcommon" => 1,
            "bg" => 2,
            "cut" => 3,
            "chara" => 4,
            "shader" => 5,
            "ui" => 6,
            "sound" => 7,
            "vfx" => 8,
            "ui_script" => 9,
            "exd" => 10,
            "game_script" => 11,
            "music" => 12,
            "sqpack_test" => 0x12,
            "debug" => 0x13,
            _ => return None,
        };
        let second = components.next()?;
        let repository_id = match second.strip_prefix("ex") {
            Some(id) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => {
                let id = id.parse::<u8>().ok()?;
                if id == 0 || components.next().is_none() {
                    return None;
                }
                id
            }
            _ => 0,
        };
        let repository = if repository_id == 0 {
            "ffxiv".to_owned()
        } else {
            format!("ex{repository_id}")
        };
        let hash = (u64::from(format::hash(directory.as_bytes())) << 32)
            | u64::from(format::hash(filename.as_bytes()));
        let hash2 = format::hash(path.as_bytes());
        Some(Self {
            path,
            repository,
            repository_id,
            category,
            hash,
            hash2,
        })
    }

    fn stem(&self, chunk: u8) -> String {
        format!(
            "{}/{:02x}{:02x}{chunk:02x}.win32",
            self.repository, self.category, self.repository_id
        )
    }
}

fn invalid<E>(file: &str, reason: &'static str) -> Error<E> {
    Error::InvalidData {
        file: file.to_owned(),
        reason,
    }
}

impl<S: PackSource> SqPack<S> {
    /// Resolve an ASCII game resource path (case-insensitive) to extracted bytes.
    ///
    /// Supports Win32 index/index2, type-2 standard entries and type-4 texture
    /// entries, including raw and raw-DEFLATE blocks. Model and synonym/collision
    /// entries are not supported yet and return errors, not absence or guessed bytes.
    ///
    /// In the absence of a manifest or listing API, probes all 256 chunk names
    /// for the path's category/repository until found. Prefers index, using index2
    /// only when index is confirmed absent. Missing paths can therefore require
    /// 512 opens. Does not verify container SHA-1 checksums. Index tables are
    /// re-read on every call unless [`SqPack::with_index_cache`] is enabled.
    pub async fn read(&self, path: &str) -> Result<Vec<u8>, Error<S::Error>> {
        let path =
            ResourcePath::parse(path).ok_or_else(|| Error::InvalidResourcePath(path.to_owned()))?;
        for chunk in 0..=u8::MAX {
            let stem = path.stem(chunk);
            let index_name = format!("{stem}.index");
            let (index, index_name, index2) = match self.try_open(&index_name).await? {
                Some(index) => (index, index_name, false),
                None => {
                    let index_name = format!("{stem}.index2");
                    let Some(index) = self.try_open(&index_name).await? else {
                        continue;
                    };
                    (index, index_name, true)
                }
            };
            let kind = if index2 {
                IndexKind::Index2
            } else {
                IndexKind::Index
            };
            let index = self.load_index(&index, &index_name, kind).await?;
            let hash = if index2 {
                IndexHash::Index2(path.hash2)
            } else {
                IndexHash::Index {
                    directory: (path.hash >> 32) as u32,
                    filename: path.hash as u32,
                }
            };
            let Some(location) =
                index
                    .find_entry(hash)
                    .map_err(|reason| Error::UnsupportedIndexEntry {
                        file: index_name.clone(),
                        reason,
                    })?
            else {
                continue;
            };
            let dat_name = format!("{stem}.dat{}", location.dat);
            let dat = self.open(&dat_name).await?;
            return self.extract(&dat, &dat_name, location.offset).await;
        }
        Err(Error::MissingResource(path.path))
    }

    async fn load_index(
        &self,
        reader: &PackReader<S::File>,
        name: &str,
        kind: IndexKind,
    ) -> Result<Arc<Index<Vec<u8>>>, Error<S::Error>> {
        if let Some(index) = self.cached_index(name) {
            return Ok(index);
        }
        let index = Arc::new(index::load(reader, name, kind).await?);
        self.store_index(name.to_owned(), index.clone());
        Ok(index)
    }

    async fn extract(
        &self,
        dat: &PackReader<S::File>,
        name: &str,
        offset: u64,
    ) -> Result<Vec<u8>, Error<S::Error>> {
        let magic = dat.read_at(0, 8).await?;
        let data_start = if magic == b"SqPack\0\0" {
            let prefix = dat.read_at(0, 24).await?;
            let header_offset =
                format::sqpack_header(&prefix, dat.len(), 1).map_err(|e| invalid(name, e))?;
            let header = dat.read_at(header_offset, 4).await?;
            format::data_start(&header, header_offset, dat.len()).map_err(|e| invalid(name, e))?
        } else {
            // Some installations split large DAT files into segments that start
            // in the middle of the entry area and carry no container header.
            0
        };
        if offset < data_start {
            return Err(invalid(name, "resource entry overlaps container headers"));
        }
        let header = dat.read_at(offset, 24).await?;
        let entry =
            format::entry_header(&header, offset, dat.len()).map_err(|e| invalid(name, e))?;
        match entry.kind {
            2 => self.extract_standard(dat, name, offset, &entry).await,
            4 => self.extract_texture(dat, name, offset, &entry).await,
            kind => Err(Error::UnsupportedContentType {
                file: name.to_owned(),
                kind,
            }),
        }
    }

    async fn extract_standard(
        &self,
        dat: &PackReader<S::File>,
        name: &str,
        offset: u64,
        entry: &format::EntryHeader,
    ) -> Result<Vec<u8>, Error<S::Error>> {
        if let Some(limit) = self
            .max_read_bytes
            .filter(|&limit| entry.output_size > limit)
        {
            return Err(Error::ReadTooLarge {
                requested: entry.output_size,
                limit,
            });
        }
        let table_offset = offset.checked_add(24).ok_or(Error::RangeOverflow)?;
        let table = dat.read_at(table_offset, entry.table_size).await?;
        format::validate_blocks(&table, entry, dat.len()).map_err(|e| invalid(name, e))?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(entry.output_size)
            .map_err(|_| Error::AllocationFailed {
                requested: entry.output_size,
            })?;
        for record in table.as_chunks::<8>().0 {
            let block = format::block(record, entry.body_offset, dat.len())
                .map_err(|e| invalid(name, e))?;
            let bytes = dat.read_at(block.offset, block.stored_size).await?;
            let bytes =
                format::decode_block(&bytes, block.output_size).map_err(|e| invalid(name, e))?;
            output.extend_from_slice(&bytes);
        }
        Ok(output)
    }

    /// Rebuild a texture entry's file: the prefix stored at the entry body (the
    /// TEX header) followed by every level's decoded data blocks.
    async fn extract_texture(
        &self,
        dat: &PackReader<S::File>,
        name: &str,
        offset: u64,
        entry: &format::EntryHeader,
    ) -> Result<Vec<u8>, Error<S::Error>> {
        if let Some(limit) = self
            .max_read_bytes
            .filter(|&limit| entry.output_size > limit)
        {
            return Err(Error::ReadTooLarge {
                requested: entry.output_size,
                limit,
            });
        }
        let table = dat.read_at(offset + 24, entry.table_size).await?;
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(entry.table_size / 20)
            .map_err(|_| Error::AllocationFailed {
                requested: entry.table_size,
            })?;
        for record in table.as_chunks::<20>().0 {
            levels.push(format::lod_block(record).map_err(|e| invalid(name, e))?);
        }
        let Some(first) = levels.first() else {
            return Err(invalid(name, "texture entry has no levels"));
        };
        // One u16 span per data block follows the level table inside the entry header.
        let blocks = levels.iter().try_fold(0usize, |total, level| {
            total.checked_add(level.block_count as usize)
        });
        let blocks = blocks.ok_or_else(|| invalid(name, "texture block count overflow"))?;
        let spans_offset = 24usize
            .checked_add(entry.table_size)
            .ok_or_else(|| invalid(name, "texture table offset overflow"))?;
        let spans_length = blocks
            .checked_mul(2)
            .ok_or_else(|| invalid(name, "texture span table overflow"))?;
        if spans_offset
            .checked_add(spans_length)
            .is_none_or(|end| end > entry.header_size)
        {
            return Err(invalid(name, "texture span table exceeds entry header"));
        }
        let spans = dat
            .read_at(offset + spans_offset as u64, spans_length)
            .await?;
        let spans: Vec<u16> = spans
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();

        let header_size = usize::try_from(first.compressed_offset)
            .map_err(|_| invalid(name, "texture header size overflow"))?;
        if header_size > entry.output_size {
            return Err(invalid(name, "texture header exceeds output size"));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(entry.output_size)
            .map_err(|_| Error::AllocationFailed {
                requested: entry.output_size,
            })?;
        if header_size > 0 {
            output.extend_from_slice(&dat.read_at(entry.body_offset, header_size).await?);
        }
        let mut span = 0usize;
        for level in &levels {
            if level.block_count == 0 {
                return Err(invalid(name, "texture level has no data blocks"));
            }
            let mut start = entry
                .body_offset
                .checked_add(u64::from(level.compressed_offset))
                .ok_or_else(|| invalid(name, "texture block offset overflow"))?;
            let mut decoded = 0usize;
            for _ in 0..level.block_count {
                let header = dat.read_at(start, 16).await?;
                let block = format::block_header(&header).map_err(|e| invalid(name, e))?;
                if block.output_size > entry.output_size - output.len() {
                    return Err(invalid(name, "texture block exceeds remaining output size"));
                }
                let read_size = block
                    .header_size
                    .checked_add(block.stored_size)
                    .ok_or_else(|| invalid(name, "texture block size overflow"))?;
                let block_bytes = dat.read_at(start, read_size).await?;
                let decoded_block = format::decode_block(&block_bytes, block.output_size)
                    .map_err(|e| invalid(name, e))?;
                output.extend_from_slice(&decoded_block);
                decoded = decoded
                    .checked_add(block.output_size)
                    .ok_or_else(|| invalid(name, "texture decoded size overflow"))?;
                // The per-block span covers padding up to the next block.
                let occupied = block
                    .header_size
                    .checked_add(block.stored_size)
                    .ok_or_else(|| invalid(name, "texture block size overflow"))?;
                let advance = usize::from(spans[span]);
                if advance < occupied {
                    return Err(invalid(name, "texture block span overlaps next block"));
                }
                start = start
                    .checked_add(advance as u64)
                    .ok_or_else(|| invalid(name, "texture block offset overflow"))?;
                span += 1;
            }
            if level.decompressed_size != 0 && decoded != level.decompressed_size as usize {
                return Err(invalid(name, "texture level decoded size mismatch"));
            }
        }
        if output.len() != entry.output_size {
            return Err(invalid(name, "decoded size disagrees with entry size"));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_paths_normalize_and_route_without_a_platform_dependency() {
        let path = ResourcePath::parse("EXD/Item.Exh").unwrap();
        assert_eq!(path.path, "exd/item.exh");
        assert_eq!(path.hash, 0xe39b_7999_debb_8979);
        assert_eq!(path.hash2, 0xb118_d1a9);
        assert_eq!(path.stem(0), "ffxiv/0a0000.win32");
        let path = ResourcePath::parse("bg/ex3/zone/file.bin").unwrap();
        assert_eq!(path.stem(2), "ex3/020302.win32");
        let path = ResourcePath::parse("music/ex12/example.scd").unwrap();
        assert_eq!(path.stem(255), "ex12/0c0cff.win32");
        assert_eq!(
            ResourcePath::parse("bg/ffxiv/zone/file.bin")
                .unwrap()
                .stem(0),
            "ffxiv/020000.win32"
        );
        assert_eq!(
            ResourcePath::parse("chara/weapon/file.bin")
                .unwrap()
                .stem(0),
            "ffxiv/040000.win32"
        );
    }

    #[test]
    fn malformed_or_unknown_resource_names_are_rejected() {
        for path in [
            "",
            "item.exh",
            "unknown/file.bin",
            "/exd/item.exh",
            "exd/../item.exh",
            "bg/ex0/file.bin",
            "bg/ex256/file.bin",
            "bg/ex1",
            "exd//item.exh",
            "exd\\item.exh",
        ] {
            assert!(ResourcePath::parse(path).is_none(), "{path}");
        }
    }
}

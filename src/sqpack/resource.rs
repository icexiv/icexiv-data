//! Resource lookup orchestrates byte acquisition; format.rs owns binary parsing.

use super::{Error, PackReader, PackSource, SqPack, format, is_valid_file_name};

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
    /// Supports Win32 index/index2 and type-2 standard binary entries, including
    /// raw and raw-DEFLATE blocks. Models, textures and synonym/collision entries
    /// are not supported yet and return errors, not absence or guessed bytes.
    ///
    /// In the absence of a manifest or listing API, probes all 256 chunk names
    /// for the path's category/repository until found. Prefers index, using index2
    /// only when index is confirmed absent. Missing paths can therefore require
    /// 512 opens. Does not cache indexes or verify container SHA-1 checksums.
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
            let prefix = index.read_at(0, 24).await?;
            let header_offset = format::sqpack_header(&prefix, index.len(), 2)
                .map_err(|e| invalid(&index_name, e))?;
            let header = index.read_at(header_offset, 16).await?;
            let header = format::index_header(
                &header,
                header_offset,
                index.len(),
                if index2 { 8 } else { 16 },
            )
            .map_err(|e| invalid(&index_name, e))?;
            let table = index.read_at(header.offset, header.length).await?;
            let hash = if index2 {
                u64::from(path.hash2)
            } else {
                path.hash
            };
            let Some(location) = format::find_entry(&table, hash, index2).map_err(|e| match e {
                format::LookupError::Malformed(reason) => invalid(&index_name, reason),
                format::LookupError::Unsupported(reason) => Error::UnsupportedIndexEntry {
                    file: index_name.clone(),
                    reason,
                },
            })?
            else {
                continue;
            };
            let dat_name = format!("{stem}.dat{}", location.dat);
            let dat = self.open(&dat_name).await?;
            return self
                .extract_standard(&dat, &dat_name, location.offset)
                .await;
        }
        Err(Error::MissingResource(path.path))
    }

    async fn extract_standard(
        &self,
        dat: &PackReader<S::File>,
        name: &str,
        offset: u64,
    ) -> Result<Vec<u8>, Error<S::Error>> {
        let prefix = dat.read_at(0, 24).await?;
        let header_offset =
            format::sqpack_header(&prefix, dat.len(), 1).map_err(|e| invalid(name, e))?;
        let header = dat.read_at(header_offset, 4).await?;
        let data_start =
            format::data_start(&header, header_offset, dat.len()).map_err(|e| invalid(name, e))?;
        if offset < data_start {
            return Err(invalid(name, "resource entry overlaps container headers"));
        }
        let header = dat.read_at(offset, 24).await?;
        let entry =
            format::entry_header(&header, offset, dat.len()).map_err(|e| invalid(name, e))?;
        if entry.kind != 2 {
            return Err(Error::UnsupportedContentType {
                file: name.to_owned(),
                kind: entry.kind,
            });
        }
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
        format::validate_blocks(&table, &entry, dat.len()).map_err(|e| invalid(name, e))?;
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

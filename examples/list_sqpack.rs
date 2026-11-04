//! List the hashed resource organization, not reconstructed resource paths.
//!
//! # Usage
//!
//! ```sh
//! cargo run --features local --example list_sqpack -- /path/to/game/sqpack > resources.txt
//! ```
//!
//! Scans regular Win32 index files directly under `ffxiv/` and `ex<digits>/`,
//! skipping symlink entries. Prefers `.index` over `.index2` for each pack and
//! never reads DAT files. Scan and parse failures are reported as errors.
//!
//! Output is grouped by repository, pack and hashes, not original names:
//!
//! ```text
//! ffxiv/
//!   0a0000.win32.index (1 entries)
//!     directory-hash:e39b7999/
//!       filename-hash:debb8979 -> dat0 @ 0x800
//! ```
//!
//! This is illustrative output, not an installed-game measurement. Index2-only
//! packs list `path-hash:` entries without directory grouping. Duplicate records
//! are retained and synonyms are labeled unresolved. All entries are printed;
//! redirect stdout for large installations.
//!
//! Requires a native platform and creates its own Tokio runtime. Each read is
//! capped at 64 MiB, not total memory use. Keep the installation stable while
//! reading it.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(native::run())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The list_sqpack example requires a native platform.");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use icexiv_data::sqpack::index::{Index, IndexHash, IndexKind};
    use icexiv_data::sqpack::{SqPack, local::Local};
    use std::collections::BTreeMap;
    use std::io::{self, Write};
    use std::path::{Path, PathBuf};

    const USAGE: &str = "Usage: cargo run --features local --example list_sqpack -- <game/sqpack>\n\
        Lists Win32 resources by repository, pack and hashes (not original names).\n\
        Prefers .index over .index2 for the same pack. Does not read DAT files.\n\
        Outputs all entries; redirect stdout to save large listings.\n\
        Per-read cap: 64 MiB. Do not patch the installation while reading.";

    fn repository(name: &str) -> bool {
        name == "ffxiv"
            || name
                .strip_prefix("ex")
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
    }

    fn index_name(name: &str) -> Option<(&str, IndexKind)> {
        let (stem, kind) = if let Some(stem) = name.strip_suffix(".win32.index") {
            (stem, IndexKind::Index)
        } else {
            (name.strip_suffix(".win32.index2")?, IndexKind::Index2)
        };
        (stem.len() == 6 && stem.bytes().all(|b| b.is_ascii_hexdigit())).then_some((stem, kind))
    }

    // Filesystem discovery belongs to this local example, not the parser API.
    // Do not follow symlink entries or silently turn scan errors into absence.
    fn discover(root: &Path) -> io::Result<BTreeMap<String, BTreeMap<String, IndexKind>>> {
        let mut repositories = BTreeMap::new();
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(repo) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !repository(&repo) {
                continue;
            }
            let mut packs = BTreeMap::new();
            for file in std::fs::read_dir(entry.path())? {
                let file = file?;
                if !file.file_type()?.is_file() {
                    continue;
                }
                let name = file.file_name();
                let Some((stem, kind)) = name.to_str().and_then(index_name) else {
                    continue;
                };
                // Preserve exact filename case; group the two extensions only.
                let selected = packs.entry(stem.to_owned()).or_insert(kind);
                if kind == IndexKind::Index {
                    *selected = kind;
                }
            }
            if !packs.is_empty() {
                repositories.insert(repo, packs);
            }
        }
        Ok(repositories)
    }

    fn write_entries(out: &mut impl Write, index: &Index) -> io::Result<()> {
        let mut entries: Vec<_> = index.entries().collect();
        // Keep duplicate records, including flagged records; sort only for display.
        entries.sort_by_key(|entry| match entry.hash {
            IndexHash::Index {
                directory,
                filename,
            } => (directory, filename),
            IndexHash::Index2(hash) => (0, hash),
        });
        let mut previous_directory = None;
        for entry in entries {
            match entry.hash {
                IndexHash::Index {
                    directory,
                    filename,
                } => {
                    if previous_directory != Some(directory) {
                        writeln!(out, "    directory-hash:{directory:08x}/")?;
                        previous_directory = Some(directory);
                    }
                    write!(out, "      filename-hash:{filename:08x}")?;
                }
                IndexHash::Index2(hash) => write!(out, "    path-hash:{hash:08x}")?,
            }
            if let Some(location) = entry.location() {
                writeln!(out, " -> dat{} @ 0x{:x}", location.dat, location.offset)?;
            } else {
                writeln!(
                    out,
                    " -> synonym (unresolved, packed=0x{:08x})",
                    entry.packed
                )?;
            }
        }
        Ok(())
    }

    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() == 1 && args[0] == "--help" {
            println!("{USAGE}");
            return Ok(());
        }
        if args.len() != 1 {
            return Err(USAGE.into());
        }
        let root = PathBuf::from(&args[0]);
        let source = Local::new(&root).await?;
        let repositories = tokio::task::spawn_blocking(move || discover(&root)).await??;
        if repositories.is_empty() {
            return Err(
                "No regular Win32 index files found under ffxiv/ or ex*/ (symlinks are skipped)."
                    .into(),
            );
        }
        let sqpack = SqPack::new(source).with_max_read_bytes(Some(64 * 1024 * 1024));
        let mut out = io::BufWriter::new(io::stdout().lock());
        writeln!(
            out,
            "# Hash-based organization only; names and synonym targets are unresolved."
        )?;
        for (repo, packs) in repositories {
            writeln!(out, "{repo}/")?;
            for (stem, kind) in packs {
                let extension = if kind == IndexKind::Index {
                    "index"
                } else {
                    "index2"
                };
                let file = format!("{stem}.win32.{extension}");
                let index = sqpack.read_index(&format!("{repo}/{file}"), kind).await?;
                writeln!(out, "  {file} ({} entries)", index.len())?;
                write_entries(&mut out, &index)?;
            }
        }
        out.flush()?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn recognizes_only_repository_and_win32_index_names() {
            assert!(repository("ffxiv"));
            assert!(repository("ex12"));
            assert!(!repository("ex"));
            assert!(!repository("extra"));
            assert_eq!(
                index_name("0a0000.win32.index"),
                Some(("0a0000", IndexKind::Index))
            );
            assert_eq!(
                index_name("020101.win32.index2"),
                Some(("020101", IndexKind::Index2))
            );
            for name in [
                "0a0000.win32.dat0",
                "0a0000.ps3.index",
                "xyzxyz.win32.index",
                "000.win32.index",
            ] {
                assert!(index_name(name).is_none());
            }
        }

        #[test]
        fn displays_directory_groups_duplicates_locations_and_synonyms() {
            let mut table = Vec::new();
            for (directory, filename, packed) in [(2u32, 3u32, 1u32), (1, 4, 0x102), (1, 4, 0x102)]
            {
                table.extend_from_slice(&filename.to_le_bytes());
                table.extend_from_slice(&directory.to_le_bytes());
                table.extend_from_slice(&packed.to_le_bytes());
                table.extend_from_slice(&0u32.to_le_bytes());
            }
            let index = Index::from_table(table, IndexKind::Index).unwrap();
            let mut out = Vec::new();
            write_entries(&mut out, &index).unwrap();
            assert_eq!(
                String::from_utf8(out).unwrap(),
                "    directory-hash:00000001/\n      filename-hash:00000004 -> dat1 @ 0x800\n      filename-hash:00000004 -> dat1 @ 0x800\n    directory-hash:00000002/\n      filename-hash:00000003 -> synonym (unresolved, packed=0x00000001)\n"
            );
        }

        #[test]
        fn index2_does_not_invent_directory_hashes() {
            let table = [42u32.to_le_bytes(), 0x100u32.to_le_bytes()].concat();
            let index = Index::from_table(table, IndexKind::Index2).unwrap();
            let mut out = Vec::new();
            write_entries(&mut out, &index).unwrap();
            assert_eq!(
                String::from_utf8(out).unwrap(),
                "    path-hash:0000002a -> dat0 @ 0x800\n"
            );
        }
    }
}

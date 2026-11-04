//! Show the sheet names listed in `exd/root.exl` as a path tree.
//!
//! # Usage
//!
//! ```sh
//! cargo run --features local --example show_root_exl -- /path/to/game/sqpack
//! ```
//!
//! The argument is the SqPack root (`game/sqpack`, not `game`). Reads only
//! `exd/root.exl`, without loading EXH or EXD files. Illustrative output:
//!
//! ```text
//! exd/
//! ├── Quest/
//! │   └── Example_001
//! └── Item
//! ```
//!
//! Slashes in sheet names form directories. Numeric suffixes in sheet names are
//! preserved: they do not identify EXD pages. Leaves are sheet names, not page
//! filenames. Redirect stdout to save the tree, and keep the installation stable
//! while reading it.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(native::run())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The show_root_exl example requires a native platform.");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use icexiv_data::sqpack::{SqPack, local::Local};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{self, Write};
    use std::path::Path;

    const USAGE: &str = "Usage: cargo run --features local --example show_root_exl -- <game/sqpack>\n\
        Lists root.exl sheet names as a path tree without reading EXH or EXD files.";

    #[derive(Default)]
    struct Tree {
        directories: BTreeMap<String, Tree>,
        sheets: BTreeSet<String>,
    }

    impl Tree {
        fn insert(&mut self, sheet: &str) {
            let mut parts = sheet.split('/').peekable();
            let mut directory = self;
            while let Some(part) = parts.next() {
                if parts.peek().is_none() {
                    directory.sheets.insert(part.to_owned());
                    break;
                }
                directory = directory.directories.entry(part.to_owned()).or_default();
            }
        }

        fn write(&self, out: &mut impl Write, prefix: &str) -> io::Result<()> {
            let mut remaining = self.directories.len() + self.sheets.len();
            for (name, directory) in &self.directories {
                remaining -= 1;
                let branch = if remaining == 0 {
                    "└──"
                } else {
                    "├──"
                };
                writeln!(out, "{prefix}{branch} {name}/")?;
                let continuation = if remaining == 0 { "    " } else { "│   " };
                directory.write(out, &format!("{prefix}{continuation}"))?;
            }
            for name in &self.sheets {
                remaining -= 1;
                let branch = if remaining == 0 {
                    "└──"
                } else {
                    "├──"
                };
                writeln!(out, "{prefix}{branch} {name}")?;
            }
            Ok(())
        }
    }

    // EXL text handling is local to this example, not part of the binary parsers.
    fn sheet_names(bytes: &[u8]) -> io::Result<Vec<String>> {
        let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
        let text = std::str::from_utf8(bytes).map_err(|error| invalid(error.to_string()))?;
        let mut lines = text.trim_start_matches('\u{feff}').lines();
        let header = lines.next().ok_or_else(|| invalid("empty EXL".into()))?;
        let (magic, version) = header
            .split_once(',')
            .ok_or_else(|| invalid("invalid EXL header".into()))?;
        if magic != "EXLT" || version.parse::<u32>().is_err() {
            return Err(invalid("invalid EXL header".into()));
        }

        let mut names = Vec::new();
        let mut seen = BTreeSet::new();
        for (index, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            let (name, id) = line
                .split_once(',')
                .ok_or_else(|| invalid(format!("EXL line {}: expected name,id", index + 2)))?;
            if id.parse::<i32>().is_err()
                || !name.split('/').all(|part| {
                    !part.is_empty()
                        && part != "."
                        && part != ".."
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
                })
                // Bound the depth of the example's recursive tree writer.
                || name.split('/').count() > 64
            {
                return Err(invalid(format!(
                    "EXL line {}: invalid sheet entry",
                    index + 2
                )));
            }
            if !seen.insert(name.to_ascii_lowercase()) {
                return Err(invalid(format!(
                    "EXL line {}: duplicate sheet {name}",
                    index + 2
                )));
            }
            names.push(name.to_owned());
        }
        Ok(names)
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

        let source = Local::new(Path::new(&args[0])).await?;
        let sqpack = SqPack::new(source);
        let bytes = sqpack.read("exd/root.exl").await?;
        let names = sheet_names(&bytes)?;
        let mut tree = Tree::default();
        for name in names {
            tree.insert(&name);
        }

        let mut stdout = io::stdout().lock();
        writeln!(stdout, "exd/")?;
        tree.write(&mut stdout, "")?;
        stdout.flush()?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn exl_preserves_paths_numeric_suffixes_and_unassigned_ids() {
            assert_eq!(
                sheet_names(b"EXLT,2\r\nItem,1\r\nQuest/Example_001,-1\r\n\r\n").unwrap(),
                ["Item", "Quest/Example_001"]
            );
            assert_eq!(sheet_names(b"EXLT,2\n").unwrap(), Vec::<String>::new());
        }

        #[test]
        fn malformed_exl_is_not_silently_treated_as_an_empty_list() {
            for bytes in [
                b"".as_slice(),
                b"Other,2\nItem,1\n",
                b"EXLT,x\n",
                b"EXLT,2\nItem\n",
                b"EXLT,2\nItem,no-id\n",
                b"EXLT,2\n../Item,1\n",
                b"EXLT,2\nQuest//Item,1\n",
                b"EXLT,2\nItem,1\nitem,2\n",
                b"EXLT,2\n\xff,1\n",
            ] {
                assert!(sheet_names(bytes).is_err());
            }
        }

        #[test]
        fn tree_groups_sheet_names_without_stripping_numeric_suffixes() {
            let mut tree = Tree::default();
            tree.insert("Item");
            tree.insert("Quest/Example_001");
            tree.insert("Quest/Example_002");
            tree.insert("Quest/Nested/Empty");
            let mut output = Vec::new();
            tree.write(&mut output, "").unwrap();
            assert_eq!(
                String::from_utf8(output).unwrap(),
                "├── Quest/\n│   ├── Nested/\n│   │   └── Empty\n│   ├── Example_001\n│   └── Example_002\n└── Item\n"
            );
        }

        #[test]
        fn sheet_and_directory_with_the_same_name_do_not_overwrite_each_other() {
            let mut tree = Tree::default();
            tree.insert("Foo");
            tree.insert("Foo/Bar");
            let mut output = Vec::new();
            tree.write(&mut output, "").unwrap();
            assert_eq!(
                String::from_utf8(output).unwrap(),
                "├── Foo/\n│   └── Bar\n└── Foo\n"
            );
        }
    }
}

//! Args: sqpack-root manifest new-output-directory.
//! Manifest lines: index2-name followed by path, DAT name, offset triplets (TSV).
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use icexiv_data::sqpack::{
        PackSource, SqPack,
        index::{Index, IndexKind, Location},
        local::{Local, LocalFile},
    };
    use std::{cell::Cell, io, path::Path};
    struct Index2Only {
        local: Local,
        hidden: Cell<usize>,
    }
    impl PackSource for Index2Only {
        type Error = io::Error;
        type File = LocalFile;
        async fn open(&self, name: &str) -> io::Result<Option<LocalFile>> {
            if name.ends_with(".index") {
                self.hidden.set(self.hidden.get() + 1);
                return Ok(None);
            }
            self.local.open(name).await
        }
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let args: Vec<_> = std::env::args().skip(1).collect();
            if args.len() != 3 {
                return Err("expected root manifest output-directory".into());
            }
            let root = Path::new(&args[0]);
            let out = Path::new(&args[2]);
            std::fs::create_dir(out)?;
            let source = Index2Only {
                local: Local::new(root).await?,
                hidden: Cell::new(0),
            };
            let pack = SqPack::new(source)
                .with_max_read_bytes(Some(64 << 20))
                .with_index_cache(Some(64 << 20));
            let direct = SqPack::new(Local::new(root).await?).with_max_read_bytes(Some(64 << 20));
            let mut total = 0;
            for line in std::fs::read_to_string(&args[1])?.lines() {
                let fields: Vec<_> = line.split('\t').collect();
                if fields.is_empty() || fields.len() % 3 != 1 {
                    return Err("invalid manifest".into());
                }
                let name = fields[0];
                if !name.ends_with(".index2") {
                    return Err("manifest requires index2".into());
                }
                let bytes = std::fs::read(root.join(name))?;
                let borrowed = Index::parse(&bytes, IndexKind::Index2)?;
                let loaded = pack.read_index(name, IndexKind::Index2).await?;
                assert_eq!(
                    borrowed.entries().collect::<Vec<_>>(),
                    loaded.entries().collect::<Vec<_>>()
                );
                assert_eq!(
                    borrowed
                        .collision_records()
                        .map(|r| r.map(|v| v.raw().to_vec()).collect::<Vec<_>>()),
                    loaded
                        .collision_records()
                        .map(|r| r.map(|v| v.raw().to_vec()).collect::<Vec<_>>())
                );
                for triple in fields[1..].chunks_exact(3) {
                    let path = triple[0];
                    let dat = triple[1];
                    let offset: u64 = triple[2].parse()?;
                    let dat_id = dat
                        .rsplit_once(".dat")
                        .ok_or("invalid DAT name")?
                        .1
                        .parse()?;
                    let expected = Some(Location {
                        dat: dat_id,
                        offset,
                    });
                    assert_eq!(borrowed.find_path(path)?, expected);
                    assert_eq!(loaded.find_path(path)?, expected);
                    let decoded = pack.read(path).await?;
                    assert_eq!(decoded, direct.read_entry(dat, offset).await?);
                    std::fs::write(out.join(format!("{total:03}.bin")), &decoded)?;
                    println!("{total}\t{path}\t{}", decoded.len());
                    total += 1;
                }
            }
            // The wrapper never opens a primary index: it returns confirmed absence.
            assert!(total > 0);
            Ok(())
        })
}
#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("validate_index2 requires native local access");
}

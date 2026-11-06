//! Read-only IMC validation: <game/sqpack> <resource-path> ...
//! No game execution; prints every decoded record for independent comparison.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("validate_imc requires a native platform");
}

#[cfg(not(target_arch = "wasm32"))]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use icexiv_data::{
        imc::Imc,
        sqpack::{SqPack, local::Local},
    };
    let mut args = std::env::args_os().skip(1).peekable();
    let root = args.next().ok_or("expected game/sqpack and IMC paths")?;
    let animation_fields = args.next_if(|arg| arg == "--animation-fields").is_some();
    let sqpack = SqPack::new(Local::new(root).await?)
        .with_max_read_bytes(Some(64 * 1024 * 1024))
        .with_index_cache(Some(16 * 1024 * 1024));
    let mut files = 0;
    for path in args {
        let path = path.to_str().ok_or("invalid resource path")?;
        let bytes = sqpack.read(path).await?;
        let imc = Imc::parse(&bytes)?;
        println!(
            "file\t{path}\t{}\t{}\t{:#06x}",
            bytes.len(),
            imc.variant_count(),
            imc.part_mask()
        );
        for subset in 0..=imc.variant_count() {
            for part in imc.parts() {
                let entry = imc.entry(subset, part)?;
                println!(
                    "entry\t{subset}\t{part}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    entry.material_id,
                    entry.decal_id,
                    entry.raw_mask,
                    entry.attribute_mask(),
                    entry.sound_id(),
                    entry.vfx_id,
                    entry.raw_animation
                );
                if animation_fields {
                    println!(
                        "animation\t{subset}\t{part}\t{}",
                        entry.material_animation_id()
                    );
                }
            }
        }
        files += 1;
    }
    if files == 0 {
        return Err("expected at least one IMC path".into());
    }
    Ok(())
}

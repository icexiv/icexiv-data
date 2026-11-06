//! Args: extracted.pbd ...; raw EPBD root fields and local selector associations.
use icexiv_data::pbd::Pbd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args_os().skip(1) {
        if std::fs::metadata(&path)?.len() > 64 * 1024 * 1024 {
            return Err("PBD exceeds 64 MiB".into());
        }
        let bytes = std::fs::read(path)?;
        let pbd = Pbd::parse(&bytes)?;
        let extension = pbd.extension().ok_or("expected EPBD")?;
        println!(
            "root\t{:?}\t{:?}\t{}",
            extension.version_tag(),
            extension.unknown_root_scalar(),
            extension.entry_count()
        );
        // The reversed pair and wide IDs test exact raw identities, while the
        // original pairs exercise data lookup without selecting local IDs.
        for (child, parent) in [
            (1501, 901),
            (1601, 201),
            (1701, 101),
            (1801, 201),
            (901, 1501),
            (0, 0),
            (u32::MAX, u32::MAX),
            (0x10000 + 1801, 201),
        ] {
            let found = extension.find_entry(child, parent, extension.entry_count())?;
            println!(
                "pair\t{child}\t{parent}\t{:?}",
                found.map(|e| (e.child_race, e.parent_race, e.pbd_bytes().len()))
            );
        }
        if extension.entry_count() != 0 {
            assert!(matches!(
                extension.find_entry(1501, 901, extension.entry_count() - 1),
                Err(icexiv_data::pbd::Error::BudgetExceeded("EPBD entry query"))
            ));
        }
        for i in 0..extension.entry_count() {
            let entry = extension.entry(i)?;
            let inner = entry.pbd()?;
            for (raw, local) in entry.conditions().zip(entry.local_skeleton_ids()) {
                let found = inner.find_deformer(local)?;
                println!(
                    "selector\t{i}\t{}\t{}\t{raw}\t{local}\t{:?}",
                    entry.child_race,
                    entry.parent_race,
                    found.map(|d| (d.skeleton_id, d.connection_index, d.bone_count()))
                );
            }
        }
    }
    Ok(())
}

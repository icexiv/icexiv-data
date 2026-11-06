//! Args: extracted.cmp. Mathematical percentages, not native customize policy.
use icexiv_data::cmp::{Cmp, Error, FILE_SIZE, Sex};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected CMP input")?;
    if std::fs::metadata(&path)?.len() != FILE_SIZE as u64 {
        return Err("CMP profile size".into());
    }
    let b = std::fs::read(path)?;
    let c = Cmp::parse(&b)?;
    for i in 0..c.parameter_count() {
        let p = c.parameter_record(i)?;
        for percent in 0..=100 {
            let bust = p.bust().at_percent(percent)?;
            let values = [
                p.height(Sex::Male).at_percent(percent)?,
                p.tail(Sex::Male).at_percent(percent)?,
                p.height(Sex::Female).at_percent(percent)?,
                p.tail(Sex::Female).at_percent(percent)?,
                bust[0],
                bust[1],
                bust[2],
            ];
            println!(
                "sample\t{i}\t{percent}\t{}",
                values.map(|v| format!("{:08x}", v.to_bits())).join(",")
            );
        }
        for percent in 101..=255 {
            for r in [
                p.height(Sex::Male),
                p.tail(Sex::Male),
                p.height(Sex::Female),
                p.tail(Sex::Female),
            ] {
                if r.at_percent(percent) != Err(Error::InvalidPercentage(percent)) {
                    return Err("unexpected percent policy".into());
                }
            }
            if p.bust().at_percent(percent) != Err(Error::InvalidPercentage(percent)) {
                return Err("unexpected vector percent policy".into());
            }
        }
    }
    Ok(())
}

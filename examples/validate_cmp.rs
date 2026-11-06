//! Args: extracted human.cmp; all colors and coordinate-addressed parameter bits.
use icexiv_data::cmp::{Cmp, Sex};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("expected CMP path")?;
    if std::fs::metadata(&path)?.len() != icexiv_data::cmp::FILE_SIZE as u64 {
        return Err("unsupported CMP profile".into());
    }
    let b = std::fs::read(path)?;
    let c = Cmp::parse(&b)?;
    println!(
        "cmp\t{}\t{}\t{}",
        b.len(),
        c.color_count(),
        c.parameter_count()
    );
    for i in 0..c.color_count() {
        let color = c.color(i)?;
        println!(
            "color\t{i}\t{:02x}{:02x}{:02x}{:02x}",
            color[0], color[1], color[2], color[3]
        );
    }
    for race in 0..8 {
        for body in 1..=5 {
            for clan in 0..2 {
                let p = c.parameters(race, body, clan)?;
                let bits = p.raw.map(|f| format!("{:08x}", f.to_bits())).join(",");
                println!("parameters\t{race}\t{body}\t{clan}\t{bits}");
                for sex in [Sex::Male, Sex::Female] {
                    let h = p.height(sex);
                    let t = p.tail(sex);
                    println!(
                        "ranges\t{:?}\t{:08x},{:08x},{:08x},{:08x}",
                        sex,
                        h.min.to_bits(),
                        h.max.to_bits(),
                        t.min.to_bits(),
                        t.max.to_bits()
                    );
                }
                let v = p.bust();
                println!(
                    "bust\t{}",
                    v.min
                        .into_iter()
                        .chain(v.max)
                        .map(|f| format!("{:08x}", f.to_bits()))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
    }
    Ok(())
}

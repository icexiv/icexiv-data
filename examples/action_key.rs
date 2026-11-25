//! Resolve an Action row against supplied, extracted Action/ActionTimeline
//! EXH/EXD page pairs. The linked row must be in the supplied page.
use icexiv_data::excel::{Exd, Exh};
use std::{error::Error, io::Read};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [ah, ad, th, td, id] = args.as_slice() else {
        return Err("Action.exh Action.exd ActionTimeline.exh ActionTimeline.exd action-id".into());
    };
    let load = |p: &str| -> Result<Vec<u8>, Box<dyn Error>> {
        let mut b = Vec::new();
        std::fs::File::open(p)?
            .take((64 << 20) + 1)
            .read_to_end(&mut b)?;
        if b.len() > 64 << 20 {
            return Err("sheet exceeds 64 MiB".into());
        }
        Ok(b)
    };
    let ah = Exh::parse(&load(ah)?)?;
    let th = Exh::parse(&load(th)?)?;
    let ad = load(ad)?;
    let td = load(td)?;
    let actions = Exd::parse(&ad, &ah)?;
    let keys = Exd::parse(&td, &th)?;
    let key_id = actions
        .row(id.parse()?)?
        .ok_or("missing Action row")?
        .action_timeline_id()?;
    let key = keys
        .row(u32::try_from(key_id)?)?
        .ok_or("missing ActionTimeline row")?
        .action_timeline_key()?;
    println!("{id}\t{key_id}\t{}", std::str::from_utf8(key)?);
    Ok(())
}

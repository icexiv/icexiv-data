//! Inspect extracted STM files and optionally a Stain EXH/EXD pair.
//! Args: legacy.stm dawntrail.stm [Stain.exh Stain-page.exd]
//! Fingerprints cover each column's raw bytes and every representable stain.
use icexiv_data::{
    excel::{Exd, Exh, Value},
    stm::Stm,
};
struct Hash(u64);
impl Hash {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ u64::from(x)).wrapping_mul(0x100000001b3);
        }
    }
    fn word(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
fn value(v: Value<'_>) -> String {
    match v {
        Value::String(b) => format!("s:{}", hex(b)),
        Value::Bool(b) => format!("b:{}", u8::from(b)),
        Value::Int8(v) => format!("i8:{v}"),
        Value::UInt8(v) => format!("u8:{v}"),
        Value::Int16(v) => format!("i16:{v}"),
        Value::UInt16(v) => format!("u16:{v}"),
        Value::Int32(v) => format!("i32:{v}"),
        Value::UInt32(v) => format!("u32:{v}"),
        Value::Int64(v) => format!("i64:{v}"),
        Value::UInt64(v) => format!("u64:{v}"),
        Value::Float32(v) => format!("f32:{:08x}", v.to_bits()),
    }
}
fn read(path: &std::ffi::OsStr) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if std::fs::metadata(path)?.len() > 16 * 1024 * 1024 {
        return Err("sample exceeds 16 MiB".into());
    }
    Ok(std::fs::read(path)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 && args.len() != 4 {
        return Err("expected two STMs and optional Stain EXH/EXD pair".into());
    }
    for path in &args[..2] {
        let b = read(path)?;
        let stm = Stm::parse(&b)?;
        println!(
            "stm\t{}\t{:?}\t{}\t{},{}\t{}",
            b.len(),
            stm.layout(),
            stm.template_count(),
            stm.declared_channel_counts()[0],
            stm.declared_channel_counts()[1],
            stm.trailing_bytes().len()
        );
        for i in 0..stm.template_count() {
            let template = stm.template(i)?;
            println!(
                "template\t{}\t{}",
                stm.template_id(i)?,
                template.stain_capacity()?
            );
            for c in 0..template.column_count() {
                let column = template.column(c)?;
                let mut raw = Hash::new();
                raw.bytes(column.raw());
                let mut decoded = Hash::new();
                for stain in 1..=column.stain_capacity() as u16 {
                    let bits = column.half_bits(stain)?;
                    let values = column.value(stain)?;
                    for j in 0..3 {
                        decoded.word(u32::from(bits[j]));
                        decoded.word(values[j].to_bits());
                    }
                }
                println!(
                    "column\t{c}\t{:?}\t{}\t{}\t{}\t{:016x}\t{:016x}",
                    column.encoding(),
                    column.raw().len(),
                    column.palette_count(),
                    column.components(),
                    raw.0,
                    decoded.0
                );
            }
        }
    }
    if args.len() == 4 {
        let h = read(&args[2])?;
        let b = read(&args[3])?;
        let header = Exh::parse(&h)?;
        let page = Exd::parse(&b, &header)?;
        println!("stains\t{}\t{}", page.len(), header.columns().len());
        for row in page.rows() {
            println!(
                "stain\t{}\t{}",
                row.id(),
                row.fields()
                    .map(|f| f.map(value))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\t")
            );
        }
    }
    Ok(())
}

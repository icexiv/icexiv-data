//! Inspect complete EXH and EXD values. Args: extracted.exh extracted.exd ...
use icexiv_data::excel::{ColumnType, Exd, Exh, Language, Value, Variant};
fn kind(t: ColumnType) -> u16 {
    match t {
        ColumnType::String => 0,
        ColumnType::Bool => 1,
        ColumnType::Int8 => 2,
        ColumnType::UInt8 => 3,
        ColumnType::Int16 => 4,
        ColumnType::UInt16 => 5,
        ColumnType::Int32 => 6,
        ColumnType::UInt32 => 7,
        ColumnType::Float32 => 9,
        ColumnType::Int64 => 10,
        ColumnType::UInt64 => 11,
        ColumnType::PackedBool(b) => 25 + u16::from(b),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err("EXH and EXD paths required".into());
    }
    let load = |path: &std::ffi::OsStr| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if std::fs::metadata(path)?.len() > 64 << 20 {
            return Err("input size".into());
        }
        Ok(std::fs::read(path)?)
    };
    let raw = load(&args[0])?;
    let h = Exh::parse(&raw)?;
    println!(
        "header\t{}\t{}\t{}\t{}\t{}\t{}",
        h.fixed_row_size(),
        if h.variant() == Variant::Default {
            1
        } else {
            2
        },
        h.row_count(),
        h.columns().len(),
        h.pages().len(),
        h.languages().len()
    );
    for (i, c) in h.columns().iter().enumerate() {
        println!("column\t{i}\t{}\t{}", kind(c.data_type), c.offset);
    }
    for p in h.pages() {
        println!("page\t{}\t{}", p.start_id, p.row_count);
    }
    for l in h.languages() {
        let id = match l {
            Language::None => 0,
            Language::Japanese => 1,
            Language::English => 2,
            Language::German => 3,
            Language::French => 4,
            Language::ChineseSimplified => 5,
            Language::ChineseTraditional => 6,
            Language::Korean => 7,
        };
        println!("language\t{id}\t{}", l.suffix());
    }
    for path in &args[1..] {
        let b = load(path)?;
        let page = Exd::parse(&b, &h)?;
        println!(
            "exd\t{}\t{}\t{}",
            b.len(),
            page.parent_row_count(),
            page.len()
        );
        let mut work = 2_000_000usize;
        for row in page.rows() {
            let mut values = Vec::new();
            for value in row.fields() {
                work = work.checked_sub(1).ok_or("field output budget")?;
                let v = match value? {
                    Value::String(v) => format!(
                        "s:{}",
                        v.iter().map(|v| format!("{v:02x}")).collect::<String>()
                    ),
                    Value::Bool(v) => format!("b:{}", u8::from(v)),
                    Value::Int8(v) => format!("i8:{v}"),
                    Value::UInt8(v) => format!("u8:{v}"),
                    Value::Int16(v) => format!("i16:{v}"),
                    Value::UInt16(v) => format!("u16:{v}"),
                    Value::Int32(v) => format!("i32:{v}"),
                    Value::UInt32(v) => format!("u32:{v}"),
                    Value::Int64(v) => format!("i64:{v}"),
                    Value::UInt64(v) => format!("u64:{v}"),
                    Value::Float32(v) => format!("f:{:08x}", v.to_bits()),
                };
                values.push(v);
            }
            println!(
                "row\t{}\t{}\t{}",
                row.id(),
                row.subrow_id()
                    .map_or_else(|| "none".into(), |v| v.to_string()),
                values.join("\t")
            );
        }
    }
    Ok(())
}

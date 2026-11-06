//! Newly authored EXH/EXD fixtures. Fields are written independently with byte
//! operations, never BinWrite or extracted game tables.

use icexiv_data::excel::{ColumnType, Error, Exd, Exh, Language, Value, Variant};

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn exh(
    fixed: u16,
    variant: u8,
    columns: &[(u16, u16)],
    pages: &[(u32, u32)],
    languages: &[u16],
) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    bytes[..4].copy_from_slice(b"EXHF");
    put16(&mut bytes, 4, 3);
    put16(&mut bytes, 6, fixed);
    put16(&mut bytes, 8, columns.len() as u16);
    put16(&mut bytes, 10, pages.len() as u16);
    put16(&mut bytes, 12, languages.len() as u16);
    // Ensure unknown bytes are not confused with the one-byte variant.
    bytes[16] = 0xa5;
    bytes[17] = variant;
    put32(&mut bytes, 20, 42);
    for &(kind, offset) in columns {
        bytes.extend_from_slice(&kind.to_be_bytes());
        bytes.extend_from_slice(&offset.to_be_bytes());
    }
    for &(start, count) in pages {
        bytes.extend_from_slice(&start.to_be_bytes());
        bytes.extend_from_slice(&count.to_be_bytes());
    }
    for language in languages {
        bytes.extend_from_slice(&language.to_le_bytes());
    }
    bytes
}

fn exd(rows: &[(u32, u16, &[u8])]) -> Vec<u8> {
    let index_size = rows.len() * 8;
    let mut bytes = vec![0; 32 + index_size];
    bytes[..4].copy_from_slice(b"EXDF");
    put16(&mut bytes, 4, 2);
    put32(&mut bytes, 8, index_size as u32);
    for (i, &(id, count, payload)) in rows.iter().enumerate() {
        let start = bytes.len();
        put32(&mut bytes, 32 + i * 8, id);
        put32(&mut bytes, 36 + i * 8, start as u32);
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes.extend_from_slice(payload);
    }
    // EXD data-section length excludes the 32-byte header and row index.
    let data_size = bytes.len() - 32 - index_size;
    put32(&mut bytes, 12, data_size as u32);
    bytes
}

fn header(fixed: u16, variant: u8, columns: &[(u16, u16)]) -> Exh {
    Exh::parse(&exh(fixed, variant, columns, &[(0, 1000)], &[0])).unwrap()
}

#[test]
fn header_metadata_and_page_boundaries_are_decoded_without_schema_names() {
    let header = Exh::parse(&exh(
        8,
        1,
        &[(7, 0), (0x20, 7)],
        &[(100, 10), (0, 2)],
        &[2, 5],
    ))
    .unwrap();
    assert_eq!(header.fixed_row_size(), 8);
    assert_eq!(header.variant(), Variant::Default);
    assert_eq!(header.row_count(), 42);
    assert_eq!(header.columns()[0].data_type, ColumnType::UInt32);
    assert_eq!(header.columns()[1].data_type, ColumnType::PackedBool(7));
    assert_eq!(
        header.languages(),
        &[Language::English, Language::ChineseSimplified]
    );
    assert_eq!(header.pages()[0].start_id, 0);
    assert_eq!(header.page_for_row(1).unwrap().start_id, 0);
    assert!(header.page_for_row(2).is_none());
    assert_eq!(header.page_for_row(109).unwrap().start_id, 100);
    assert!(header.page_for_row(110).is_none());
    assert_eq!(
        header
            .page_paths("Content/Story", Language::English)
            .unwrap()
            .collect::<Vec<_>>(),
        ["exd/content/story_0_en.exd", "exd/content/story_100_en.exd"]
    );
    assert!(matches!(
        header.page_paths("Item", Language::Japanese),
        Err(Error::LanguageUnavailable(Language::Japanese))
    ));
}

#[test]
fn every_language_suffix_and_unlocalized_paths_are_explicit() {
    let languages = [
        Language::None,
        Language::Japanese,
        Language::English,
        Language::German,
        Language::French,
        Language::ChineseSimplified,
        Language::ChineseTraditional,
        Language::Korean,
    ];
    let suffixes = ["", "_ja", "_en", "_de", "_fr", "_chs", "_cht", "_ko"];
    let hdr = Exh::parse(&exh(0, 1, &[], &[(5, 1)], &(0..8).collect::<Vec<_>>())).unwrap();
    assert_eq!(hdr.languages(), languages);
    for (language, suffix) in languages.into_iter().zip(suffixes) {
        assert_eq!(
            hdr.page_paths("Test", language).unwrap().next().unwrap(),
            format!("exd/test_5{suffix}.exd")
        );
    }
    let hdr = Exh::parse(&exh(0, 1, &[], &[(0, 1)], &[])).unwrap();
    assert_eq!(
        hdr.page_paths("Test", Language::None)
            .unwrap()
            .next()
            .unwrap(),
        "exd/test_0.exd"
    );
    assert!(hdr.page_paths("Test", Language::English).is_err());
    for sheet in [
        "", "/item", ".", "..", "a/../b", "a//b", "a?b", "a%2fb", "a\\b",
    ] {
        assert!(matches!(
            hdr.page_paths(sheet, Language::None),
            Err(Error::InvalidSheetName)
        ));
    }
}

#[test]
fn all_scalar_columns_and_packed_bits_decode_big_endian() {
    let mut columns = vec![
        (0, 0),
        (1, 4),
        (2, 5),
        (3, 6),
        (4, 8),
        (5, 10),
        (6, 12),
        (7, 16),
        (9, 20),
        (10, 24),
        (11, 32),
    ];
    columns.extend((0x19..=0x20).map(|kind| (kind, 7)));
    let hdr = header(40, 1, &columns);
    let mut payload = vec![0; 40];
    payload[4] = 0xfe;
    payload[5] = (-7i8) as u8;
    payload[6] = 240;
    payload[7] = 0b1010_0101;
    payload[8..10].copy_from_slice(&(-1234i16).to_be_bytes());
    put16(&mut payload, 10, 50000);
    payload[12..16].copy_from_slice(&(-9876543i32).to_be_bytes());
    put32(&mut payload, 16, 0x8123_4567);
    put32(&mut payload, 20, (-12.5f32).to_bits());
    payload[24..32].copy_from_slice(&(i64::MIN + 1234).to_be_bytes());
    payload[32..40].copy_from_slice(&(u64::MAX - 7).to_be_bytes());
    payload.extend_from_slice(b"hello \xff\0");
    let bytes = exd(&[(42, 1, &payload)]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page.parent_row_count(), 1);
    let row = page.row(42).unwrap().unwrap();
    assert_eq!(row.id(), 42);
    assert_eq!(row.subrow_id(), None);
    let expected = [
        Value::String(b"hello \xff"),
        Value::Bool(true),
        Value::Int8(-7),
        Value::UInt8(240),
        Value::Int16(-1234),
        Value::UInt16(50000),
        Value::Int32(-9876543),
        Value::UInt32(0x8123_4567),
        Value::Float32(-12.5),
        Value::Int64(i64::MIN + 1234),
        Value::UInt64(u64::MAX - 7),
    ];
    for (i, expected) in expected.into_iter().enumerate() {
        assert_eq!(row.field(i).unwrap(), expected);
    }
    for bit in 0..8 {
        assert_eq!(
            row.field(11 + bit).unwrap(),
            Value::Bool(payload[7] & (1 << bit) != 0)
        );
    }
    assert_eq!(row.fields().len(), columns.len());
    assert!(matches!(
        row.field(columns.len()),
        Err(Error::ColumnOutOfBounds { .. })
    ));
    if let Value::String(value) = row.field(0).unwrap() {
        assert_eq!(value.as_ptr(), bytes[32 + 8 + 6 + 40..].as_ptr());
        assert!(std::str::from_utf8(value).is_err());
    } else {
        panic!("expected raw borrowed string");
    }
    assert!(page.row(99).unwrap().is_none());
    assert!(matches!(page.subrow(42, 0), Err(Error::VariantMismatch)));
}

#[test]
fn default_strings_support_offsets_aliases_empty_and_raw_payloads() {
    let hdr = header(8, 1, &[(0, 0), (0, 4), (0, 0)]);
    let mut payload = vec![0; 8];
    put32(&mut payload, 4, 1);
    payload.extend_from_slice(b"\0\x02\xffopaque\x03\0");
    let bytes = exd(&[(1, 1, &payload)]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    let row = page.row(1).unwrap().unwrap();
    assert_eq!(row.field(0).unwrap(), Value::String(b""));
    assert_eq!(row.field(1).unwrap(), Value::String(b"\x02\xffopaque\x03"));
    assert_eq!(row.field(2).unwrap(), Value::String(b""));
}

fn subrows() -> Vec<u8> {
    let mut payload = vec![0; 16];
    put16(&mut payload, 0, 7);
    put32(&mut payload, 2, 0); // relative to the shared parent string pool
    put16(&mut payload, 6, 100);
    put16(&mut payload, 8, 42);
    put32(&mut payload, 10, 6); // same shared string-pool origin
    put16(&mut payload, 14, 200);
    payload.extend_from_slice(b"alpha\0beta\0");
    payload
}

#[test]
fn subrows_use_stored_sparse_ids_and_bounded_shared_strings() {
    let hdr = header(6, 2, &[(0, 0), (5, 4)]);
    let bytes = exd(&[(88, 2, &subrows())]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert_eq!(page.parent_row_count(), 1);
    assert_eq!(page.len(), 2);
    assert_eq!(
        page.rows()
            .map(|row| (row.id(), row.subrow_id()))
            .collect::<Vec<_>>(),
        [(88, Some(7)), (88, Some(42))]
    );
    let first = page.subrow(88, 7).unwrap().unwrap();
    assert_eq!(first.field(0).unwrap(), Value::String(b"alpha"));
    assert_eq!(first.field(1).unwrap(), Value::UInt16(100));
    let second = page.subrow(88, 42).unwrap().unwrap();
    assert_eq!(second.field(0).unwrap(), Value::String(b"beta"));
    assert_eq!(second.field(1).unwrap(), Value::UInt16(200));
    assert!(page.subrow(88, 0).unwrap().is_none());
    assert!(page.subrow(999, 7).unwrap().is_none());
    assert!(matches!(page.row(88), Err(Error::VariantMismatch)));
}

#[test]
fn unsorted_parent_and_subrow_ids_are_supported_but_duplicates_are_not() {
    let hdr = header(0, 1, &[]);
    let bytes = exd(&[(9, 1, b""), (2, 1, b"")]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert_eq!(page.rows().map(|row| row.id()).collect::<Vec<_>>(), [2, 9]);
    assert!(Exd::parse(&exd(&[(2, 1, b""), (2, 1, b"")]), &hdr).is_err());
    let hdr = header(0, 2, &[]);
    let bytes = exd(&[(9, 2, &[0xff, 0xff, 0, 1])]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert!(page.subrow(9, u16::MAX).unwrap().is_some());
    assert!(page.subrow(9, 1).unwrap().is_some());
    assert!(Exd::parse(&exd(&[(9, 2, &[0, 1, 0, 1])]), &hdr).is_err());
}

#[test]
fn empty_pages_empty_subrow_groups_and_zero_width_rows_are_safe() {
    for variant in [1, 2] {
        let hdr = header(0, variant, &[]);
        let bytes = exd(&[]);
        let page = Exd::parse(&bytes, &hdr).unwrap();
        assert!(page.is_empty());
        assert_eq!(page.rows().count(), 0);
    }
    let hdr = header(0, 1, &[]);
    let bytes = exd(&[(1, 1, b"")]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert_eq!(page.row(1).unwrap().unwrap().fields().count(), 0);
    let hdr = header(0, 2, &[]);
    let bytes = exd(&[(1, 0, b"")]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert!(page.is_empty());
    assert_eq!(page.parent_row_count(), 1);
    assert!(page.subrow(1, 0).unwrap().is_none());
}

#[test]
fn every_truncated_header_and_page_prefix_is_rejected() {
    let bytes = exh(4, 1, &[(7, 0)], &[(0, 5)], &[2]);
    for length in 0..bytes.len() {
        assert!(Exh::parse(&bytes[..length]).is_err(), "EXH prefix {length}");
    }
    let hdr = Exh::parse(&bytes).unwrap();
    let bytes = exd(&[(1, 1, &[1, 2, 3, 4])]);
    for length in 0..bytes.len() {
        assert!(
            Exd::parse(&bytes[..length], &hdr).is_err(),
            "EXD prefix {length}"
        );
    }
}

#[test]
fn unknown_versions_variants_columns_and_languages_are_explicit_errors() {
    let mut bytes = exh(4, 1, &[(7, 0)], &[], &[0]);
    bytes[0] = 0;
    assert!(Exh::parse(&bytes).is_err());
    let mut bytes = exh(4, 1, &[(7, 0)], &[], &[0]);
    put16(&mut bytes, 4, 99);
    assert!(matches!(
        Exh::parse(&bytes),
        Err(Error::UnsupportedVersion {
            format: "EXH",
            version: 99
        })
    ));
    assert!(matches!(
        Exh::parse(&exh(4, 9, &[(7, 0)], &[], &[0])),
        Err(Error::UnsupportedVariant(9))
    ));
    assert!(matches!(
        Exh::parse(&exh(4, 1, &[(8, 0)], &[], &[0])),
        Err(Error::UnsupportedColumnType(8))
    ));
    assert!(matches!(
        Exh::parse(&exh(4, 1, &[(7, 0)], &[], &[256])),
        Err(Error::UnsupportedLanguage(256))
    ));
    let hdr = header(0, 1, &[]);
    let mut bytes = exd(&[]);
    put16(&mut bytes, 4, 99);
    assert!(matches!(
        Exd::parse(&bytes, &hdr),
        Err(Error::UnsupportedVersion {
            format: "EXD",
            version: 99
        })
    ));
}

#[test]
fn column_spans_page_ranges_and_languages_are_validated() {
    assert!(Exh::parse(&exh(3, 1, &[(0, 0)], &[], &[0])).is_err());
    assert!(Exh::parse(&exh(8, 1, &[(11, u16::MAX)], &[], &[0])).is_err());
    assert!(Exh::parse(&exh(0, 1, &[], &[(0, 10), (5, 10)], &[0])).is_err());
    assert!(Exh::parse(&exh(0, 1, &[], &[(0, 0), (0, 1)], &[0])).is_err());
    assert!(Exh::parse(&exh(0, 1, &[], &[(u32::MAX, 2)], &[0])).is_err());
    let hdr = Exh::parse(&exh(0, 1, &[], &[(u32::MAX, 1)], &[0])).unwrap();
    assert!(hdr.page_for_row(u32::MAX).is_some());
    assert!(Exh::parse(&exh(0, 1, &[], &[], &[2, 2])).is_err());
    let mut bytes = exh(0, 1, &[], &[], &[]);
    put16(&mut bytes, 8, u16::MAX);
    assert!(Exh::parse(&bytes).is_err());
}

#[test]
fn malformed_row_indexes_and_payload_lengths_never_become_absence() {
    let hdr = header(4, 1, &[(7, 0)]);
    for (offset, value) in [
        (8, 7),
        (8, u32::MAX - 7),
        (36, 0),
        (36, u32::MAX),
        (40, u32::MAX),
    ] {
        let mut bytes = exd(&[(1, 1, &[0; 4])]);
        put32(&mut bytes, offset, value);
        assert!(Exd::parse(&bytes, &hdr).is_err(), "offset {offset}");
    }
    let mut bytes = exd(&[(1, 1, &[0; 4]), (2, 1, &[0; 4])]);
    let first = u32::from_be_bytes(bytes[36..40].try_into().unwrap());
    put32(&mut bytes, 44, first);
    assert!(Exd::parse(&bytes, &hdr).is_err());
    assert!(Exd::parse(&exd(&[(1, 0, &[0; 4])]), &hdr).is_err());
    assert!(Exd::parse(&exd(&[(1, 2, &[0; 4])]), &hdr).is_err());
    let hdr = header(4, 2, &[(7, 0)]);
    assert!(Exd::parse(&exd(&[(1, 2, &[0; 6])]), &hdr).is_err());
}

#[test]
fn strings_use_the_shared_pool_and_cannot_escape_their_parent_row() {
    let hdr = header(4, 1, &[(0, 0)]);
    let bytes = exd(&[(1, 1, &[0, 0, 0, 0, b'a']), (2, 1, &[0; 5])]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert!(page.row(1).unwrap().unwrap().field(0).is_err());
    assert_eq!(
        page.row(2).unwrap().unwrap().field(0).unwrap(),
        Value::String(b"")
    );
    let bytes = exd(&[(1, 1, &[255, 255, 255, 255, 0])]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert!(page.row(1).unwrap().unwrap().field(0).is_err());
    let hdr = header(6, 2, &[(0, 0), (5, 4)]);
    let mut payload = subrows();
    put32(&mut payload, 2, u32::MAX);
    let bytes = exd(&[(88, 2, &payload)]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    assert!(page.subrow(88, 7).unwrap().unwrap().field(0).is_err());
    assert_eq!(
        page.subrow(88, 42).unwrap().unwrap().field(0).unwrap(),
        Value::String(b"beta")
    );
}

#[test]
fn excel_policy_limits_cover_metadata_counts_validation_and_lazy_strings() {
    use icexiv_data::excel::Limits;
    let bytes = exh(4, 1, &[(0, 0)], &[(0, 10)], &[0]);
    for limits in [
        Limits {
            max_input_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_metadata_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_columns: 0,
            ..Limits::default()
        },
        Limits {
            max_work: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Exh::parse_with_limits(&bytes, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
    let h = Exh::parse(&bytes).unwrap();
    let b = exd(&[(1, 1, b"\0\0\0\0hello\0")]);
    for limits in [
        Limits {
            max_input_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_metadata_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_parent_rows: 0,
            ..Limits::default()
        },
        Limits {
            max_rows: 0,
            ..Limits::default()
        },
        Limits {
            max_work: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Exd::parse_with_limits(&b, &h, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
    let p = Exd::parse_with_limits(
        &b,
        &h,
        Limits {
            max_string_bytes: 4,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        p.row(1).unwrap().unwrap().field(0),
        Err(Error::BudgetExceeded("string length"))
    ));
    let p = Exd::parse_with_limits(
        &b,
        &h,
        Limits {
            max_string_bytes: 5,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(
        p.row(1).unwrap().unwrap().field(0).unwrap(),
        Value::String(b"hello")
    );
    let mut wrong = b.clone();
    put32(&mut wrong, 12, 0);
    assert!(Exd::parse(&wrong, &h).is_err());
    wrong = b.clone();
    wrong.push(0);
    assert!(Exd::parse(&wrong, &h).is_err());
}

#[test]
fn first_and_later_subrows_alias_the_same_parent_string_pool() {
    let h = header(6, 2, &[(0, 0), (5, 4)]);
    let mut data = subrows();
    put32(&mut data, 10, 0);
    let b = exd(&[(88, 2, &data)]);
    let p = Exd::parse(&b, &h).unwrap();
    let a = p.subrow(88, 7).unwrap().unwrap().field(0).unwrap();
    let c = p.subrow(88, 42).unwrap().unwrap().field(0).unwrap();
    assert_eq!(a, Value::String(b"alpha"));
    assert_eq!(a, c);
    if let (Value::String(a), Value::String(c)) = (a, c) {
        assert_eq!(a.as_ptr(), c.as_ptr());
    }
}

#[test]
fn float_bits_are_not_silently_normalized() {
    let hdr = header(4, 1, &[(9, 0)]);
    let bits = 0x7fc0_1234u32;
    let bytes = exd(&[(1, 1, &bits.to_be_bytes())]);
    let page = Exd::parse(&bytes, &hdr).unwrap();
    match page.row(1).unwrap().unwrap().field(0).unwrap() {
        Value::Float32(value) => assert_eq!(value.to_bits(), bits),
        _ => panic!("expected f32"),
    }
}

#[test]
fn extracted_sheet_header_drives_all_page_paths_and_row_decoding() {
    use icexiv_data::sqpack::{PackSource, ReadAt, SqPack};
    use std::collections::HashMap;
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};

    struct Memory(HashMap<String, Arc<Vec<u8>>>);
    struct File(Arc<Vec<u8>>);
    impl PackSource for Memory {
        type Error = &'static str;
        type File = File;
        async fn open(&self, name: &str) -> Result<Option<File>, Self::Error> {
            Ok(self.0.get(name).cloned().map(File))
        }
    }
    impl ReadAt for File {
        type Error = &'static str;
        fn len(&self) -> u64 {
            self.0.len() as u64
        }
        async fn read_at(&self, offset: u64, length: usize) -> Result<Vec<u8>, Self::Error> {
            let offset = usize::try_from(offset).map_err(|_| "offset overflow")?;
            let end = offset.checked_add(length).ok_or("range overflow")?;
            Ok(self.0.get(offset..end).ok_or("out of range")?.to_vec())
        }
    }
    fn container(kind: u32) -> Vec<u8> {
        let mut bytes = vec![0; 0x800];
        bytes[..8].copy_from_slice(b"SqPack\0\0");
        for (offset, value) in [(12, 0x400u32), (16, 1), (20, kind), (0x400, 0x400)] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    let resources = [
        (
            "exd/fixture.exh",
            exh(4, 1, &[(7, 0)], &[(0, 2), (10, 1)], &[2]),
        ),
        (
            "exd/fixture_0_en.exd",
            exd(&[(1, 1, &20u32.to_be_bytes()), (0, 1, &10u32.to_be_bytes())]),
        ),
        (
            "exd/fixture_10_en.exd",
            exd(&[(10, 1, &30u32.to_be_bytes())]),
        ),
    ];
    let mut index = container(2);
    index[0x404..0x408].copy_from_slice(&1u32.to_le_bytes());
    index[0x408..0x40c].copy_from_slice(&0x800u32.to_le_bytes());
    index[0x40c..0x410].copy_from_slice(&(resources.len() as u32 * 16).to_le_bytes());
    let mut dat = container(1);
    for (path, payload) in resources {
        let (folder, name) = path.rsplit_once('/').unwrap();
        let hash = (u64::from(!crc32fast::hash(folder.as_bytes())) << 32)
            | u64::from(!crc32fast::hash(name.as_bytes()));
        index.extend_from_slice(&hash.to_le_bytes());
        index.extend_from_slice(&(dat.len() as u32 / 8).to_le_bytes());
        index.extend_from_slice(&0u32.to_le_bytes());
        let start = dat.len();
        let stored = (16 + payload.len()).div_ceil(128) * 128;
        dat.resize(start + 128 + stored, 0);
        for (offset, value) in [
            (0, 128u32),
            (4, 2),
            (8, payload.len() as u32),
            (20, 1),
            (128, 16),
            (136, 32000),
            (140, payload.len() as u32),
        ] {
            dat[start + offset..start + offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        dat[start + 28..start + 30].copy_from_slice(&(stored as u16).to_le_bytes());
        dat[start + 30..start + 32].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        dat[start + 144..start + 144 + payload.len()].copy_from_slice(&payload);
    }
    let source = Memory(HashMap::from([
        ("ffxiv/0a0000.win32.index".to_owned(), Arc::new(index)),
        ("ffxiv/0a0000.win32.dat0".to_owned(), Arc::new(dat)),
    ]));
    let future = async {
        let sqpack = SqPack::new(source);
        let bytes = sqpack.read("exd/fixture.exh").await.unwrap();
        let header = Exh::parse(&bytes).unwrap();
        let mut values = Vec::new();
        for path in header.page_paths("Fixture", Language::English).unwrap() {
            let bytes = sqpack.read(&path).await.unwrap();
            let page = Exd::parse(&bytes, &header).unwrap();
            for row in page.rows() {
                let Value::UInt32(value) = row.field(0).unwrap() else {
                    panic!("expected UInt32");
                };
                values.push((row.id(), value));
            }
        }
        assert_eq!(values, [(0, 10), (1, 20), (10, 30)]);
    };
    // This fixture backend is immediately ready; no runtime, server or game data.
    let mut future = std::pin::pin!(future);
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    ));
}

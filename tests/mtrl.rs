use icexiv_data::mtrl::{ColorLayout, Error, Mtrl};

fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn append(b: &mut Vec<u8>, n: usize) -> usize {
    let p = b.len();
    b.resize(p + n, 0);
    p
}
struct Fixture {
    b: Vec<u8>,
    dataset: usize,
    material: usize,
    constants: usize,
    samplers: usize,
}
fn fixture(data_size: usize) -> Fixture {
    let strings = b"x.tex\0map1\0colorSet1\0test.shpk\0";
    let mut b = vec![0; 28];
    put32(&mut b, 0, 0x01030000);
    put16(&mut b, 6, data_size as u16);
    put16(&mut b, 8, strings.len() as u16);
    put16(&mut b, 10, 21);
    b[12..16].copy_from_slice(&[1, 1, 1, 4]);
    put16(&mut b, 18, 0x8000);
    put16(&mut b, 20, 6);
    put16(&mut b, 24, 11);
    b.extend_from_slice(strings);
    let flags: u32 = match data_size {
        512 => 4,
        544 => 12,
        2048 => 0x534,
        2176 => 0x53c,
        _ => 0,
    };
    b.extend_from_slice(&flags.to_le_bytes());
    let dataset = append(&mut b, data_size);
    if data_size >= 512 {
        for (lane, bits) in [
            (0, 0x3c00),
            (1, 0xbc00),
            (2, 0x4000),
            (4, 0x3800),
            (8, 0x4200),
            (12, 0x4000),
            (13, 0xbc00),
            (14, 0x3c00),
            (15, 0x3c00),
        ] {
            put16(&mut b, dataset + lane * 2, bits);
        }
        if data_size < 2048 {
            put16(&mut b, dataset + 22, 0x2600);
        }
    }
    if data_size == 544 {
        put16(&mut b, dataset + 512, (25 << 5) | 0x1b);
    }
    if data_size >= 2048 {
        for (lane, bits) in [
            (16, 0x3800),
            (18, 0x3c00),
            (24, 0x4700),
            (25, 0x2600),
            (27, 0x4200),
            (28, 0x4000),
            (29, 0xbc00),
            (30, 0x3c00),
            (31, 0x3c00),
        ] {
            put16(&mut b, dataset + lane * 2, bits);
        }
        if data_size == 2176 {
            put32(&mut b, dataset + 2048, (1 << 27) | (25 << 16) | 0x1037);
        }
    }
    let material = append(&mut b, 12);
    put16(&mut b, material, 8);
    for offset in [2, 4, 6] {
        put16(&mut b, material + offset, 1);
    }
    put32(&mut b, material + 8, 0x11);
    let p = append(&mut b, 8);
    put32(&mut b, p, 12);
    put32(&mut b, p + 4, 34);
    let constants = append(&mut b, 8);
    put32(&mut b, constants, 77);
    put16(&mut b, constants + 6, 8);
    let samplers = append(&mut b, 12);
    put32(&mut b, samplers, 0x0c5ec1f1);
    put32(&mut b, samplers + 4, 0x112233);
    b[samplers + 9..samplers + 12].copy_from_slice(&[0xa5, 0x5a, 1]);
    b.extend_from_slice(&2f32.to_le_bytes());
    b.extend_from_slice(&(-0.5f32).to_le_bytes());
    let len = b.len();
    put16(&mut b, 4, len as u16);
    Fixture {
        b,
        dataset,
        material,
        constants,
        samplers,
    }
}

#[test]
fn declared_color_dimensions_disambiguate_equal_sized_datasets() {
    for (size, lanes, rows, dye) in [
        (512, 16, 16, false),
        (544, 16, 16, true),
        (2048, 32, 32, false),
        (2176, 32, 32, true),
    ] {
        let f = fixture(size);
        let m = Mtrl::parse(&f.b).unwrap();
        let d = m.inline_color_table_declaration().unwrap();
        assert_eq!((d.half_lanes, d.rows, d.has_dye), (lanes, rows, dye));
        assert!(m.color_table().unwrap().is_some());
    }
    let mut f = fixture(544);
    // 8 rows x 32 half lanes + 8 u32 dye words also occupies 544 bytes.
    // It must not be mislabeled as a 16-row Legacy table by byte length alone.
    put32(&mut f.b, f.dataset - 4, 0x33c);
    let m = Mtrl::parse(&f.b).unwrap();
    assert_eq!(
        (
            m.inline_color_table_declaration().unwrap().half_lanes,
            m.inline_color_table_declaration().unwrap().rows
        ),
        (32, 8)
    );
    assert!(matches!(
        m.color_table(),
        Err(Error::UnsupportedColorTableLayout {
            half_lanes: 32,
            rows: 8
        })
    ));
    assert_eq!(m.raw_dataset().len(), 544);
    put32(&mut f.b, f.dataset - 4, 0xfffc);
    let m = Mtrl::parse(&f.b).unwrap();
    let d = m.inline_color_table_declaration().unwrap();
    assert_eq!((d.half_lanes, d.rows), (131072, 32768));
    assert!(matches!(
        m.color_table(),
        Err(Error::UnsupportedColorTableLayout { .. })
    ));
    for size in [512, 544, 2048, 2176] {
        let mut f = fixture(size);
        f.b[f.dataset - 4] ^= 8;
        assert!(matches!(
            Mtrl::parse(&f.b).unwrap().color_table(),
            Err(Error::InvalidData("dye declaration disagrees with dataset"))
        ));
    }
    assert!(
        Mtrl::parse(&fixture(0).b)
            .unwrap()
            .inline_color_table_declaration()
            .is_none()
    );
}
#[test]
fn every_legacy_dye_word_preserves_low_flags_and_template_without_channel_policy() {
    let mut f = fixture(544);
    for page in 0..4096u32 {
        for row in 0..16usize {
            put16(
                &mut f.b,
                f.dataset + 512 + row * 2,
                (page * 16 + row as u32) as u16,
            );
        }
        let m = Mtrl::parse(&f.b).unwrap();
        let table = m.color_table().unwrap().unwrap();
        for row in 0..16usize {
            let word = page * 16 + row as u32;
            let d = table.dye_row(row).unwrap().unwrap();
            assert_eq!(d.raw, word);
            assert_eq!(u32::from(d.flags()), word % 32);
            assert_eq!(u32::from(d.template()), word / 32);
            assert_eq!(d.channel(), None);
            assert_eq!(d.unknown_bits(), 0);
        }
    }
}
#[test]
fn material_metadata_and_constant_values_are_not_placeholders() {
    let f = fixture(0);
    let m = Mtrl::parse(&f.b).unwrap();
    assert_eq!(m.shader_package().unwrap(), b"test.shpk");
    assert_eq!(m.texture(0).unwrap().path, b"x.tex");
    assert_eq!(m.texture(0).unwrap().flags, 0x8000);
    assert_eq!(m.uv_set(0).unwrap().name, b"map1");
    assert_eq!(m.color_set(0).unwrap().name, b"colorSet1");
    assert_eq!(m.shader_key(0).unwrap().value, 34);
    assert_eq!(
        m.constant_floats(0).unwrap().collect::<Vec<_>>(),
        [2., -0.5]
    );
    assert_eq!(m.sampler(0).unwrap().reserved, [0xa5, 0x5a, 1]);
    assert_eq!(m.shader_flags(), 0x11);
    assert!(m.color_table().unwrap().is_none());
}

#[test]
fn both_color_table_layouts_keep_hdr_negative_values_and_all_rows() {
    for size in [512, 544, 2048, 2176] {
        let f = fixture(size);
        let m = Mtrl::parse(&f.b).unwrap();
        let t = m.color_table().unwrap().unwrap();
        let legacy = size < 2048;
        assert_eq!(
            t.layout(),
            if legacy {
                ColorLayout::Legacy
            } else {
                ColorLayout::Dawntrail
            }
        );
        assert_eq!(t.row_count(), if legacy { 16 } else { 32 });
        let row = t.row(0).unwrap();
        assert_eq!(row.diffuse(), [1., -1., 2.]);
        assert_eq!(row.tile_selector(), 1.5);
        assert_eq!(row.tile_matrix(), [2., -1., 1., 1.]);
        assert_eq!(row.roughness(), if legacy { None } else { Some(0.5) });
        if !legacy {
            assert_eq!(row.shader_selector(), Some(7.));
            assert_eq!(row.sphere_selector(), Some(3.));
        }
        assert!(t.row(t.row_count()).is_err());
        assert!(row.value(usize::MAX).is_err());
        assert_eq!(t.has_dye(), matches!(size, 544 | 2176));
        if let Some(dye) = t.dye_row(0).unwrap() {
            assert_eq!(dye.template(), 25);
            assert_eq!(dye.flags(), if legacy { 0x1b } else { 0x37 });
            assert_eq!(dye.channel(), if legacy { None } else { Some(1) });
            assert_eq!(dye.unknown_bits(), if legacy { 0 } else { 0x1000 });
        }
    }
}

#[test]
fn unknown_dataset_is_not_reported_as_confirmed_absence() {
    let f = fixture(4);
    let m = Mtrl::parse(&f.b).unwrap();
    assert_eq!(m.raw_dataset(), &f.b[f.dataset..f.dataset + 4]);
    assert!(matches!(
        m.color_table(),
        Err(Error::UnsupportedDatasetSize(4))
    ));
}

#[test]
fn invalid_lengths_references_and_string_budget_return_errors() {
    for mutation in 0..9 {
        let mut f = fixture(2176);
        match mutation {
            0 => put16(&mut f.b, 4, 1),
            1 => put16(&mut f.b, 10, u16::MAX),
            2 => put16(&mut f.b, 8, u16::MAX),
            3 => put16(&mut f.b, f.material, u16::MAX),
            4 => put16(&mut f.b, f.material + 2, u16::MAX),
            5 => put16(&mut f.b, f.constants + 4, 4),
            6 => f.b[f.samplers + 8] = 1,
            7 => f.b[15] = 255,
            _ => put32(&mut f.b, 0, 0),
        }
        assert!(Mtrl::parse(&f.b).is_err(), "mutation {mutation}");
    }
    let f = fixture(0);
    assert!(matches!(
        Mtrl::parse_with_string_budget(&f.b, 0),
        Err(Error::StringBudgetExceeded)
    ));
    let mut unaligned = f.b.clone();
    put16(&mut unaligned, f.constants + 4, 1);
    put16(&mut unaligned, f.constants + 6, 4);
    let m = Mtrl::parse(&unaligned).unwrap();
    assert!(m.constant_floats(0).is_err());
    assert_eq!(m.constant_bytes(0).unwrap().len(), 4);
}

#[test]
fn truncations_and_arbitrary_mutations_never_panic() {
    let f = fixture(2176);
    for n in 0..f.b.len() {
        assert!(Mtrl::parse(&f.b[..n]).is_err());
    }
    let mut state = 0x1415u32;
    for _ in 0..4096 {
        let mut b = f.b.clone();
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let i = state as usize % b.len();
        b[i] ^= (state >> 16) as u8;
        if let Ok(m) = Mtrl::parse(&b) {
            let _ = m.color_table();
            let _ = m.sampler(usize::MAX);
        }
    }
}
#[test]
fn dye_lane_mask_separates_source_layout_and_destination_profile() {
    use icexiv_data::mtrl::{ColorLayout, DyeOverrideProfile, DyeRow};
    let expected = [
        0x7,
        0x70,
        0x700,
        1 << 11,
        1 << 18,
        1 << 16,
        1 << 12,
        1 << 13,
        1 << 14,
        1 << 19,
        1 << 27,
        1 << 21,
    ];
    for (flag, mask) in expected.into_iter().enumerate() {
        for extra in [0, 0xffff_f000] {
            let dye = DyeRow {
                layout: ColorLayout::Dawntrail,
                raw: (1 << flag) | extra,
            };
            assert_eq!(
                dye.canonical_override_lane_mask(DyeOverrideProfile::Dawntrail),
                mask
            );
        }
    }
    let all = expected.into_iter().fold(0, |mask, bits| mask | bits);
    assert_eq!(
        DyeRow {
            layout: ColorLayout::Dawntrail,
            raw: u32::MAX
        }
        .canonical_override_lane_mask(DyeOverrideProfile::Dawntrail),
        all
    );
    assert_eq!(
        DyeRow {
            layout: ColorLayout::Dawntrail,
            raw: 0xffff_f000
        }
        .canonical_override_lane_mask(DyeOverrideProfile::Dawntrail),
        0
    );
    let legacy_targets = [0x7, 0x70, 0x700, 1 << 3, 1 << 7];
    for layout in [ColorLayout::Legacy, ColorLayout::Dawntrail] {
        for (flag, mask) in legacy_targets.into_iter().enumerate() {
            let dye = DyeRow {
                layout,
                raw: 1 << flag,
            };
            assert_eq!(
                dye.canonical_override_lane_mask(DyeOverrideProfile::Legacy),
                mask
            );
            assert_eq!(
                dye.canonical_override_lane_mask(DyeOverrideProfile::Dawntrail),
                expected[flag]
            );
        }
    }
    // Source Legacy template bits do not become additional destination flags.
    let dye = DyeRow {
        layout: ColorLayout::Legacy,
        raw: 0xffe0,
    };
    for profile in [DyeOverrideProfile::Legacy, DyeOverrideProfile::Dawntrail] {
        assert_eq!(dye.canonical_override_lane_mask(profile), 0);
    }
}

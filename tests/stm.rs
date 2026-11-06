use icexiv_data::stm::{Encoding, Error, Layout, Limits, Stm};
fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn halfs(values: &[u16]) -> Vec<u8> {
    values.iter().copied().flat_map(u16::to_le_bytes).collect()
}
fn fixture(layout: Layout) -> Vec<u8> {
    let n = layout.column_count();
    let mut template = vec![0; n * 2];
    let mut palette = halfs(&[0x3c00, 0, 0, 0x4000, 0x3800, 0xbc00]);
    let mut map = [1; 254];
    map[0] = 255;
    map[2] = 2;
    map[3] = 0;
    palette.extend_from_slice(&map);
    let mut columns = vec![
        palette,
        halfs(&[0x3800; 3]),
        vec![],
        (0..254u16).flat_map(u16::to_le_bytes).collect(),
        halfs(&[0x4000]),
    ];
    columns.resize(n, vec![]);
    for (i, c) in columns.iter().enumerate() {
        template.extend_from_slice(c);
        let end = (template.len() - n * 2) / 2;
        put16(&mut template, i * 2, end as u16);
    }
    let mut b = vec![0; 24];
    b[..2].copy_from_slice(b"MS");
    put16(
        &mut b,
        2,
        if layout == Layout::Legacy {
            0x0101
        } else {
            0x0201
        },
    );
    put16(&mut b, 4, 2);
    if layout == Layout::Dawntrail {
        b[6] = 3;
        b[7] = 9;
    }
    put32(&mut b, 8, 100);
    put32(&mut b, 12, 200);
    put32(&mut b, 20, (template.len() / 2) as u32);
    b.extend_from_slice(&template);
    b.extend_from_slice(&template);
    b.extend_from_slice(&[0xa5; 16]);
    b
}

#[test]
fn both_layouts_decode_real_storage_variants_without_owned_expansion() {
    for layout in [Layout::Legacy, Layout::Dawntrail] {
        let b = fixture(layout);
        let stm = Stm::parse(&b).unwrap();
        assert_eq!(stm.layout(), layout);
        assert_eq!(stm.template_count(), 2);
        assert_eq!(stm.trailing_bytes(), [0xa5; 16]);
        let t = stm.find_template(100).unwrap().unwrap();
        assert_eq!(t.stain_capacity().unwrap(), 253);
        assert_eq!(t.column(0).unwrap().encoding(), Encoding::Palette);
        assert_eq!(t.column(0).unwrap().value(2).unwrap(), [2., 0.5, -1.]);
        assert_eq!(t.column(0).unwrap().value(3).unwrap(), [0.; 3]);
        assert_eq!(t.column(1).unwrap().encoding(), Encoding::Uniform);
        assert_eq!(t.column(2).unwrap().encoding(), Encoding::Zero);
        assert_eq!(t.column(3).unwrap().encoding(), Encoding::Dense);
        assert_eq!(t.column(3).unwrap().half_bits(254).unwrap(), [253, 0, 0]);
        let values = t.sample(2).unwrap();
        assert_eq!(values.diffuse, [2., 0.5, -1.]);
        assert_eq!(values.specular, [0.5; 3]);
        if layout == Layout::Legacy {
            assert_eq!(values.specular_strength(), Some(2.));
            assert_eq!(values.metalness(), None);
        } else {
            assert_eq!(values.metalness(), Some(2.));
            assert_eq!(values.gloss_strength(), None);
        }
        assert!(stm.find_template(999).unwrap().is_none());
        assert!(stm.template(usize::MAX).is_err());
        assert!(t.sample(0).is_err());
        assert!(t.sample(254).is_err());
        assert!(t.column(usize::MAX).is_err());
        assert!(matches!(
            t.column(0).unwrap().value(254),
            Err(Error::StainOutOfBounds { .. })
        ));
    }
}

#[test]
fn invalid_palettes_are_errors_not_silent_default_colors() {
    for mutation in 0..8 {
        let mut b = fixture(Layout::Dawntrail);
        let data = 24 + 24;
        match mutation {
            0 => put32(&mut b, 16, u32::MAX),
            1 => put32(&mut b, 12, 100),
            2 => put16(&mut b, 24, u16::MAX),
            3 => put16(&mut b, 26, 1),
            4 => b[data + 12] = 0,
            5 => b[data + 13] = 3,
            6 => b[7] = 8,
            _ => put16(&mut b, 2, 1),
        }
        assert!(Stm::parse(&b).is_err(), "mutation {mutation}");
    }
    let b = fixture(Layout::Legacy);
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_templates: 1,
            ..Limits::default()
        },
        Limits {
            max_validation_steps: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Stm::parse_with_limits(&b, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
}

#[test]
fn truncations_and_deterministic_mutations_do_not_panic() {
    let b = fixture(Layout::Dawntrail);
    // Trailing unreferenced bytes may be removed, but no referenced byte may be.
    for n in 0..b.len() - 16 {
        assert!(Stm::parse(&b[..n]).is_err());
    }
    let mut state = 0x1749u32;
    for _ in 0..4096 {
        let mut sample = b.clone();
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let at = state as usize % sample.len();
        sample[at] ^= (state >> 16) as u8;
        let _ = Stm::parse(&sample);
    }
}

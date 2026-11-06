use icexiv_data::sgb::{Asset, DependencyKind, Error, Limits, Sgb};
fn word(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn pointer(b: &mut [u8], at: usize, origin: usize, target: usize) {
    word(b, at, (target as i64 - origin as i64) as i32 as u32);
}
fn string(b: &mut [u8], at: usize, v: &[u8]) {
    b[at..at + v.len()].copy_from_slice(v);
    b[at + v.len()] = 0;
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0; 1024];
    b[..4].copy_from_slice(b"SGB1");
    word(&mut b, 4, 1024);
    word(&mut b, 8, 1);
    b[12..16].copy_from_slice(b"SCN1");
    word(&mut b, 16, 72);
    pointer(&mut b, 20, 20, 120);
    word(&mut b, 24, 1);
    word(&mut b, 120, 7);
    pointer(&mut b, 124, 120, 700);
    pointer(&mut b, 128, 120, 160);
    word(&mut b, 132, 2);
    pointer(&mut b, 160, 160, 200);
    pointer(&mut b, 164, 160, 500);
    for (p, id, name, table, count) in [(200, 11, 710, 260, 2), (500, 12, 720, 552, 0)] {
        word(&mut b, p, id);
        pointer(&mut b, p + 4, p, name);
        pointer(&mut b, p + 8, p, table);
        word(&mut b, p + 12, count);
    }
    pointer(&mut b, 260, 260, 400);
    pointer(&mut b, 264, 260, 300);
    for (p, typ, id) in [(400, 1, 21), (300, 6, 22)] {
        word(&mut b, p, typ);
        word(&mut b, p + 4, id);
        pointer(&mut b, p + 8, p, 730);
        for (i, v) in [2f32, 3., 4., 0., -0., 0., 1., 1., 1.]
            .into_iter()
            .enumerate()
        {
            word(&mut b, p + 12 + i * 4, v.to_bits());
        }
    }
    pointer(&mut b, 448, 400, 800);
    pointer(&mut b, 452, 400, 900);
    pointer(&mut b, 348, 300, 850);
    string(&mut b, 700, b"group");
    string(&mut b, 710, b"layer1");
    string(&mut b, 720, b"layer2");
    string(&mut b, 730, b"instance");
    string(&mut b, 800, b"bgcommon/model.mdl");
    string(&mut b, 850, b"bgcommon/child.sgb");
    string(&mut b, 940, b"bgcommon/DECOY.mdl");
    b
}
#[test]
fn nested_relative_tables_preserve_order_and_only_structural_dependencies() {
    let b = fixture();
    let f = Sgb::parse(&b).unwrap();
    assert_eq!(f.group.offset, 120);
    assert_eq!(f.group.id, 7);
    assert_eq!(f.group.name, b"group");
    assert_eq!(f.group.layers.len(), 2);
    assert!(f.group.layers[1].instances.is_empty());
    let i = &f.group.layers[0].instances[0];
    assert_eq!(i.offset, 400);
    assert_eq!(i.id, 21);
    assert_eq!(i.transform.translation, [2., 3., 4.]);
    assert_eq!(i.transform.rotation[1].to_bits(), (-0f32).to_bits());
    assert_eq!(i.transform.scale, [1.; 3]);
    assert_eq!(i.raw_header.len(), 48);
    assert!(
        matches!(i.asset,Asset::Background{model_path,collision_path}if model_path==b"bgcommon/model.mdl" && collision_path.is_empty())
    );
    let d = f.dependencies().collect::<Vec<_>>();
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].kind, DependencyKind::Model);
    assert_eq!(d[1].kind, DependencyKind::SharedGroup);
    assert_eq!(d[1].instance_offset, 300);
    assert_eq!(d[1].instance_index, 1);
    assert!(!d.iter().any(|d| d.path.windows(5).any(|w| w == b"DECOY")));
    assert_eq!(f.raw_bytes(), b);
}
#[test]
fn unknown_instances_are_retained_not_confirmed_asset_absence() {
    let mut b = fixture();
    word(&mut b, 400, 123456);
    let f = Sgb::parse(&b).unwrap();
    assert_eq!(f.other_instances().count(), 1);
    assert_eq!(f.dependencies().count(), 1);
    assert!(matches!(
        f.group.layers[0].instances[0].asset,
        Asset::Other(123456)
    ));
}
#[test]
fn signed_table_entries_and_controlled_noncontiguous_layout() {
    let mut b = fixture();
    pointer(&mut b, 208, 200, 680);
    pointer(&mut b, 680, 680, 400);
    pointer(&mut b, 684, 680, 300);
    let f = Sgb::parse(&b).unwrap();
    assert_eq!(f.group.layers[0].instances[0].offset, 400);
    assert_eq!(f.group.layers[0].instances[1].offset, 300);
    pointer(&mut b, 124, 120, 100);
    assert!(Sgb::parse(&b).is_err());
}
#[test]
fn structural_offsets_strings_profiles_and_budgets_are_checked() {
    let b = fixture();
    for n in 0..b.len() {
        assert!(Sgb::parse(&b[..n]).is_err());
    }
    for (at, v) in [
        (4, 100),
        (8, 2),
        (16, 0),
        (24, 2),
        (128, u32::MAX / 2),
        (260, u32::MAX / 2),
        (448, 20000),
        (412, f32::NAN.to_bits()),
    ] {
        let mut bad = b.clone();
        word(&mut bad, at, v);
        assert!(Sgb::parse(&bad).is_err());
    }
    let mut bad = b.clone();
    bad[800..].fill(b'a');
    assert!(Sgb::parse(&bad).is_err());
    for limits in [
        Limits {
            max_input_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_allocation_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_layers: 0,
            ..Limits::default()
        },
        Limits {
            max_instances: 0,
            ..Limits::default()
        },
        Limits {
            max_string_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_work: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Sgb::parse_with_limits(&b, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
    let mut state = 0x138ac1u32;
    for _ in 0..8192 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut data = b.clone();
        let at = state as usize % data.len();
        data[at] ^= (state >> 16) as u8;
        if let Ok(f) = Sgb::parse(&data) {
            let _ = f.dependencies().count();
            let _ = f.other_instances().count();
        }
    }
}

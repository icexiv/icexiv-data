use icexiv_data::avfx::{AssetLimits, BinderSlot, Block, Error, Kind, Node};
fn block(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut tag = [0; 4];
    tag[..name.len()].copy_from_slice(name.as_bytes());
    tag.reverse();
    let mut b = tag.to_vec();
    b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    b.extend_from_slice(payload);
    b.resize(b.len() + payload.len().wrapping_neg() % 4, 0);
    b
}
fn int(name: &str, v: i32) -> Vec<u8> {
    block(name, &v.to_le_bytes())
}
fn node(b: &[u8]) -> Node<'_> {
    let block = Block {
        name: *b"Bind",
        payload: b,
        offset: 0,
        padding: &[],
    };
    Node {
        kind: Kind::Binder,
        block,
        fields: block.fields(4096).unwrap(),
    }
}
fn fixture() -> Vec<u8> {
    let p = [
        int("BPT", 0),
        int("BPTP", 3),
        int("BPID", 5),
        block("Name", b"null\0\0\0\0"),
        int("CoUF", -1),
        int("bRng", 0),
        block("RnPY", &(-0f32).to_le_bytes()),
        block("Pos", &int("ACT", 0)),
        block("ZZZZ", &[1, 2]),
    ]
    .concat();
    block("PrpS", &p)
}
#[test]
fn coordinate_update_cutoff_is_signed_and_inclusive() {
    let b = fixture();
    let p = node(&b).binder_properties(AssetLimits::default()).unwrap()[0];
    assert_eq!(p.coordinate_refresh_allowed(240.).unwrap(), Some(true));
    let mut p = p;
    p.coordinate_update_frame = Some(0x10002);
    assert_eq!(p.coordinate_refresh_allowed(2.).unwrap(), Some(true));
    assert_eq!(p.coordinate_refresh_allowed(2.25).unwrap(), Some(false));
    p.coordinate_update_frame = None;
    assert_eq!(p.coordinate_refresh_allowed(0.).unwrap(), None);
    assert!(p.coordinate_refresh_allowed(f32::NAN).is_err());
}

#[test]
fn random_channel_draw_lifetime_and_seed_sequence() {
    use icexiv_data::avfx::{Curve, CurveRng};
    let seed = CurveRng([1, 2, 3, 4]);
    let mut steps = seed;
    assert_eq!(
        std::array::from_fn::<_, 7, _>(|_| steps.next_u16()),
        [2061, 6175, 4, 8224, 77, 8378, 142]
    );
    for mode in 0..=5 {
        let key = [
            0x00010000u32,
            1f32.to_bits(),
            1f32.to_bits(),
            0.5f32.to_bits(),
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
        let bytes = [
            int("RanT", mode),
            int("BvPr", 0),
            int("BvPo", 0),
            block("Keys", &key),
        ]
        .concat();
        let b = Block {
            name: *b"SclR",
            offset: 0,
            padding: &[],
            payload: &bytes,
        };
        let c = Curve::parse(b, 4)
            .unwrap()
            .random_amplitude(1, 128)
            .unwrap();
        let mut rng = seed;
        let inst = c.instantiate(&mut rng);
        let initial = rng;
        let a = inst.sample(0., 2., &mut rng).unwrap();
        let b = inst.sample(16., 2., &mut rng).unwrap();
        if mode < 3 {
            assert_ne!(initial, seed);
            assert_eq!(rng, initial);
            assert_eq!(a, b);
            let expected = [1.535, 2.015, 1.52][mode as usize];
            assert!((a - expected).abs() < 1e-6);
        } else {
            assert_eq!(initial, seed);
            assert_ne!(a, b);
            let u = 2061. / 65535.;
            let expected = match mode {
                3 => 1.5 + u,
                4 => 2. + 0.5 * u,
                _ => 1.5 + 0.5 * u,
            };
            assert!((a - expected).abs() < 1e-6);
            assert_eq!(rng.0, [3, 4, 0x80d, 0x181f]);
        }
    }
}

#[test]
fn rgb_uses_three_color_lanes_and_its_own_repeat_endpoints() {
    use icexiv_data::avfx::Curve;
    let mut keys = Vec::new();
    for (t, kind, v) in [
        (0i16, 1u16, [0f32, 2., 4.]),
        (4, 0, [4., 0., -4.]),
        (8, 2, [1., 3., 5.]),
    ] {
        keys.extend(t.to_le_bytes());
        keys.extend(kind.to_le_bytes());
        for c in v {
            keys.extend(c.to_le_bytes());
        }
    }
    let bytes = [int("BvPr", 1), int("BvPo", 2), block("Keys", &keys)].concat();
    let raw = Block {
        name: *b"test",
        offset: 0,
        padding: &[],
        payload: &bytes,
    };
    let c = Curve::parse(raw, 3).unwrap();
    assert!(c.rgb(2).is_err());
    let c = c.rgb(3).unwrap();
    for (f, v) in [
        (0., [1., 3., 5.]),
        (2., [2., 1., 0.]),
        (6., [4., 0., -4.]),
        (8., [0., 2., 4.]),
        (10., [2., 1., 0.]),
    ] {
        assert_eq!(c.sample(f).unwrap(), v);
    }
    assert!(c.sample(f32::NAN).is_err());
}

#[test]
fn scalar_uses_destination_segment_kind_signed_handles_and_exterior_periods() {
    use icexiv_data::avfx::Curve;
    let build = |keys: &[(i16, u16, [f32; 3])]| {
        let mut b = Vec::new();
        for (t, k, vs) in keys {
            b.extend(t.to_le_bytes());
            b.extend(k.to_le_bytes());
            for v in vs {
                b.extend(v.to_le_bytes());
            }
        }
        [
            int("BvPr", 1),
            int("BvPo", 2),
            int("RanT", 0),
            block("Keys", &b),
        ]
        .concat()
    };
    let eval = |bytes: &[u8]| {
        let b = Block {
            name: *b"test",
            offset: 0,
            padding: &[],
            payload: bytes,
        };
        Curve::parse(b, 4)?.scalar(32, 1024)
    };
    let raw = build(&[
        (0, 0, [1., 1., 4.]),
        (4, 2, [1., 1., 12.]),
        (8, 1, [1., 1., 20.]),
    ]);
    let c = eval(&raw).unwrap();
    for (f, v) in [
        (2., 4.),
        (4., 12.),
        (6., 16.),
        (10., 20.),
        (-2., 16.),
        (-8., 20.),
        (16., 36.),
    ] {
        assert_eq!(c.sample(f).unwrap(), v);
    }
    let h = eval(&build(&[(0, 1, [0., 1., 0.]), (4, 0, [1., 0., 8.])])).unwrap();
    assert_eq!(h.sample(1.).unwrap(), 1.25); // zero tangents: smoothstep(1/4)*8
    let h = eval(&build(&[
        (0, 0, [1., 1., 2.]),
        (4, 0, [1., 1., 10.]),
        (10, 0, [1., 1., 22.]),
    ]))
    .unwrap();
    assert_eq!(h.sample(7.).unwrap(), 16.);
    assert!(eval(&build(&[(0, 1, [1.; 3]), (0, 2, [1.; 3])])).is_err());
}

#[test]
fn host_frame_keeps_axis_scale_separate_from_follow() {
    use icexiv_data::avfx::BinderTransformFlags as Flags;
    let axes = [[0., 2., 0.], [-3., 0., 0.], [0., 0., 0.]];
    let pos = [7., 8., 9.];
    let f = Flags {
        follow: Some(255),
        scale: Some(1),
    }
    .host_frame(axes, pos)
    .unwrap();
    assert_eq!(f.translation, pos);
    assert_eq!(f.axes, [[0., 1., 0.], [-1., 0., 0.], [0.; 3]]);
    assert_eq!(f.scale, [2., 3., f32::EPSILON]);
    let f = Flags {
        follow: Some(0),
        scale: Some(1),
    }
    .host_frame(axes, pos)
    .unwrap();
    assert_eq!(f.axes, [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]);
    assert_eq!(f.scale, [2., 3., 0.]);
    let f = Flags {
        follow: None,
        scale: None,
    }
    .host_frame(axes, pos)
    .unwrap();
    assert_eq!(f.scale, [1.; 3]);
    assert_eq!(f.axes[0], [0., 1., 0.]);
}

#[test]
fn constant_point_profile_is_not_a_fallback_for_other_motion() {
    let key = [
        0x00010000u32,
        2f32.to_bits(),
        2f32.to_bits(),
        2f32.to_bits(),
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    let profile = |random, extra: &[u8]| {
        let curve = [
            int("KeyC", 1),
            int("RanT", random),
            int("BvPo", 0),
            int("BvPr", 0),
            block("Keys", &key),
        ]
        .concat();
        [
            int("BnVr", 0),
            block("bFTO", &[255]),
            block("bTSc", &[1]),
            block("Data", &[block("SpS", &curve), extra.to_vec()].concat()),
        ]
        .concat()
    };
    let data = profile(0, &[]);
    let n = node(&data);
    assert_eq!(n.constant_point_sps(6).unwrap(), Some(2.));
    assert_eq!(
        n.point_sps_curve(AssetLimits::default())
            .unwrap()
            .unwrap()
            .sample(25.)
            .unwrap(),
        2.
    );
    assert!(n.constant_point_sps(5).is_err());
    assert_eq!(n.binder_transform_flags().unwrap().follow, Some(255));
    assert_eq!(n.binder_transform_flags().unwrap().scale, Some(1));
    for b in [profile(1, &[]), profile(0, &block("SpSR", &[])), Vec::new()] {
        assert_eq!(node(&b).constant_point_sps(10).unwrap(), None);
    }
    assert!(node(&int("bFTO", 1)).binder_transform_flags().is_err());
}
#[test]
fn selector_fields_names_and_missing_slots_are_not_runtime_defaults() {
    let bytes = fixture();
    let props = node(&bytes)
        .binder_properties(AssetLimits::default())
        .unwrap();
    assert_eq!(props.len(), 1);
    let p = props[0];
    assert_eq!(p.slot, BinderSlot::Start);
    assert_eq!(p.bind_point, Some(0));
    assert_eq!(p.target_point, Some(3));
    assert_eq!(p.bind_point_id, Some(5));
    assert_eq!(p.name, Some(&b"null"[..]));
    assert_eq!(p.coordinate_update_frame, Some(-1));
    assert_eq!(p.ring.enabled, Some(0));
    assert_eq!(p.ring.position[1].unwrap().to_bits(), (-0f32).to_bits());
    assert_eq!(p.ring.position[0], None);
    assert_eq!(p.position.unwrap().len(), 1);
    assert_eq!(p.fields.one(b"ZZZZ").unwrap().unwrap().payload, [1, 2]);
}
#[test]
fn absent_and_encoded_empty_names_are_distinct() {
    for (name, expected) in [(Vec::new(), None), (block("Name", &[]), Some(&[][..]))] {
        let data = block("PrpG", &[name, int("BPID", -1)].concat());
        let p = node(&data)
            .binder_properties(AssetLimits::default())
            .unwrap();
        assert_eq!(p[0].slot, BinderSlot::Goal);
        assert_eq!(p[0].name, expected);
        assert_eq!(p[0].bind_point_id, Some(-1));
    }
}
#[test]
fn malformed_slots_scalars_and_names_and_budgets_fail_explicitly() {
    let data = fixture();
    let n = node(&data);
    for limits in [
        AssetLimits {
            max_blocks: 0,
            ..AssetLimits::default()
        },
        AssetLimits {
            max_items: 0,
            ..AssetLimits::default()
        },
        AssetLimits {
            max_output_bytes: 0,
            ..AssetLimits::default()
        },
        AssetLimits {
            max_work: 0,
            ..AssetLimits::default()
        },
    ] {
        assert!(matches!(
            n.binder_properties(limits),
            Err(Error::BudgetExceeded)
        ));
    }
    for p in [
        block("Name", b"unterminated"),
        block("Name", b"a\0bad"),
        block("BPID", &[0]),
        [int("BPID", 1), int("BPID", 2)].concat(),
    ] {
        let b = block("PrpS", &p);
        assert!(node(&b).binder_properties(AssetLimits::default()).is_err());
    }
    let duplicate = [data.clone(), data.clone()].concat();
    assert!(
        node(&duplicate)
            .binder_properties(AssetLimits::default())
            .is_err()
    );
    let mut state = 0x123facu32;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut bytes = data.clone();
        let at = state as usize % bytes.len();
        bytes[at] ^= (state >> 16) as u8;
        let b = Block {
            name: *b"Bind",
            payload: &bytes,
            offset: 0,
            padding: &[],
        };
        if let Ok(fields) = b.fields(4096) {
            let n = Node {
                kind: Kind::Binder,
                block: b,
                fields,
            };
            let _ = n.binder_properties(AssetLimits::default());
        }
    }
}

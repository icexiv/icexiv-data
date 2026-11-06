use icexiv_data::avfx::{
    AssetLimits, Avfx, DataKind, DataView, Error, Kind, ParameterValue, ReferenceKind,
    ReferenceStatus,
};
fn block(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut wire = [0; 4];
    wire[..name.len()].copy_from_slice(name.as_bytes());
    wire.reverse();
    let mut out = wire.to_vec();
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out.resize(out.len() + payload.len().wrapping_neg() % 4, 0);
    out
}
fn int(name: &str, v: i32) -> Vec<u8> {
    block(name, &v.to_le_bytes())
}
fn item(target: i32) -> Vec<u8> {
    [
        int("bEnb", 1),
        int("TgtB", target),
        int("CrTm", 2),
        int("CrCn", 3),
        int("CrPr", 75),
    ]
    .concat()
}
fn fixture() -> Vec<u8> {
    let mut root = Vec::new();
    for (t, n) in [
        ("ScCn", 0),
        ("TlCn", 0),
        ("EmCn", 1),
        ("PrCn", 1),
        ("EfCn", 0),
        ("BdCn", 0),
        ("TxCn", 1),
        ("MdCn", 1),
    ] {
        root.extend(int(t, n));
    }
    root.extend(block("Tex", b"vfx/test.atex\0"));
    root.extend(block("Modl", &[]));
    let emitter = [
        int("EVT", 5),
        int("EmCn", 1),
        int("PrCn", 1),
        int("EfNo", -1),
        block("ItEm", &item(0)),
        block("ItPr", &[item(0), item(0)].concat()),
        block(
            "Data",
            &[int("MdNo", 0), int("ROT", 2), int("GeMT", 3)].concat(),
        ),
    ]
    .concat();
    root.extend(block("Emit", &emitter));
    let mut key = vec![0, 0, 1, 0];
    for f in [0f32, 0., 0.] {
        key.extend_from_slice(&f.to_le_bytes());
    }
    let txcurve = [int("KeyC", 1), block("Keys", &key)].concat();
    let slot = [
        int("bEna", 0),
        int("TxNo", 5),
        int("UvSN", 0),
        block("TLst", &[0, 0]),
        block("TxN", &txcurve),
    ]
    .concat();
    let particle = [
        int("PrVT", 5),
        block(
            "Data",
            &[block("MdNo", &[0, 2]), int("MNRv", 0), block("UNKN", &[17])].concat(),
        ),
        block("TC1", &slot),
    ]
    .concat();
    root.extend(block("Ptcl", &particle));
    block("AVFX", &root)
}
#[test]
fn typed_data_byte_model_lists_and_cumulative_cross_kind_targets() {
    let b = fixture();
    let a = Avfx::parse(&b).unwrap();
    let emitter = *a.nodes_of(Kind::Emitter).next().unwrap();
    let items = emitter.emitter_items(AssetLimits::default()).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].target_kind, Kind::Emitter);
    assert_eq!(items[1].target_kind, Kind::Particle);
    assert_eq!(items[1].create_count, Some(3));
    let particle = *a.nodes_of(Kind::Particle).next().unwrap();
    let DataView::Known {
        kind, parameters, ..
    } = particle.data_view(AssetLimits::default()).unwrap()
    else {
        panic!("known Data")
    };
    assert_eq!(kind, DataKind::ParticleModel);
    assert!(matches!(parameters[0].value,ParameterValue::Bytes(v)if v==[0,2]));
    assert!(matches!(parameters[2].value,ParameterValue::Unknown(v)if v.payload==[17]));
    let slots = particle.texture_slots(AssetLimits::default()).unwrap();
    assert_eq!(slots[0].enabled, Some(0));
    assert_eq!(slots[0].texture_index, Some(5));
    assert_eq!(slots[0].mask_indices, Some(&[0, 0][..]));
    assert_eq!(slots[0].texture_curve.unwrap().keys().len(), 1);
    let report = a.asset_report(AssetLimits::default()).unwrap();
    assert_eq!(report.animated_texture_curves, 1);
    assert!(
        report
            .references
            .iter()
            .any(|r| r.kind == ReferenceKind::Texture
                && r.raw_index == 5
                && r.status == ReferenceStatus::OutOfRange)
    );
    assert!(
        report
            .references
            .iter()
            .any(|r| r.raw_index == -1 && r.status == ReferenceStatus::Negative)
    );
    assert!(
        report
            .references
            .iter()
            .any(|r| r.kind == ReferenceKind::Model
                && r.raw_index == 2
                && r.status == ReferenceStatus::OutOfRange)
    );
}
#[test]
fn unknown_data_does_not_get_a_default_type_or_fake_identity() {
    let b = fixture();
    let mut a = Avfx::parse(&b).unwrap();
    let n = a
        .nodes
        .iter_mut()
        .find(|n| n.kind == Kind::Particle)
        .unwrap();
    n.kind = Kind::Binder;
    assert!(matches!(
        n.data_view(AssetLimits::default()).unwrap(),
        DataView::Unknown { variant: None, .. }
    ));
}

#[test]
fn model_selection_width_is_preserved_and_invalid_width_is_not_identity() {
    use icexiv_data::avfx::{Block, Node};
    for raw in [&[2u8][..], &[2u8, 0, 0, 0][..], &[2u8, 0, 0][..]] {
        let payload = [int("PrVT", 13), block("Data", &block("MNO", raw))].concat();
        let b = Block {
            name: *b"Ptcl",
            payload: &payload,
            offset: 0,
            padding: &[],
        };
        let n = Node {
            kind: Kind::Particle,
            block: b,
            fields: b.fields(100).unwrap(),
        };
        let result = n.data_view(AssetLimits::default());
        if raw.len() == 3 {
            assert!(result.is_err());
        } else {
            let DataView::Known {
                kind, parameters, ..
            } = result.unwrap()
            else {
                panic!("light model")
            };
            assert_eq!(kind, DataKind::ParticleLightModel);
            if raw.len() == 1 {
                assert!(matches!(parameters[0].value, ParameterValue::Byte(2)));
            } else {
                assert!(matches!(parameters[0].value, ParameterValue::Integer(2)));
            }
        }
    }
}

#[test]
fn interleaved_particle_first_items_use_their_outer_tags_not_a_fixed_prefix() {
    use icexiv_data::avfx::{Block, Node};
    let p0 = item(7);
    let e0 = item(3);
    let p1 = item(8);
    let payload = [
        int("EmCn", 1),
        int("PrCn", 2),
        block("ItPr", &p0),
        block("ItEm", &[p0.clone(), e0.clone()].concat()),
        block("ItPr", &[p0, e0, p1].concat()),
    ]
    .concat();
    let b = Block {
        name: *b"Emit",
        payload: &payload,
        offset: 0,
        padding: &[],
    };
    let n = Node {
        kind: Kind::Emitter,
        block: b,
        fields: b.fields(100).unwrap(),
    };
    let items = n.emitter_items(AssetLimits::default()).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|v| (v.target_kind, v.target_index))
            .collect::<Vec<_>>(),
        [
            (Kind::Particle, Some(7)),
            (Kind::Emitter, Some(3)),
            (Kind::Particle, Some(8))
        ]
    );
}

#[test]
fn model_skin_groups_decal_floats_and_raw_quad_flags_are_distinct() {
    use icexiv_data::avfx::{Block, Node};
    for (variant, expected, tag, value) in [
        (
            14,
            DataKind::ParticleModelSkin,
            "AuTT",
            7i32.to_le_bytes().to_vec(),
        ),
        (
            10,
            DataKind::ParticleDecal,
            "SS",
            1f32.to_le_bytes().to_vec(),
        ),
        (8, DataKind::ParticleQuad, "bMP", vec![255]),
    ] {
        let payload = [int("PrVT", variant), block("Data", &block(tag, &value))].concat();
        let b = Block {
            name: *b"Ptcl",
            payload: &payload,
            offset: 0,
            padding: &[],
        };
        let n = Node {
            kind: Kind::Particle,
            block: b,
            fields: b.fields(100).unwrap(),
        };
        let DataView::Known {
            kind, parameters, ..
        } = n.data_view(AssetLimits::default()).unwrap()
        else {
            panic!("known type")
        };
        assert_eq!(kind, expected);
        match variant {
            14 => assert!(matches!(parameters[0].value, ParameterValue::Integer(7))),
            10 => assert!(matches!(parameters[0].value, ParameterValue::Real(1.))),
            8 => assert!(matches!(parameters[0].value, ParameterValue::Byte(255))),
            _ => unreachable!(),
        }
    }
}
#[test]
fn budgets_truncations_and_mutations_are_bounded() {
    let b = fixture();
    let a = Avfx::parse(&b).unwrap();
    for l in [
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
        assert!(matches!(a.asset_report(l), Err(Error::BudgetExceeded)));
    }
    for n in 0..b.len() {
        assert!(Avfx::parse(&b[..n]).is_err());
    }
    let mut state = 0x8ac789u32;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut bad = b.clone();
        let at = state as usize % bad.len();
        bad[at] ^= (state >> 16) as u8;
        if let Ok(a) = Avfx::parse(&bad) {
            let _ = a.asset_report(AssetLimits {
                max_items: 4096,
                max_output_bytes: 1 << 20,
                max_work: 65536,
                ..AssetLimits::default()
            });
        }
    }
}

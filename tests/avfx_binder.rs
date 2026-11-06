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

use icexiv_data::avfx::{Avfx, Block, Curve, Error, Kind, Limits};
fn block(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut tag = [0; 4];
    tag[..name.len()].copy_from_slice(name.as_bytes());
    let mut b = tag.into_iter().rev().collect::<Vec<_>>();
    b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    b.extend_from_slice(payload);
    b.resize(b.len() + payload.len().wrapping_neg() % 4, 0);
    b
}
fn scalar(name: &str, v: u32) -> Vec<u8> {
    block(name, &v.to_le_bytes())
}
fn fixture() -> Vec<u8> {
    let mut root = Vec::new();
    for (tag, count) in [
        ("ScCn", 1),
        ("TlCn", 0),
        ("EmCn", 0),
        ("PrCn", 0),
        ("EfCn", 0),
        ("BdCn", 0),
        ("TxCn", 1),
        ("MdCn", 1),
    ] {
        root.extend(scalar(tag, count));
    }
    let row = [
        scalar("bEna", 1),
        scalar("StTm", 2),
        scalar("TlNo", u32::MAX),
    ]
    .concat();
    let scheduler = [
        scalar("ItCn", 2),
        block("Item", &row),
        block("Item", &[row.clone(), row].concat()),
    ]
    .concat();
    root.extend(block("Schd", &scheduler));
    root.extend(block("Tex", b"vfx/test.atex\0"));
    let mut vertex = vec![0; 36];
    vertex[6..8].copy_from_slice(&0x3c00u16.to_le_bytes());
    vertex[8..12].copy_from_slice(&[0, 127, 128, 255]);
    vertex[16..20].copy_from_slice(&[1, 2, 3, 4]);
    vertex[20..22].copy_from_slice(&0xbc00u16.to_le_bytes());
    let mut emission = Vec::new();
    for v in [1f32, 2., 3., 0., 1., 0.] {
        emission.extend_from_slice(&v.to_le_bytes());
    }
    emission.extend_from_slice(&[9, 8, 7, 6]);
    let model = [
        block("VDrw", &vertex),
        block("VIdx", &[0; 6]),
        block("VEmt", &emission),
        block("VNum", &u16::MAX.to_le_bytes()),
    ]
    .concat();
    root.extend(block("Modl", &model));
    root.extend(block("ZZZZ", &[1, 2, 3]));
    block("AVFX", &root)
}
#[test]
fn borrowed_assets_geometry_unknown_payloads_and_cumulative_schedules() {
    let b = fixture();
    let a = Avfx::parse(&b).unwrap();
    assert_eq!(a.textures, [b"vfx/test.atex" as &[u8]]);
    assert_eq!(a.global.one(b"ZZZZ").unwrap().unwrap().payload, [1, 2, 3]);
    let m = a.models[0];
    let v = m.draw_vertices().next().unwrap();
    assert_eq!(v.position, [0., 0., 0., 1.]);
    assert_eq!(v.normal, [-128, -1, 0, 127]);
    assert_eq!(v.uv[0], [-1., 0.]);
    assert_eq!(v.color, [1, 2, 3, 4]);
    assert_eq!(m.indices().collect::<Vec<_>>(), [0, 0, 0]);
    let e = m.emit_vertices().next().unwrap();
    assert_eq!(e.position, [1., 2., 3.]);
    assert_eq!(e.normal, [0., 1., 0.]);
    assert_eq!(e.color, [9, 8, 7, 6]);
    assert_eq!(m.emit_numbers().next(), Some(u16::MAX));
    let n = a.nodes_of(Kind::Scheduler).next().unwrap();
    let rows = n.scheduler_items(false, 100).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].timeline_index, -1);
    assert_eq!(rows[0].start_time, 2);
    assert!(n.scheduler_items(false, 0).is_err());
}
#[test]
fn checked_curve_records_and_signed_time() {
    let mut k = vec![0xff, 0xff, 2, 0];
    for v in [1f32, 2., 3.] {
        k.extend_from_slice(&v.to_le_bytes());
    }
    let data = [scalar("KeyC", 1), block("Keys", &k)].concat();
    let b = Block {
        name: *b"\0\0\0X",
        payload: &data,
        offset: 0,
        padding: &[],
    };
    let c = Curve::parse(b, 10).unwrap();
    let key = c.keys().next().unwrap();
    assert_eq!(key.time, -1);
    assert_eq!(key.kind, 2);
    assert_eq!(key.values, [1., 2., 3.]);
    assert!(b.is(b"X"));
    assert!(b.fields(0).is_err());
    assert!(b.u32().is_err());
}
#[test]
fn malformed_and_resource_limited_inputs() {
    let b = fixture();
    for n in 0..b.len() {
        assert!(Avfx::parse(&b[..n]).is_err());
    }
    for limits in [
        Limits {
            max_input_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_blocks: 0,
            ..Limits::default()
        },
        Limits {
            max_allocation_bytes: 0,
            ..Limits::default()
        },
        Limits {
            max_work: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Avfx::parse_with_limits(&b, limits),
            Err(Error::BudgetExceeded)
        ));
    }
    let mut state = 0x9814u32;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut data = b.clone();
        let at = state as usize % data.len();
        data[at] ^= (state >> 16) as u8;
        if let Ok(a) = Avfx::parse(&data) {
            for m in a.models {
                let _ = m.draw_vertices().count();
            }
            for n in a.nodes {
                let _ = n.scheduler_items(false, 4096);
            }
        }
    }
}

#[test]
fn timeline_references_and_fixed_clip_keep_signed_and_raw_values() {
    use icexiv_data::avfx::{Clip, Node};
    let row = [
        scalar("bEna", 1),
        scalar("StTm", 0),
        scalar("EdTm", 20),
        scalar("BdNo", (-2i32) as u32),
        scalar("EfNo", u32::MAX),
        scalar("EmNo", 3),
        scalar("Plfm", 0),
        scalar("ClNo", 2),
    ]
    .concat();
    let data = block("Item", &row);
    let b = Block {
        name: *b"TmLn",
        payload: &data,
        offset: 0,
        padding: &[],
    };
    let n = Node {
        kind: Kind::Timeline,
        block: b,
        fields: b.fields(100).unwrap(),
    };
    let rows = n.timeline_items(100).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].binder_index, -2);
    assert_eq!(rows[0].effector_index, -1);
    assert_eq!(rows[0].emitter_index, 3);
    assert_eq!(rows[0].clip_index, Some(2));
    let mut bytes = vec![0; 164];
    bytes[..4].copy_from_slice(b"TSET");
    bytes[4..8].copy_from_slice(&(-1i32).to_le_bytes());
    bytes[20..24].copy_from_slice(&2f32.to_le_bytes());
    bytes[36..40].copy_from_slice(b"name");
    let b = Block {
        name: *b"Clip",
        payload: &bytes,
        offset: 0,
        padding: &[],
    };
    let clip = Clip::parse(b).unwrap();
    assert_eq!(clip.tag, *b"TEST");
    assert_eq!(clip.integers[0], -1);
    assert_eq!(clip.reals[0], 2.);
    assert_eq!(clip.names[0], b"name");
    assert!(
        Clip::parse(Block {
            payload: &bytes[..163],
            ..b
        })
        .is_err()
    );
}

use icexiv_data::pbd::{Error, Limits, Matrix, Pbd};
fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn base() -> Vec<u8> {
    let mut b = vec![0; 108];
    put32(&mut b, 0, 2);
    put16(&mut b, 4, 101);
    put16(&mut b, 6, 1);
    put32(&mut b, 12, 1f32.to_bits());
    put16(&mut b, 16, 201);
    put32(&mut b, 20, 44);
    put32(&mut b, 24, 2f32.to_bits());
    for (at, v) in [
        (28, 1),
        (30, 65535),
        (32, 65535),
        (34, 1),
        (36, 65535),
        (38, 0),
        (40, 65535),
    ] {
        put16(&mut b, at, v);
    }
    put32(&mut b, 44, 1);
    put16(&mut b, 48, 56);
    let rows = [[2f32, 3., 4., 5.], [6., 7., 8., 9.], [10., 11., 12., 13.]];
    for (i, v) in rows.into_iter().flatten().enumerate() {
        put32(&mut b, 52 + i * 4, v.to_bits());
    }
    b[100..107].copy_from_slice(b"j_root\0");
    b
}
fn packed(inner: &[u8]) -> Vec<u8> {
    // Hand-authored standard FlatBuffers wire envelope. Root field 2 is a
    // table vector; entry fields 0/1/2/3 are child/parent/conditions/PBD bytes.
    let mut f = vec![0; 80];
    put32(&mut f, 0, 20);
    f[4..8].copy_from_slice(b"EPBD");
    put16(&mut f, 8, 10);
    put16(&mut f, 10, 8);
    put16(&mut f, 16, 4);
    put32(&mut f, 20, 12);
    put32(&mut f, 24, 4);
    put32(&mut f, 28, 1);
    put32(&mut f, 32, 16);
    put16(&mut f, 36, 12);
    put16(&mut f, 38, 20);
    for (p, v) in [(40, 4), (42, 8), (44, 12), (46, 16)] {
        put16(&mut f, p, v);
    }
    put32(&mut f, 48, 12);
    put32(&mut f, 52, 301);
    put32(&mut f, 56, 101);
    put32(&mut f, 60, 8);
    put32(&mut f, 64, 12);
    put32(&mut f, 68, 1);
    put32(&mut f, 72, 7);
    put32(&mut f, 76, inner.len() as u32);
    f.extend_from_slice(inner);
    wrap_flatbuffer(&f)
}
fn wrap_flatbuffer(f: &[u8]) -> Vec<u8> {
    let mut b = base();
    b.extend_from_slice(b"EPBD");
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&(f.len() as u64).to_le_bytes());
    b.extend_from_slice(f);
    b.extend_from_slice(b"PACK");
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&(-(16 + f.len() as i64)).to_le_bytes());
    b.extend_from_slice(&(40 + f.len() as u64).to_le_bytes());
    b
}
#[test]
fn ancestor_paths_use_inverse_indices_are_budgeted_and_distinguish_absence() {
    let mut b = vec![0; 104];
    put32(&mut b, 0, 5);
    for (i, c) in [2, 4, 0, 3, 1].into_iter().enumerate() {
        put16(&mut b, 4 + i * 12, 101 + i as u16 * 100);
        put16(&mut b, 6 + i * 12, c);
        put32(&mut b, 12 + i * 12, 1f32.to_bits());
    }
    for (i, row) in [
        [4i16, -1, -1, 2],
        [-1, -1, -1, 4],
        [-1, 4, -1, 0],
        [2, -1, -1, 3],
        [2, 0, 3, 1],
    ]
    .into_iter()
    .enumerate()
    {
        for (j, v) in row.into_iter().enumerate() {
            put16(&mut b, 64 + i * 8 + j * 2, v as u16);
        }
    }
    let p = Pbd::parse(&b).unwrap();
    let collect = |d, a, budget| {
        p.ancestor_path(d, a, budget)
            .unwrap()
            .map(|v| v.collect::<Result<Vec<_>, _>>().unwrap())
    };
    assert_eq!(collect(2, 0, 6), Some(vec![2, 1]));
    assert_eq!(collect(3, 0, 4), Some(vec![3]));
    assert_eq!(collect(2, 2, 2), Some(vec![]));
    assert_eq!(collect(2, 3, 8), None);
    assert_eq!(collect(2, 4, 8), None);
    assert_eq!(collect(0, 2, 4), None);
    assert!(matches!(
        p.ancestor_path(2, 0, 5),
        Err(Error::BudgetExceeded("ancestry query"))
    ));
    assert!(matches!(
        p.ancestor_path(2, 4, 7),
        Err(Error::BudgetExceeded("ancestry query"))
    ));
    assert!(matches!(
        p.ancestor_path(2, 2, 1),
        Err(Error::BudgetExceeded(_))
    ));
    assert!(matches!(
        p.ancestor_path(usize::MAX, 0, 1024),
        Err(Error::IndexOutOfBounds)
    ));
    assert!(matches!(
        p.ancestor_path(0, 5, 1024),
        Err(Error::IndexOutOfBounds)
    ));
    let mut path = p.ancestor_path(2, 0, 6).unwrap().unwrap();
    assert_eq!(path.next().unwrap().unwrap(), 2);
    assert_eq!(path.next().unwrap().unwrap(), 1);
    assert!(path.next().is_none());
    assert!(path.next().is_none());
}
#[test]
fn ancestor_paths_reserve_iteration_work_for_long_chains() {
    let n = 64usize;
    let mut b = vec![0; 4 + n * 20];
    put32(&mut b, 0, n as u32);
    for i in 0..n {
        let connection = n - 1 - i;
        put16(&mut b, 4 + i * 12, i as u16);
        put16(&mut b, 6 + i * 12, connection as u16);
        let at = 4 + n * 12 + connection * 8;
        put16(&mut b, at, if i == 0 { u16::MAX } else { (n - i) as u16 });
        put16(
            &mut b,
            at + 2,
            if i + 1 == n {
                u16::MAX
            } else {
                (n - 2 - i) as u16
            },
        );
        put16(&mut b, at + 4, u16::MAX);
        put16(&mut b, at + 6, i as u16);
    }
    let p = Pbd::parse(&b).unwrap();
    assert!(matches!(
        p.ancestor_path(n - 1, 0, 127),
        Err(Error::BudgetExceeded(_))
    ));
    let path = p
        .ancestor_path(n - 1, 0, 128)
        .unwrap()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(path, (1..n).rev().collect::<Vec<_>>());
    assert!(p.ancestor_path(0, n - 1, 4).unwrap().is_none());
}
#[test]
fn affine_composition_is_ordered_and_preserves_shear_and_translation() {
    let a = Matrix([[2., 3., 0., 5.], [0., 1., 4., 6.], [1., 0., 1., 7.]]);
    let b = Matrix([[1., 0., 2., 3.], [4., 1., 0., 2.], [0., 5., 1., 1.]]);
    assert_eq!(
        a.compose(b).0,
        [[14., 3., 4., 17.], [4., 21., 4., 12.], [1., 5., 3., 11.]]
    );
    assert_ne!(a.compose(b), b.compose(a));
    for p in [[0., 0., 0.], [1., -2., 3.], [-4., 5., 0.5]] {
        assert_eq!(
            a.compose(b).transform_point(p),
            a.transform_point(b.transform_point(p))
        );
    }
    let identity = Matrix([[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]]);
    assert_eq!(identity.compose(a), a);
    assert_eq!(a.compose(identity), a);
}
#[test]
fn id_lookup_preserves_scalar_and_absent_payload_without_consumer_defaults() {
    let mut b = base();
    put16(&mut b, 4, 0);
    put16(&mut b, 16, u16::MAX);
    put32(&mut b, 12, 1.52587f32.to_bits());
    let p = Pbd::parse(&b).unwrap();
    let root = p.find_deformer(0).unwrap().unwrap();
    assert!(!root.has_payload());
    assert_eq!(root.unknown_scale.to_bits(), 1.52587f32.to_bits());
    assert_eq!(root.connection_index, 1);
    let last = p.find_deformer(u16::MAX).unwrap().unwrap();
    assert_eq!(last.unknown_scale, 2.);
    assert!(last.has_payload());
    assert!(p.find_deformer(101).unwrap().is_none());
}
#[test]
fn inverse_tables_and_file_matrix_not_rest_pose() {
    let b = base();
    let p = Pbd::parse(&b).unwrap();
    assert_eq!(p.deformer_count(), 2);
    assert!(!p.find_deformer(101).unwrap().unwrap().has_payload());
    let d = p.find_deformer(201).unwrap().unwrap();
    assert_eq!(d.connection_index, 0);
    assert_eq!(p.connection(0).unwrap().parent, Some(1));
    let bone = d.bone(0).unwrap();
    assert_eq!(bone.name, b"j_root");
    assert_eq!(bone.matrix.transform_point([1., 2., 3.]), [25., 53., 81.]);
    assert_eq!(
        bone.matrix.column_major_4x4(),
        [
            2., 6., 10., 0., 3., 7., 11., 0., 4., 8., 12., 0., 5., 9., 13., 1.
        ]
    );
    assert_eq!(d.find_bone(b"j_root").unwrap().unwrap().matrix, bone.matrix);
    assert!(d.find_bone(b"missing").unwrap().is_none());
    assert!(p.find_deformer(999).unwrap().is_none());
    assert!(p.connection(usize::MAX).is_err());
    assert!(d.bone(usize::MAX).is_err());
    assert_eq!(
        Matrix([[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]])
            .transform_point([3., 4., 5.]),
        [3., 4., 5.]
    );
}
#[test]
fn epbd_exact_pair_queries_distinguish_absence_ambiguity_and_budget() {
    let b = packed(&base());
    let ext = Pbd::parse(&b).unwrap().extension().unwrap();
    assert_eq!(
        ext.find_entry(301, 101, 1).unwrap().unwrap().child_race,
        301
    );
    assert!(ext.find_entry(101, 301, 1).unwrap().is_none());
    assert!(ext.find_entry(u32::MAX, u32::MAX, 1).unwrap().is_none());
    for pair in [(301, 101), (0, 0)] {
        assert!(matches!(
            ext.find_entry(pair.0, pair.1, 0),
            Err(Error::BudgetExceeded("EPBD entry query"))
        ));
    }
    let mut f = b[base().len() + 16..b.len() - 24].to_vec();
    // Two vector offsets sharing the same valid entry table are legal wire
    // data. An exact-pair query must not silently choose the first reference.
    f.splice(32..32, [0; 4]);
    put32(&mut f, 28, 2);
    put32(&mut f, 32, 20);
    let b = wrap_flatbuffer(&f);
    let ext = Pbd::parse(&b).unwrap().extension().unwrap();
    assert_eq!(ext.entry_count(), 2);
    assert!(matches!(
        ext.find_entry(301, 101, 1),
        Err(Error::BudgetExceeded("EPBD entry query"))
    ));
    assert!(matches!(
        ext.find_entry(301, 101, 2),
        Err(Error::InvalidData("ambiguous EPBD race pair"))
    ));
    assert!(ext.find_entry(0, 0, 2).unwrap().is_none());

    let mut f = packed(&base());
    // u32 wire identity is preserved even when a native key uses low u16.
    let payload = base().len() + 16;
    put32(&mut f, payload + 52, 0x1234012d);
    let ext = Pbd::parse(&f).unwrap().extension().unwrap();
    assert!(ext.find_entry(301, 101, 1).unwrap().is_none());
    assert_eq!(
        ext.find_entry(0x1234012d, 101, 1)
            .unwrap()
            .unwrap()
            .child_race,
        0x1234012d
    );
}
#[test]
fn epbd_root_scalars_are_u16_not_indirect_tables() {
    let seed = packed(&base());
    let mut f = seed[base().len() + 16..seed.len() - 24].to_vec();
    // Insert two adjacent scalar fields before the entries-vector offset.
    // Later relative offsets move with their targets and remain unchanged.
    f.splice(24..24, [1, 0, 0xef, 0xbe]);
    put16(&mut f, 10, 12);
    put16(&mut f, 12, 4);
    put16(&mut f, 14, 6);
    put16(&mut f, 16, 8);
    let b = wrap_flatbuffer(&f);
    let p = Pbd::parse(&b).unwrap();
    let e = p.extension().unwrap();
    assert_eq!(e.version_tag(), Some(1));
    assert_eq!(e.unknown_root_scalar(), Some(0xbeef));
    assert_eq!(e.entry_count(), 1);
    assert_eq!(e.entry(0).unwrap().child_race, 301);
    assert_eq!(e.entry(0).unwrap().pbd().unwrap().deformer_count(), 2);
    assert_eq!(
        e.entry(0).unwrap().local_skeleton_ids().collect::<Vec<_>>(),
        [7]
    );
    let mut wide = f.clone();
    put32(&mut wide, 76, 0xabcd0007);
    let b = wrap_flatbuffer(&wide);
    let e = Pbd::parse(&b)
        .unwrap()
        .extension()
        .unwrap()
        .entry(0)
        .unwrap();
    assert_eq!(e.conditions().collect::<Vec<_>>(), [0xabcd0007]);
    assert_eq!(e.local_skeleton_ids().collect::<Vec<_>>(), [7]);
    assert!(e.pbd().unwrap().find_deformer(7).unwrap().is_none());
    let mut malformed = f.clone();
    put16(&mut malformed, 12, 11);
    assert!(Pbd::parse(&wrap_flatbuffer(&malformed)).is_err());
    put16(&mut malformed, 12, 4);
    put16(&mut malformed, 14, 11);
    assert!(Pbd::parse(&wrap_flatbuffer(&malformed)).is_err());

    // Exact two-byte fields at the table/input end, with no entries vector.
    let mut f = vec![0; 24];
    put32(&mut f, 0, 16);
    f[4..8].copy_from_slice(b"EPBD");
    put16(&mut f, 8, 8);
    put16(&mut f, 10, 8);
    put16(&mut f, 12, 6);
    put16(&mut f, 14, 4);
    put32(&mut f, 16, 8);
    for version in [0, 1, 2, 255, u16::MAX] {
        put16(&mut f, 20, 0x1234);
        put16(&mut f, 22, version);
        let b = wrap_flatbuffer(&f);
        let e = Pbd::parse(&b).unwrap().extension().unwrap();
        assert_eq!(e.version_tag(), Some(version));
        assert_eq!(e.unknown_root_scalar(), Some(0x1234));
        assert_eq!(e.entry_count(), 0);
        assert!(e.find_entry(0, 0, 0).unwrap().is_none());
        assert!(e.entry(0).is_err());
    }
    f.pop();
    assert!(Pbd::parse(&wrap_flatbuffer(&f)).is_err());
}
#[test]
fn epbd_embedded_tables_and_shared_budgets() {
    let b = packed(&base());
    let p = Pbd::parse(&b).unwrap();
    let e = p.extension().unwrap();
    assert_eq!(e.version_tag(), None);
    assert_eq!(e.unknown_root_scalar(), None);
    assert_eq!(e.entry_count(), 1);
    let v = e.entry(0).unwrap();
    assert_eq!((v.child_race, v.parent_race), (301, 101));
    assert_eq!(v.conditions().collect::<Vec<_>>(), [7]);
    assert_eq!(v.pbd().unwrap().deformer_count(), 2);
    assert!(e.entry(1).is_err());
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_deformers: 3,
            ..Limits::default()
        },
        Limits {
            max_bones: 1,
            ..Limits::default()
        },
        Limits {
            max_validation_steps: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Pbd::parse_with_limits(&b, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
}
#[test]
fn malformed_offsets_graphs_strings_and_flatbuffers_fail_without_panics() {
    let b = packed(&base());
    for n in 112..b.len() {
        assert!(Pbd::parse(&b[..n]).is_err(), "truncated extension at {n}");
    }
    for (at, val, wide) in [
        (0, u32::MAX, true),
        (6, 0, false),
        (20, 4, true),
        (28, 0, false),
        (30, 0, false),
        (32, 0, false),
        (36, 0, false),
        (38, 65535, false),
        (44, u32::MAX, true),
        (48, 8, false),
    ] {
        let mut b = base();
        if wide {
            put32(&mut b, at, val)
        } else {
            put16(&mut b, at, val as u16)
        };
        assert!(Pbd::parse(&b).is_err(), "at {at}");
    }
    let b = base();
    for n in 0..107 {
        assert!(Pbd::parse(&b[..n]).is_err(), "length {n}");
    }
    let b = packed(&base());
    let mut state = 0x4881u32;
    for _ in 0..8192 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut v = b.clone();
        let at = state as usize % v.len();
        v[at] ^= (state >> 16) as u8;
        let _ = Pbd::parse(&v);
    }
    let b = packed(&packed(&base()));
    assert!(Pbd::parse(&b).is_ok());
    assert!(matches!(
        Pbd::parse_with_limits(
            &b,
            Limits {
                max_depth: 2,
                ..Limits::default()
            }
        ),
        Err(Error::BudgetExceeded(_))
    ));
}
#[test]
fn affine_inverse_is_checked_and_does_not_select_a_deformer_route() {
    use icexiv_data::pbd::{Matrix, MatrixInverseError};
    let m = Matrix([[0., 2., 0., 3.], [1., 0., 1., 4.], [0., 0., -4., 5.]]);
    let expected = Matrix([
        [0., 1., 0.25, -5.25],
        [0.5, 0., 0., -1.5],
        [0., 0., -0.25, 1.25],
    ]);
    let inverse = m.try_inverse().unwrap();
    assert_eq!(inverse, expected);
    let identity = Matrix([[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]]);
    assert_eq!(m.compose(inverse), identity);
    assert_eq!(inverse.compose(m), identity);
    assert_eq!(inverse.try_inverse().unwrap(), m);
    assert_eq!(
        inverse.transform_point(m.transform_point([2., -1., 3.])),
        [2., -1., 3.]
    );
    let mut bad = identity;
    bad.0[1] = bad.0[0];
    assert_eq!(bad.try_inverse(), Err(MatrixInverseError::Singular));
    for scalar in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for r in 0..3 {
            for c in 0..4 {
                let mut bad = identity;
                bad.0[r][c] = scalar;
                assert_eq!(bad.try_inverse(), Err(MatrixInverseError::NonFiniteInput));
            }
        }
    }
    let mut tiny = identity;
    tiny.0[0][0] = f32::from_bits(1);
    assert_eq!(
        tiny.try_inverse(),
        Err(MatrixInverseError::OutputOutOfRange)
    );
    tiny.0[0][0] = 1e-30;
    assert!(tiny.try_inverse().unwrap().0[0][0].is_finite());
    let mut huge = identity;
    huge.0[0][0] = 0.5;
    huge.0[0][3] = f32::MAX;
    assert_eq!(
        huge.try_inverse(),
        Err(MatrixInverseError::OutputOutOfRange)
    );
}

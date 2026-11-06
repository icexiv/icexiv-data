use icexiv_data::{
    havok::{Error, Limits, SkeletonLimits, Tagfile, Value},
    sklb::Sklb,
};
fn integer(b: &mut Vec<u8>, value: i64) {
    let mut v = value.unsigned_abs();
    let mut first = ((v & 63) as u8) << 1 | u8::from(value < 0);
    v >>= 6;
    if v != 0 {
        first |= 128;
    }
    b.push(first);
    while v != 0 {
        let mut x = (v & 127) as u8;
        v >>= 7;
        if v != 0 {
            x |= 128;
        }
        b.push(x);
    }
}
fn string(b: &mut Vec<u8>, value: &[u8]) {
    integer(b, value.len() as i64);
    b.extend_from_slice(value);
}
fn class(b: &mut Vec<u8>, name: &[u8], version: i64, members: &[(&[u8], i64, Option<&[u8]>)]) {
    integer(b, 2);
    string(b, name);
    integer(b, version);
    integer(b, 0);
    integer(b, members.len() as i64);
    for &(name, typ, other) in members {
        string(b, name);
        integer(b, typ);
        if typ & 32 != 0 {
            integer(b, 3);
        }
        if let Some(n) = other {
            string(b, n);
        }
    }
}
fn tagfile(parents: [i64; 2], lock: bool) -> Vec<u8> {
    let mut b = vec![0x1e, 0x0d, 0xb0, 0xca, 0xce, 0xfa, 0x11, 0xd0];
    integer(&mut b, 1);
    integer(&mut b, 3);
    class(
        &mut b,
        b"hkaSkeleton",
        3,
        &[
            (b"name", 10, None),
            (b"parentIndices", 18, None),
            (b"bones", 25, Some(b"hkaBone")),
            (b"referencePose", 22, None),
        ],
    );
    class(
        &mut b,
        b"hkaBone",
        0,
        &[(b"name", 10, None), (b"lockTranslation", 1, None)],
    );
    integer(&mut b, 4);
    integer(&mut b, 1);
    b.push(15);
    string(&mut b, b"fixture");
    integer(&mut b, 2);
    integer(&mut b, 4);
    for v in parents {
        integer(&mut b, v);
    }
    integer(&mut b, 2);
    b.push(if lock { 3 } else { 1 });
    string(&mut b, b"root");
    string(&mut b, b"child");
    if lock {
        b.extend_from_slice(&[0, 1]);
    }
    integer(&mut b, 2);
    for translation in [[0f32, 0., 0., 7.], [1., 2., 3., 8.]] {
        for f in translation
            .into_iter()
            .chain([0., 0., 0., 1.])
            .chain([1., 2., 3., 9.])
        {
            b.extend_from_slice(&f.to_le_bytes());
        }
    }
    integer(&mut b, 7);
    b
}
fn sklb(havok: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 56];
    b[..8].copy_from_slice(b"blks0031");
    b[8..12].copy_from_slice(&48u32.to_le_bytes());
    b[12..16].copy_from_slice(&56u32.to_le_bytes());
    b[48..56].copy_from_slice(b"hpla\0\0\0\0");
    b.extend_from_slice(havok);
    b
}
#[test]
fn skeleton_semantics_preserve_rest_lanes_and_columnar_names() {
    for lock in [false, true] {
        let b = sklb(&tagfile([-1, 0], lock));
        let file = Sklb::parse(&b).unwrap();
        let s = file.skeletons(4096).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].bones.len(), 2);
        assert_eq!(s[0].bones[0].name, b"root");
        assert_eq!(s[0].bones[1].name, b"child");
        assert_eq!(s[0].bones[1].parent, Some(0));
        assert_eq!(s[0].bones[1].lock_translation, lock);
        assert_eq!(s[0].bones[1].rest.translation, [1., 2., 3., 8.]);
        assert_eq!(s[0].bones[1].rest.rotation, [0., 0., 0., 1.]);
        assert_eq!(s[0].bones[1].rest.scale, [1., 2., 3., 9.]);
        assert_eq!(file.auxiliary, b"hpla\0\0\0\0");
    }
}
#[test]
fn parents_and_output_budgets_are_checked_not_repaired() {
    for parents in [[-2, 0], [2, 0], [1, 0], [0, 0]] {
        let b = tagfile(parents, true);
        let t = Tagfile::parse(&b).unwrap();
        assert!(t.skeletons(4096).is_err());
    }
    let b = tagfile([-1, 0], true);
    let t = Tagfile::parse(&b).unwrap();
    assert!(matches!(t.skeletons(1), Err(Error::BudgetExceeded(_))));
    assert!(matches!(
        t.skeletons_with_limits(SkeletonLimits {
            max_bones: 1,
            ..SkeletonLimits::default()
        }),
        Err(Error::BudgetExceeded(_))
    ));
    assert!(matches!(
        t.skeletons_with_limits(SkeletonLimits {
            max_validation_steps: 1,
            ..SkeletonLimits::default()
        }),
        Err(Error::BudgetExceeded(_))
    ));
}
#[test]
fn graph_metadata_and_limits_remain_backend_independent() {
    let b = tagfile([-1, 0], true);
    let t = Tagfile::parse(&b).unwrap();
    assert_eq!(t.classes.len(), 2);
    assert_eq!(t.objects.len(), 1);
    assert!(t.object(0).unwrap().is_none());
    assert!(t.object(2).is_err());
    assert!(t.class(0).is_err());
    let Some(Value::Vectors { components, data }) = t.objects[0].field(b"referencePose") else {
        panic!("pose")
    };
    assert_eq!(*components, 12);
    assert_eq!(data.len(), 24);
    assert_eq!(data.get(usize::MAX), None);
    assert_eq!(data.values().count(), 24);
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_allocation_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_classes: 1,
            ..Limits::default()
        },
        Limits {
            max_members: 1,
            ..Limits::default()
        },
        Limits {
            max_objects: 0,
            ..Limits::default()
        },
        Limits {
            max_strings: 1,
            ..Limits::default()
        },
        Limits {
            max_string_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_array_length: 1,
            ..Limits::default()
        },
        Limits {
            max_values: 1,
            ..Limits::default()
        },
        Limits {
            max_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_validation_steps: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Tagfile::parse_with_limits(&b, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
}
#[test]
fn forward_refs_are_checked_after_objects_and_nulls_stay_null() {
    let mut b = vec![0x1e, 0x0d, 0xb0, 0xca, 0xce, 0xfa, 0x11, 0xd0];
    integer(&mut b, 1);
    integer(&mut b, 3);
    class(&mut b, b"Node", 0, &[(b"next", 8, Some(b"Node"))]);
    for reference in [2, 0] {
        integer(&mut b, 4);
        integer(&mut b, 1);
        b.push(1);
        integer(&mut b, reference);
    }
    integer(&mut b, 7);
    assert!(Tagfile::parse(&b).is_ok());
    let at = b.len() - 2;
    b[at] = 6;
    assert!(Tagfile::parse(&b).is_err());
}
#[test]
fn all_truncations_overlong_varints_and_mutations_do_not_panic() {
    let b = sklb(&tagfile([-1, 0], true));
    for n in 0..b.len() {
        assert!(Sklb::parse(&b[..n]).is_err());
    }
    let mut bad = tagfile([-1, 0], true);
    bad[8..18].fill(255);
    assert!(Tagfile::parse(&bad).is_err());
    let mut state = 0x8741u32;
    for _ in 0..8192 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut v = b.clone();
        let at = state as usize % v.len();
        v[at] ^= (state >> 16) as u8;
        if let Ok(t) = Sklb::parse(&v) {
            let _ = t.skeletons(65536);
        }
    }
}

#[test]
fn generic_arrays_tuples_vectors_signed64_and_optional_roots() {
    let mut b = vec![0x1e, 0x0d, 0xb0, 0xca, 0xce, 0xfa, 0x11, 0xd0];
    integer(&mut b, 1);
    integer(&mut b, 3);
    class(
        &mut b,
        b"Payload",
        0,
        &[
            (b"bytes", 17, None),
            (b"integers", 18, None),
            (b"reals", 19, None),
            (b"vectors", 20, None),
            (b"tuple", 34, None),
            (b"text", 10, None),
        ],
    );
    integer(&mut b, 4);
    integer(&mut b, 1);
    b.push(63);
    integer(&mut b, 3);
    b.extend_from_slice(&[0, 128, 255]);
    integer(&mut b, 3);
    integer(&mut b, 8);
    for v in [i64::MIN, 0, i64::MAX] {
        integer(&mut b, v);
    }
    integer(&mut b, 2);
    b.extend_from_slice(&(-1f32).to_le_bytes());
    b.extend_from_slice(&2f32.to_le_bytes());
    integer(&mut b, 2);
    integer(&mut b, 3);
    for v in [1f32, 2., 3., 4., 5., 6.] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    integer(&mut b, 4);
    for v in [-1, 64, 65536] {
        integer(&mut b, v);
    }
    integer(&mut b, -1); // Initial string slot one is null.
    integer(&mut b, 5);
    integer(&mut b, 1);
    integer(&mut b, 6);
    integer(&mut b, 7);
    let t = Tagfile::parse(&b).unwrap();
    let o = &t.objects[0];
    assert!(matches!(o.field(b"bytes"),Some(Value::Bytes(v))if *v==[0,128,255]));
    assert!(
        matches!(o.field(b"integers"),Some(Value::Integers{width:8,values})if *values==[i64::MIN,0,i64::MAX])
    );
    assert!(matches!(o.field(b"vectors"),Some(Value::Vectors{components:3,data})if data.len()==6));
    assert!(
        matches!(o.field(b"tuple"),Some(Value::Integers{width:4,values})if *values==[-1,64,65536])
    );
    assert!(matches!(o.field(b"text"), Some(Value::String(None))));
    assert_eq!(t.roots.len(), 3);
}

#[test]
fn short_container_and_animation_container_selection() {
    let mut h = tagfile([-1, 0], true);
    h.pop();
    class(
        &mut h,
        b"hkaAnimationContainer",
        1,
        &[(b"skeletons", 24, Some(b"hkaSkeleton"))],
    );
    integer(&mut h, 4);
    integer(&mut h, 3);
    h.push(1);
    integer(&mut h, 1);
    integer(&mut h, 1);
    integer(&mut h, 7);
    let mut b = vec![0; 54];
    b[..8].copy_from_slice(b"blks0021");
    b[8..10].copy_from_slice(&46u16.to_le_bytes());
    b[10..12].copy_from_slice(&54u16.to_le_bytes());
    b[46..54].copy_from_slice(b"hpla\0\0\0\0");
    b.extend_from_slice(&h);
    let f = Sklb::parse(&b).unwrap();
    assert_eq!(f.version, icexiv_data::sklb::Version::V1200);
    assert_eq!(f.raw_header.len(), 46);
    assert_eq!(f.tagfile.animation_skeleton_ids(1024).unwrap(), [1]);
    assert!(matches!(
        f.tagfile.animation_skeleton_ids(0),
        Err(Error::BudgetExceeded(_))
    ));
    let at = h.len() - 2;
    h[at] = 4;
    let g = Tagfile::parse(&h).unwrap();
    assert!(g.animation_skeleton_ids(1024).is_err());
}

#[test]
fn long_schema_names_consume_work_and_recursion_has_a_hard_ceiling() {
    let mut b = vec![0x1e, 0x0d, 0xb0, 0xca, 0xce, 0xfa, 0x11, 0xd0];
    integer(&mut b, 1);
    integer(&mut b, 3);
    class(&mut b, &vec![b'a'; 10000], 0, &[]);
    class(&mut b, &vec![b'b'; 10000], 0, &[]);
    integer(&mut b, 7);
    assert!(matches!(
        Tagfile::parse_with_limits(
            &b,
            Limits {
                max_validation_steps: 100,
                ..Limits::default()
            }
        ),
        Err(Error::BudgetExceeded(_))
    ));
    assert!(matches!(
        Tagfile::parse_with_limits(
            &b,
            Limits {
                max_depth: usize::MAX,
                ..Limits::default()
            }
        ),
        Err(Error::BudgetExceeded(_))
    ));
}

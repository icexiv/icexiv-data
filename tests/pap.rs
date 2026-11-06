use icexiv_data::{
    havok::{Error, SplineLimits},
    pap::Pap,
};
fn int(b: &mut Vec<u8>, v: i64) {
    let mut x = v.unsigned_abs();
    let mut y = ((x & 63) as u8) * 2 + u8::from(v < 0);
    x >>= 6;
    if x != 0 {
        y |= 128;
    }
    b.push(y);
    while x != 0 {
        y = (x & 127) as u8;
        x >>= 7;
        if x != 0 {
            y |= 128;
        }
        b.push(y);
    }
}
fn text(b: &mut Vec<u8>, v: &[u8]) {
    int(b, v.len() as i64);
    b.extend_from_slice(v);
}
fn class(b: &mut Vec<u8>, name: &[u8], version: i64, fields: &[(&[u8], u8)]) {
    int(b, 2);
    text(b, name);
    int(b, version);
    int(b, 0);
    int(b, fields.len() as i64);
    for (name, t) in fields {
        text(b, name);
        int(b, i64::from(*t));
        if t & 15 == 8 {
            text(b, b"hkaSplineCompressedAnimation");
        }
    }
}
fn floats(b: &mut Vec<u8>, v: &[f32]) {
    for v in v {
        b.extend_from_slice(&v.to_le_bytes());
    }
}
fn array(b: &mut Vec<u8>, v: &[i64]) {
    int(b, v.len() as i64);
    int(b, 4);
    for v in v {
        int(b, *v);
    }
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0x1e, 0x0d, 0xb0, 0xca, 0xce, 0xfa, 0x11, 0xd0];
    int(&mut b, 1);
    int(&mut b, 3);
    class(
        &mut b,
        b"hkaAnimationContainer",
        1,
        &[(b"animations", 24), (b"bindings", 24)],
    );
    class(
        &mut b,
        b"hkaAnimationBinding",
        1,
        &[
            (b"animation", 8),
            (b"originalSkeletonName", 10),
            (b"transformTrackToBoneIndices", 18),
            (b"blendHint", 2),
        ],
    );
    class(
        &mut b,
        b"hkaSplineCompressedAnimation",
        0,
        &[
            (b"type", 2),
            (b"duration", 3),
            (b"numberOfTransformTracks", 2),
            (b"numFrames", 2),
            (b"numBlocks", 2),
            (b"maxFramesPerBlock", 2),
            (b"maskAndQuantizationSize", 2),
            (b"frameDuration", 3),
            (b"blockOffsets", 18),
            (b"floatBlockOffsets", 18),
            (b"data", 17),
        ],
    );
    int(&mut b, 4);
    int(&mut b, 1);
    b.push(3);
    int(&mut b, 1);
    int(&mut b, 3);
    int(&mut b, 1);
    int(&mut b, 2);
    int(&mut b, 4);
    int(&mut b, 2);
    b.push(15);
    int(&mut b, 3);
    text(&mut b, b"root");
    array(&mut b, &[7]);
    int(&mut b, 1);
    int(&mut b, 4);
    int(&mut b, 3);
    b.extend_from_slice(&[255, 7]);
    int(&mut b, 3);
    floats(&mut b, &[2.]);
    for v in [1, 3, 1, 3, 4] {
        int(&mut b, v);
    }
    floats(&mut b, &[1.]);
    array(&mut b, &[0]);
    array(&mut b, &[40]);
    let mut data = vec![20, 16, 15, 0, 1, 0, 1, 0, 0, 2, 2, 0];
    floats(&mut data, &[-2., 6.]);
    data.extend_from_slice(&[0, 255, 0, 0]);
    floats(&mut data, &[0., 0., 0., 1.]);
    data.extend_from_slice(&[0; 8]);
    int(&mut b, data.len() as i64);
    b.extend(data);
    int(&mut b, 7);
    let mut pap = vec![0; 66];
    pap[..4].copy_from_slice(b"pap ");
    pap[4..8].copy_from_slice(&0x00020001u32.to_le_bytes());
    pap[8..10].copy_from_slice(&1u16.to_le_bytes());
    pap[10..12].copy_from_slice(&101u16.to_le_bytes());
    pap[14..18].copy_from_slice(&26u32.to_le_bytes());
    pap[18..22].copy_from_slice(&66u32.to_le_bytes());
    pap[22..26].copy_from_slice(&(66u32 + b.len() as u32).to_le_bytes());
    pap[26..30].copy_from_slice(b"test");
    pap.extend(b);
    pap
}
#[test]
fn borrowed_animation_byte_arrays_obey_explicit_length_budgets() {
    let b = fixture();
    let limits = icexiv_data::havok::Limits {
        max_array_length: 47,
        ..icexiv_data::havok::Limits::default()
    };
    assert!(matches!(
        Pap::parse_with_limits(&b, limits),
        Err(Error::BudgetExceeded("array length"))
    ));
    let p = Pap::parse_with_limits(
        &b,
        icexiv_data::havok::Limits {
            max_array_length: 48,
            ..limits
        },
    )
    .unwrap();
    let a = p
        .tagfile
        .as_ref()
        .unwrap()
        .spline_animation(3, SplineLimits::default())
        .unwrap();
    assert_eq!(
        a.sample(1000., 4096, 4096).unwrap()[0].translation,
        [2., 0., 0.]
    );
}
#[test]
fn zero_record_package_has_explicitly_absent_havok_not_a_fake_graph() {
    let mut b = vec![0u8; 32];
    b[..4].copy_from_slice(b"pap ");
    b[4..8].copy_from_slice(&0x00020001u32.to_le_bytes());
    for (offset, value) in [(14, 26u32), (18, 26), (22, 32)] {
        b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let p = Pap::parse(&b).unwrap();
    assert_eq!(p.records().len(), 0);
    assert!(p.tagfile.is_none());
    assert_eq!(p.havok_padding, [0; 6]);
    assert!(p.timeline.is_empty());
    assert!(p.binding(0).is_err());
    let mut bad = b.clone();
    bad[26] = 1;
    assert!(Pap::parse(&bad).is_err());
    let mut bad = b.clone();
    bad[22..26].copy_from_slice(&31u32.to_le_bytes());
    assert!(Pap::parse(&bad).is_err());
    let mut bad = b.clone();
    bad.resize(48, 0);
    bad[22..26].copy_from_slice(&48u32.to_le_bytes());
    assert!(Pap::parse(&bad).is_err());
    b.truncate(26);
    b[22..26].copy_from_slice(&26u32.to_le_bytes());
    assert!(Pap::parse(&b).unwrap().tagfile.is_none());
}
#[test]
fn pap_container_alignment_is_not_arbitrary_tagfile_trailing_data() {
    let original = fixture();
    let stream_end = original.len();
    // Vary the absolute stream origin to cover all sixteen alignment residues.
    for gap in 0..16 {
        let mut b = original[..66].to_vec();
        b.resize(66 + gap, 0);
        b.extend_from_slice(&original[66..]);
        b[18..22].copy_from_slice(&((66 + gap) as u32).to_le_bytes());
        let end = b.len();
        let padded = (end + 15) & !15;
        b.resize(padded, 0);
        b[22..26].copy_from_slice(&(padded as u32).to_le_bytes());
        b.extend_from_slice(b"timeline");
        let p = Pap::parse(&b).unwrap();
        assert_eq!(p.havok_padding.len(), padded - end);
        assert_eq!(p.timeline, b"timeline");
        assert_eq!(p.binding(0).unwrap().animation_object_id, 3);
        if padded > end {
            let mut bad = b.clone();
            bad[end] = 1;
            assert!(Pap::parse(&bad).is_err());
            let mut misaligned = b.clone();
            misaligned[22..26].copy_from_slice(&((padded - 1) as u32).to_le_bytes());
            if padded - 1 > end {
                assert!(Pap::parse(&misaligned).is_err());
            }
        }
        let mut excessive = b[..padded].to_vec();
        excessive.resize(padded + 16, 0);
        excessive[22..26].copy_from_slice(&((padded + 16) as u32).to_le_bytes());
        assert!(Pap::parse(&excessive).is_err());
    }
    let mut standalone = original[66..stream_end].to_vec();
    standalone.push(0);
    assert!(icexiv_data::havok::Tagfile::parse(&standalone).is_err());
}
#[test]
fn pap_binding_and_real_spline_sampling_not_just_metadata() {
    let b = fixture();
    let p = Pap::parse(&b).unwrap();
    let record = p.records().next().unwrap();
    assert_eq!(record.name, b"test");
    assert_eq!(p.skeleton_id, 101);
    let binding = p.binding(0).unwrap();
    assert_eq!(binding.animation_object_id, 3);
    assert_eq!(binding.transform_track_to_bone, [7]);
    assert_eq!(binding.blend_hint, 1);
    assert_eq!(binding.original_skeleton_name, Some(&b"root"[..]));
    let a = p
        .tagfile
        .as_ref()
        .unwrap()
        .spline_animation(3, SplineLimits::default())
        .unwrap();
    for (time, x) in [
        (-100., -2.),
        (0., -2.),
        (500., 0.),
        (1000., 2.),
        (2000., 6.),
        (3000., 6.),
    ] {
        let s = a.sample(time, 4096, 4096).unwrap();
        assert_eq!(s[0].translation, [x, 0., 0.]);
        assert_eq!(s[0].rotation, [0., 0., 0., 1.]);
        assert_eq!(s[0].scale, [1.; 3]);
    }
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(a.sample(t, 4096, 4096).is_err());
    }
    assert!(a.sample(500., 0, 4096).is_err());
    assert!(a.sample(500., 4096, 0).is_err());
    assert!(p.binding(1).is_err());
}
#[test]
fn spline_limits_and_malformed_inputs() {
    let b = fixture();
    let p = Pap::parse(&b).unwrap();
    for limits in [
        SplineLimits {
            max_tracks: 0,
            ..SplineLimits::default()
        },
        SplineLimits {
            max_blocks: 0,
            ..SplineLimits::default()
        },
        SplineLimits {
            max_control_points: 0,
            ..SplineLimits::default()
        },
        SplineLimits {
            max_allocation_bytes: 0,
            ..SplineLimits::default()
        },
        SplineLimits {
            max_work: 0,
            ..SplineLimits::default()
        },
    ] {
        assert!(matches!(
            p.tagfile.as_ref().unwrap().spline_animation(3, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
    for n in 0..b.len() {
        assert!(Pap::parse(&b[..n]).is_err());
    }
    let mut state = 0x28417u32;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut data = b.clone();
        let at = state as usize % data.len();
        data[at] ^= (state >> 16) as u8;
        if let Ok(p) = Pap::parse(&data) {
            if let Some(tagfile) = p.tagfile.as_ref() {
                if let Ok(a) = tagfile.spline_animation(3, SplineLimits::default()) {
                    let _ = a.sample(500., 4096, 4096);
                }
            }
        }
    }
}

#[test]
fn unsupported_encodings_float_tracks_and_bad_curves_are_errors() {
    let original = fixture();
    let start = original.len() - 49;
    for (at, value) in [
        (start, 12),
        (start, 22),
        (start + 6, 255),
        (start + 9, 0),
        (start + 10, 0),
        (start + 39, 127),
    ] {
        let mut bytes = original.clone();
        bytes[at] = value;
        let p = Pap::parse(&bytes).unwrap();
        assert!(
            p.tagfile
                .as_ref()
                .unwrap()
                .spline_animation(3, SplineLimits::default())
                .is_err(),
            "mutation at {at}"
        );
    }
    let mut p = Pap::parse(&original).unwrap();
    p.tagfile.as_mut().unwrap().objects[2]
        .fields
        .push(icexiv_data::havok::Field {
            name: b"numberOfFloatTracks",
            value: icexiv_data::havok::Value::Integer(1),
        });
    assert!(
        p.tagfile
            .as_ref()
            .unwrap()
            .spline_animation(3, SplineLimits::default())
            .is_err()
    );
}

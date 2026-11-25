use icexiv_data::{
    havok::Sample,
    pap::Binding,
    tmb::{Timeline, TimelineLimits},
};

fn packet(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    [
        tag.as_slice(),
        &((payload.len() + 8) as u32).to_le_bytes(),
        payload,
    ]
    .concat()
}

#[test]
fn linked_effect_preserves_targets_timing_and_trs() {
    let mut fx = vec![0; 64];
    fx[..2].copy_from_slice(&21u16.to_le_bytes());
    fx[2..4].copy_from_slice(&(-2i16).to_le_bytes());
    fx[4..8].copy_from_slice(&65538i32.to_le_bytes());
    fx[16..24].copy_from_slice(&[1, 2, 255, 255, 2, 0, 5, 0]);
    fx[56..60].copy_from_slice(&2u32.to_le_bytes());
    let mut actor = vec![0; 20];
    actor[0] = 7;
    actor[12] = 118;
    actor[16] = 1;
    let mut track = vec![0; 16];
    track[0] = 19;
    track[4] = 92;
    track[8] = 1;
    // First entry payload at 20; actors at 36, tracks at 64, command at 88.
    let mut tail = vec![7, 0, 19, 0, 21, 0];
    for (field, count) in [(24, 3), (32, 3), (40, 3), (48, 4)] {
        fx[field..field + 4].copy_from_slice(&((152 + tail.len() - 88) as u32).to_le_bytes());
        fx[field + 4] = count;
        for n in 0..count {
            tail.extend_from_slice(&(n as f32 + field as f32).to_le_bytes());
        }
    }
    fx[12..16].copy_from_slice(&((152 + tail.len() - 88) as u32).to_le_bytes());
    tail.extend_from_slice(b"vfx/test.avfx\0");
    let items = [
        packet(b"TMAL", &[132, 0, 0, 0, 1, 0, 0, 0]),
        packet(b"TMAC", &actor),
        packet(b"TMTR", &track),
        packet(b"C012", &fx),
    ]
    .concat();
    let mut b = packet(
        b"TMLB",
        &[4u32.to_le_bytes().to_vec(), items, tail].concat(),
    );
    let parse = |b: &[u8]| {
        Timeline::parse_sequence(b, Default::default())
            .unwrap()
            .remove(0)
            .effect_command(3)
            .map(|x| x.unwrap().scale)
    };
    let t = Timeline::parse_sequence(&b, Default::default())
        .unwrap()
        .remove(0);
    let links = t.command_links(1024, 1024).unwrap();
    assert_eq!(
        (links[0].actor, links[0].track, links[0].command),
        (1, 2, 3)
    );
    assert!(t.command_links(1024, 1).is_err());
    let e = t.effect_command(3).unwrap().unwrap();
    assert_eq!(
        (e.resource.id, e.resource.start, e.resource.duration),
        (21, -2, 65538)
    );
    assert_eq!(e.resource.name, b"vfx/test.avfx");
    assert_eq!(
        (e.targets[0].kind, e.targets[0].id, e.targets[1].id),
        (2, -1, 5)
    );
    assert_eq!(e.position, [40., 41., 42.]);
    assert_eq!(e.color, [48., 49., 50., 51.]);
    b[88 + 28] = 4;
    assert!(parse(&b).is_err());
    b[152] = 8;
    assert!(
        Timeline::parse_sequence(&b, Default::default()).unwrap()[0]
            .command_links(1024, 1024)
            .is_err()
    );
}
#[test]
fn tmlb_relative_links_alignment_and_bounds() {
    let list = packet(b"TMAL", &[8, 0, 0, 0, 1, 0, 0, 0]);
    let a = packet(
        b"TMLB",
        &[&1u32.to_le_bytes()[..], &list, &[13, 0, 0]].concat(),
    );
    let b = packet(
        b"TMLB",
        &[&1u32.to_le_bytes()[..], &packet(b"????", &[0xff])].concat(),
    );
    let mut data = [a.as_slice(), &[0], &b].concat();
    let decode = |d| Timeline::parse_sequence(d, TimelineLimits::default());
    let ts = decode(&data).unwrap();
    assert_eq!(ts[1].offset, 32);
    assert_eq!(ts[1].entries[0].payload, [0xff]);
    assert_eq!(
        ts[0].linked_ids(0).unwrap().unwrap().collect::<Vec<_>>(),
        [13]
    );
    assert!(
        Timeline::parse_sequence(
            &data,
            TimelineLimits {
                max_entries: 1,
                ..Default::default()
            }
        )
        .is_err()
    );
    data[20] = 100;
    assert!(
        Timeline::parse_sequence(&data, Default::default()).unwrap()[0]
            .linked_ids(0)
            .is_err()
    );
    data[31] = 1;
    assert!(Timeline::parse_sequence(&data, Default::default()).is_err());
    for i in 1..a.len() {
        assert!(Timeline::parse_sequence(&a[..i], Default::default()).is_err());
    }
}
#[test]
fn sparse_binding_respects_explicit_tracks() {
    let map = Binding {
        object_id: 3,
        animation_object_id: 4,
        original_skeleton_name: None,
        transform_track_to_bone: &[2, -1, 0],
        float_track_to_slot: &[],
        blend_hint: 1,
    };
    let sample = |v| Sample {
        translation: [v, 0., 0.],
        rotation: [0., 0., 0., 1.],
        scale: [1.; 3],
    };
    let tracks = [sample(3.), sample(7.), sample(10.)];
    assert_eq!(
        map.mapped_samples(&tracks, 4, 1024).unwrap(),
        [Some(tracks[2]), None, Some(tracks[0]), None]
    );
    assert!(map.mapped_samples(&tracks, 2, 1024).is_err());
    assert!(map.mapped_samples(&tracks, 4, 0).is_err());
    assert!(
        Binding {
            transform_track_to_bone: &[0, 0, 1],
            ..map
        }
        .mapped_samples(&tracks, 4, 1024)
        .is_err()
    );
    assert!(
        Binding {
            transform_track_to_bone: &[],
            ..map
        }
        .mapped_samples(&tracks, 4, 1024)
        .is_err()
    );
}
#[test]
fn attachment_origin_follows_animated_parent_in_bone_order() {
    use icexiv_data::havok::{Bone, Skeleton, Transform};
    let bone = |parent, x| Bone {
        name: b"",
        parent,
        lock_translation: false,
        rest: Transform {
            translation: [x, 0., 0., 0.],
            rotation: [0., 0., 0., 1.],
            scale: [1.; 4],
        },
    };
    let skel = Skeleton {
        name: None,
        object_id: 3,
        bones: vec![bone(Some(1), 1.), bone(None, 10.)],
    };
    assert!(skel.bone_by_name(b"", 100).is_err()); // duplicate, not first-match-wins
    assert_eq!(skel.bone_by_name(b"absent", 100).unwrap(), None);
    assert!(skel.bone_by_name(b"absent", 0).is_err());
    let pose = [
        None,
        Some(Sample {
            translation: [10., 1., 3.],
            rotation: [0., 0., 1., 1.],
            scale: [2., 3., 1.],
        }),
    ];
    assert_eq!(
        skel.posed_point(0, [1., 1., 0.], &pose, 2).unwrap(),
        [7., 5., 3.]
    );
    assert!(skel.posed_point(0, [0.; 3], &pose, 1).is_err());
    assert_eq!(
        skel.posed_point(0, [1., 1., 0.], &[None, None], 2).unwrap(),
        [12., 1., 0.]
    );
}

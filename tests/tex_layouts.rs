use icexiv_data::tex::{CubeFace, Error, Format, Layout, Tex};
fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn texture(
    kind: u32,
    format: Format,
    dimensions: [u16; 3],
    images: u8,
    payloads: &[Vec<u8>],
) -> Vec<u8> {
    let mut b = vec![0; 80];
    put32(&mut b, 0, kind);
    put32(&mut b, 4, format as u32);
    for (i, value) in dimensions.into_iter().enumerate() {
        put16(&mut b, 8 + i * 2, value);
    }
    b[14] = payloads.len() as u8;
    b[15] = images;
    for (i, payload) in payloads.iter().enumerate() {
        let p = b.len();
        put32(&mut b, 28 + i * 4, p as u32);
        b.extend_from_slice(payload);
    }
    b
}

#[test]
fn packed_volume_is_explicit_complete_and_retains_the_offset_words() {
    let payloads = [
        vec![1; 2048],
        vec![2; 256],
        vec![3; 64],
        vec![4; 16],
        vec![5; 8],
        vec![6; 4],
    ];
    let mut b = texture(0x01000000, Format::B8G8R8A8, [8, 32, 2], 0, &payloads);
    assert!(!Tex::parse(&b).unwrap().uses_packed_volume_layout());
    for i in 0..6 {
        put32(&mut b, 28 + i * 4, 80);
    }
    assert!(matches!(
        Tex::parse(&b),
        Err(Error::InvalidData("mip overlaps header or previous mip"))
    ));
    let t = Tex::parse_packed_volume(&b).unwrap();
    assert!(t.uses_packed_volume_layout());
    assert_eq!(t.declared_mip_offsets(), [80; 6]);
    for (m, payload) in payloads.iter().enumerate() {
        assert_eq!(t.mip(m).unwrap().data(), payload);
        assert_eq!(
            t.mip(m).unwrap().decode_rgba8(payload.len()).unwrap(),
            *payload
        );
        assert!(matches!(
            t.mip(m).unwrap().decode_rgba8(payload.len() - 1),
            Err(Error::OutputTooLarge { .. })
        ));
    }
    for end in 0..b.len() {
        assert!(Tex::parse_packed_volume(&b[..end]).is_err());
    }
    let mut other = b.clone();
    put32(&mut other, 32, 81);
    assert!(Tex::parse_packed_volume(&other).is_err());
    put32(&mut other, 32, 80);
    put32(&mut other, 0, 0x00800000);
    put16(&mut other, 12, 1);
    assert!(Tex::parse_packed_volume(&other).is_err());
}
#[test]
fn array_mips_preserve_every_layer_and_budget_the_whole_output() {
    let b = texture(
        0x10000000,
        Format::L8,
        [2, 2, 1],
        3,
        &[
            vec![0, 1, 2, 3, 10, 11, 12, 13, 20, 21, 22, 23],
            vec![100, 110, 120],
        ],
    );
    let t = Tex::parse(&b).unwrap();
    assert_eq!(t.layout(), Layout::D2Array);
    assert_eq!(t.raw_array_size(), 3);
    let m = t.mip(0).unwrap();
    assert_eq!(m.layer_count(), 3);
    assert_eq!(m.images().len(), 3);
    assert_eq!(m.image(2, 0, 0).unwrap().data(), [20, 21, 22, 23]);
    let rgba = m.decode_rgba8(48).unwrap();
    assert_eq!(&rgba[32..36], [20, 20, 20, 255]);
    assert!(matches!(
        m.decode_rgba8(47),
        Err(Error::OutputTooLarge { requested: 48, .. })
    ));
    assert_eq!(
        t.mip(1).unwrap().decode_rgba8(12).unwrap(),
        [100, 100, 100, 255, 110, 110, 110, 255, 120, 120, 120, 255]
    );
    assert!(m.image(3, 0, 0).is_err());
    assert!(m.image(0, 1, 0).is_err());
    assert!(m.image(0, 0, 1).is_err());
    assert!(m.image_by_index(usize::MAX).is_err());
}

#[test]
fn cube_header_bytes_zero_and_six_both_mean_six_faces_not_six_cubes() {
    let base: Vec<_> = (0..6u8).flat_map(|i| [i; 4]).collect();
    let small: Vec<_> = (10..16).collect();
    for count in [0, 6] {
        let b = texture(
            0x02000000,
            Format::L8,
            [2, 2, 1],
            count,
            &[base.clone(), small.clone()],
        );
        let t = Tex::parse(&b).unwrap();
        assert_eq!(t.layout(), Layout::Cube);
        assert_eq!(t.raw_array_size(), count);
        let m = t.mip(0).unwrap();
        assert_eq!(m.layer_count(), 1);
        assert_eq!(m.face_count(), 6);
        assert_eq!(m.image_count(), 6);
        for face in CubeFace::ALL {
            let surface = m.cube_face(face).unwrap();
            assert_eq!(surface.data(), [face.index() as u8; 4]);
            assert_eq!(surface.data(), m.image(0, face.index(), 0).unwrap().data());
            assert_eq!(
                t.mip(1).unwrap().cube_face(face).unwrap().data(),
                [10 + face.index() as u8]
            );
            assert_eq!(CubeFace::from_index(face.index()), Some(face));
        }
        assert_eq!(m.decode_rgba8(96).unwrap().len(), 96);
        assert_eq!(t.mip(1).unwrap().image(0, 3, 0).unwrap().data(), [13]);
        assert!(m.image(0, 6, 0).is_err());
    }
    let b = texture(0x02000000, Format::L8, [1, 1, 1], 12, &[vec![0; 12]]);
    assert!(matches!(
        Tex::parse(&b),
        Err(Error::UnsupportedCubeImageCount(12))
    ));
}

#[test]
fn named_cube_faces_reject_other_layouts_and_invalid_indices() {
    for index in [6, 7, 255, usize::MAX] {
        assert_eq!(CubeFace::from_index(index), None);
    }
    for (kind, depth, count, images) in [
        (0x00800000, 1, 0, 1),
        (0x10000000, 1, 6, 6),
        (0x01000000, 6, 0, 6),
    ] {
        let b = texture(kind, Format::L8, [1, 1, depth], count, &[vec![0; images]]);
        let mip = Tex::parse(&b).unwrap().mip(0).unwrap();
        for face in CubeFace::ALL {
            assert!(matches!(mip.cube_face(face), Err(Error::NotCubeTexture)));
        }
    }
}

#[test]
fn volume_mips_shrink_depth_and_allow_more_levels_than_width() {
    let b = texture(
        0x01000000,
        Format::L8,
        [2, 2, 4],
        0,
        &[(0..16).collect(), vec![100, 101], vec![200]],
    );
    let t = Tex::parse(&b).unwrap();
    assert_eq!(t.layout(), Layout::Volume);
    assert_eq!(t.depth(), 4);
    assert_eq!(t.mips().map(|m| m.depth()).collect::<Vec<_>>(), [4, 2, 1]);
    assert_eq!(
        t.mip(0).unwrap().image(0, 0, 3).unwrap().data(),
        [12, 13, 14, 15]
    );
    assert_eq!(
        t.mip(1)
            .unwrap()
            .image(0, 0, 1)
            .unwrap()
            .decode_rgba8(4)
            .unwrap(),
        [101, 101, 101, 255]
    );
    assert_eq!(t.mip(0).unwrap().decode_rgba8(64).unwrap().len(), 64);
    assert!(t.mip(1).unwrap().image(0, 0, 2).is_err());
}

#[test]
fn rectangular_volume_six_mips_keep_slices_after_depth_reaches_one() {
    // Layout profile independently observed in a Win32 8x32x2 BGRA8 volume.
    let payloads = (0..6)
        .map(|m| {
            let w = (8usize >> m).max(1);
            let h = (32usize >> m).max(1);
            let d = (2usize >> m).max(1);
            [m as u8, 40, 80, 120].repeat(w * h * d)
        })
        .collect::<Vec<_>>();
    let b = texture(0x01000000, Format::B8G8R8A8, [8, 32, 2], 0, &payloads);
    assert_eq!(b.len(), 2476);
    let t = Tex::parse(&b).unwrap();
    assert_eq!(
        t.mips().map(|m| m.depth()).collect::<Vec<_>>(),
        [2, 1, 1, 1, 1, 1]
    );
    for (i, mip) in t.mips().enumerate() {
        let bytes = mip.decode_rgba8(2048).unwrap();
        assert!(bytes.chunks_exact(4).all(|p| p == [80, 40, i as u8, 120]));
        for z in 0..usize::from(mip.depth()) {
            assert_eq!(
                mip.image(0, 0, z).unwrap().data(),
                mip.image_by_index(z).unwrap().data()
            );
        }
        assert!(mip.image(0, 0, usize::from(mip.depth())).is_err());
    }
    assert!(Tex::parse(&b[..b.len() - 1]).is_err());
}
#[test]
fn compressed_small_mips_round_blocks_independently_for_each_image() {
    let red = [0, 0xf8, 0, 0, 0, 0, 0, 0];
    let blue = [0x1f, 0, 0, 0, 0, 0, 0, 0];
    let payload: Vec<_> = red.into_iter().chain(blue).collect();
    let b = texture(0x10000000, Format::Bc1, [2, 2, 1], 2, &[payload]);
    let m = Tex::parse(&b).unwrap().mip(0).unwrap();
    let output = m.decode_rgba8(32).unwrap();
    assert_eq!(&output[..4], [255, 0, 0, 255]);
    assert_eq!(&output[16..20], [0, 0, 255, 255]);
    assert!(matches!(
        m.decode_rgba32f(1024),
        Err(Error::FloatOutputUnavailable(_))
    ));
}

#[test]
fn r16f_preserves_scalar_values_signed_zero_and_nonfinite_classes() {
    let words = [
        0x0000u16, 0x8000, 0x0001, 0x3c00, 0xbc00, 0x7bff, 0xfbff, 0x7c00, 0xfc00, 0x7e00, 0xfe00,
    ];
    let payload = words.into_iter().flat_map(u16::to_le_bytes).collect();
    let b = texture(0x800000, Format::R16F, [11, 1, 1], 0, &[payload]);
    let t = Tex::parse(&b).unwrap();
    assert_eq!(t.format().float_channels(), Some(1));
    let m = t.mip(0).unwrap();
    let v = m.decode_rgba32f(176).unwrap();
    for (i, expected) in [
        0.,
        -0.,
        2f32.powi(-24),
        1.,
        -1.,
        65504.,
        -65504.,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(v[i * 4].to_bits(), expected.to_bits());
        assert_eq!(v[i * 4 + 1].to_bits(), expected.to_bits());
        assert_eq!(v[i * 4 + 2].to_bits(), expected.to_bits());
        assert_eq!(v[i * 4 + 3], 1.);
    }
    assert!(v[36].is_nan() && v[40].is_nan());
    assert!(matches!(
        m.decode_rgba32f(175),
        Err(Error::OutputTooLarge { .. })
    ));
    assert!(matches!(
        m.decode_rgba8(43),
        Err(Error::OutputTooLarge { .. })
    ));
    assert_eq!(
        &m.decode_rgba8(44).unwrap()[36..],
        &[0, 0, 0, 255, 0, 0, 0, 255]
    );
    assert!(Tex::parse(&b[..b.len() - 1]).is_err());
}
#[test]
fn packed_unsigned_float_channels_have_independent_widths_and_special_values() {
    let words = [
        (15u32 << 6) | ((16 << 6) << 11) | ((17 << 5) << 22),
        1 | (63 << 11) | (1 << 22),
        0x7bf | (0x7c0 << 11) | (0x3df << 22),
        0x7c1 | (0x7ff << 11) | (0x3e1 << 22),
    ];
    let b = texture(
        0x800000,
        Format::R11G11B10F,
        [4, 1, 1],
        0,
        &[words.into_iter().flat_map(u32::to_le_bytes).collect()],
    );
    let t = Tex::parse(&b).unwrap();
    assert_eq!(t.format().float_channels(), Some(3));
    let m = t.mip(0).unwrap();
    let v = m.decode_rgba32f(64).unwrap();
    assert_eq!(&v[..4], &[1., 2., 4., 1.]);
    assert_eq!(
        &v[4..8],
        &[2f32.powi(-20), 63. * 2f32.powi(-20), 2f32.powi(-19), 1.]
    );
    assert_eq!(&v[8..12], &[65024., f32::INFINITY, 64512., 1.]);
    assert!(v[12..15].iter().all(|x| x.is_nan()));
    assert_eq!(v[15], 1.);
    assert_eq!(
        m.decode_rgba8(16).unwrap(),
        [
            255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255
        ]
    );
    assert!(matches!(
        m.decode_rgba32f(63),
        Err(Error::OutputTooLarge { .. })
    ));
    assert!(Tex::parse(&b[..b.len() - 1]).is_err());
}
#[test]
fn signed_bc6h_float_output_crops_edges_and_keeps_hdr_and_negative_values() {
    // Official BC6H mode 11: six direct signed 10-bit RGB endpoints,
    // one 3-bit anchor index, fifteen 4-bit indices (LSB first).
    let mut bits = 3u128;
    let mut at = 5;
    for v in [-511i32, 0, 511, 511, 0, -511] {
        bits |= u128::from((v & 1023) as u16) << at;
        at += 10;
    }
    at += 3;
    for _ in 1..16 {
        bits |= 15u128 << at;
        at += 4;
    }
    assert_eq!(at, 128);
    let b = texture(
        0x00800000,
        Format::Bc6hSf16,
        [2, 1, 1],
        0,
        &[bits.to_le_bytes().to_vec()],
    );
    let t = Tex::parse(&b).unwrap();
    assert_eq!(t.format().float_channels(), Some(3));
    let m = t.mip(0).unwrap();
    assert_eq!(
        m.decode_rgba32f(32).unwrap(),
        [-65504., 0., 65504., 1., 65504., 0., -65504., 1.]
    );
    assert_eq!(m.decode_rgba8(8).unwrap(), [0, 0, 255, 255, 255, 0, 0, 255]);
    assert!(matches!(
        m.decode_rgba32f(31),
        Err(Error::OutputTooLarge { .. })
    ));
    assert!(matches!(
        m.decode_rgba8(7),
        Err(Error::OutputTooLarge { .. })
    ));
    assert!(Tex::parse(&b[..b.len() - 1]).is_err());
}
#[test]
fn bc6h_all_valid_modes_match_independent_microsoft_cpu_samples() {
    // Our SHA-256-generated sample 73 for each valid mode, with independently
    // measured DirectXTex CPU pixel 15 bits. See the pinned full-corpus fixture
    // and docs/provenance/bc6h.md; no game data or upstream shipped fixture.
    let samples = [
        (
            0x377c07b932713e2b830f5348b37f8dfcu128,
            [0x3b0ae000, 0xc001e000, 0x3a96a000, 0x3f800000],
        ),
        (
            0xc110357096c528b47d25f93c3a395559u128,
            [0xba644000, 0xbb0ec000, 0x438aa000, 0x3f800000],
        ),
        (
            0x7aa58798d68d35ad1ea822c1be276a62u128,
            [0x44c84000, 0x39362000, 0xc43fa000, 0x3f800000],
        ),
        (
            0x55ee1b70157d307c5170c8af5f2b31c6u128,
            [0xc1824000, 0x41242000, 0x464e2000, 0x3f800000],
        ),
        (
            0x689ab4cd65392a8577c0afe45557a94au128,
            [0xc279a000, 0x427b0000, 0xbf386000, 0x3f800000],
        ),
        (
            0xc6be223e628127b136f9e4d35d5c5eaeu128,
            [0x39dd0000, 0x43106000, 0xbd306000, 0x3f800000],
        ),
        (
            0x3ba9173fd4853f9d7c4810b53a2e43d2u128,
            [0x3d638000, 0x442b8000, 0xc4a78000, 0x3f800000],
        ),
        (
            0x4a6a13de2e1d22d331e8e5479b3116f6u128,
            [0xbfe0e000, 0x45bfe000, 0xbd784000, 0x3f800000],
        ),
        (
            0x1ca08c58f9812d698c3fdfc8af4010bau128,
            [0x46b68000, 0x477fe000, 0x440c8000, 0x3f800000],
        ),
        (
            0x6b01193bb6c13b0d75b62a357bc57afeu128,
            [0x39fd8000, 0x427b6000, 0xb7600000, 0x3f800000],
        ),
        (
            0x8ebf7d2d69e8c9d1b9acf20ce8f9d123u128,
            [0xc4868000, 0x44dea000, 0xbfe6e000, 0x3f800000],
        ),
        (
            0x2c7c78ffeb434c935d9300a909dc2167u128,
            [0x3c180000, 0x42614000, 0xc51c6000, 0x3f800000],
        ),
        (
            0xd74975e5ad66da967c69784bb8f0552bu128,
            [0xba8a8000, 0xbba20000, 0x3b8de000, 0x3f800000],
        ),
        (
            0x76c0955ee677ec3c32dd1ea63fa81b2fu128,
            [0xc0216000, 0x46722000, 0x4400e000, 0x3f800000],
        ),
    ];
    for (block, expected) in samples {
        let b = texture(
            0x00800000,
            Format::Bc6hSf16,
            [4, 4, 1],
            0,
            &[block.to_le_bytes().to_vec()],
        );
        let t = Tex::parse(&b).unwrap();
        let pixels = t.mip(0).unwrap().decode_rgba32f(256).unwrap();
        assert_eq!(
            pixels[60..64]
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
#[test]
fn bc6h_reserved_modes_are_black_and_arbitrary_blocks_do_not_panic() {
    for mode in [19u8, 23, 27, 31] {
        let mut block = [255; 16];
        block[0] = (block[0] & !31) | mode;
        let b = texture(
            0x00800000,
            Format::Bc6hSf16,
            [1, 1, 1],
            0,
            &[block.to_vec()],
        );
        assert_eq!(
            Tex::parse(&b)
                .unwrap()
                .mip(0)
                .unwrap()
                .decode_rgba32f(16)
                .unwrap(),
            [0., 0., 0., 1.]
        );
    }
    let mut state = 0xb6c6u32;
    for mode in 0..32u8 {
        for _ in 0..64 {
            let mut block = [0; 16];
            for v in &mut block {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *v = state as u8;
            }
            block[0] = (block[0] & !31) | mode;
            let b = texture(
                0x00800000,
                Format::Bc6hSf16,
                [3, 3, 1],
                0,
                &[block.to_vec()],
            );
            let m = Tex::parse(&b).unwrap().mip(0).unwrap();
            let floats = m.decode_rgba32f(144).unwrap();
            assert_eq!(floats.len(), 36);
            assert!(floats.chunks_exact(4).all(|p| p[3] == 1.));
            assert_eq!(m.decode_rgba8(36).unwrap().len(), 36);
        }
    }
}
#[test]
fn floating_output_keeps_hdr_negative_nonfinite_and_every_image() {
    let halfs: Vec<_> = [
        0x3c00u16, 0xbc00, 0x4000, 0x3800, 0x7c00, 0xfc00, 0x7e00, 0x8000,
    ]
    .into_iter()
    .flat_map(u16::to_le_bytes)
    .collect();
    let b = texture(0x10000000, Format::R16G16B16A16F, [1, 1, 1], 2, &[halfs]);
    let m = Tex::parse(&b).unwrap().mip(0).unwrap();
    let values = m.decode_rgba32f(32).unwrap();
    assert_eq!(values[..4], [1., -1., 2., 0.5]);
    assert_eq!(values[4], f32::INFINITY);
    assert_eq!(values[5], f32::NEG_INFINITY);
    assert!(values[6].is_nan());
    assert_eq!(values[7].to_bits(), (-0f32).to_bits());
    assert!(matches!(
        m.decode_rgba32f(31),
        Err(Error::OutputTooLarge { requested: 32, .. })
    ));
    assert_eq!(
        m.image(1, 0, 0).unwrap().decode_rgba32f(16).unwrap()[4..],
        []
    );
    assert_eq!(m.decode_rgba8(8).unwrap(), [255, 0, 255, 128, 255, 0, 0, 0]);
    for (format, raw, expected) in [
        (
            Format::R32F,
            (-2f32).to_le_bytes().to_vec(),
            [-2., -2., -2., 1.],
        ),
        (
            Format::R16G16F,
            [0, 0xc0, 0, 0x42].to_vec(),
            [-2., 3., 0., 1.],
        ),
        (
            Format::R32G32F,
            [-2f32, 3.].into_iter().flat_map(f32::to_le_bytes).collect(),
            [-2., 3., 0., 1.],
        ),
        (
            Format::R32G32B32A32F,
            [-2f32, 3., 4., 5.]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
            [-2., 3., 4., 5.],
        ),
    ] {
        let b = texture(0x00800000, format, [1, 1, 1], 0, &[raw]);
        assert_eq!(
            Tex::parse(&b)
                .unwrap()
                .mip(0)
                .unwrap()
                .decode_rgba32f(16)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn all_required_images_are_range_checked_and_invalid_inputs_never_panic() {
    let b = texture(
        0x02000000,
        Format::L8,
        [2, 2, 1],
        6,
        &[vec![0; 24], vec![0; 6]],
    );
    for n in 0..b.len() {
        assert!(Tex::parse(&b[..n]).is_err());
    }
    let mut state = 0x1801u32;
    for _ in 0..4096 {
        let mut bytes = b.clone();
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let i = state as usize % bytes.len();
        bytes[i] ^= (state >> 16) as u8;
        if let Ok(t) = Tex::parse(&bytes) {
            for m in t.mips() {
                let _ = m.decode_rgba8(4096);
                let _ = m.decode_rgba32f(4096);
            }
        }
    }
}

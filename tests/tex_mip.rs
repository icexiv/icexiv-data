//! Synthetic TEX headers and payloads, authored with little-endian byte writes.
//! Float and BC controls reuse the cases in tex.rs and tex_layouts.rs; no game
//! installation, external implementation or extracted pixel data is required.

use icexiv_data::tex::{CubeFace, Error, Format, Header, Layout, Mip, Tex};

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn texture(
    kind: u32,
    format: Format,
    dimensions: [u16; 3],
    array_size: u8,
    payloads: &[Vec<u8>],
) -> Vec<u8> {
    let mut bytes = vec![0; 80];
    put32(&mut bytes, 0, kind);
    put32(&mut bytes, 4, format as u32);
    for (index, value) in dimensions.into_iter().enumerate() {
        put16(&mut bytes, 8 + index * 2, value);
    }
    bytes[14] = payloads.len() as u8;
    bytes[15] = array_size;
    for (index, payload) in payloads.iter().enumerate() {
        let offset = bytes.len() as u32;
        put32(&mut bytes, 28 + index * 4, offset);
        bytes.extend_from_slice(payload);
    }
    bytes
}

fn volume() -> Vec<u8> {
    // Same rectangular six-level profile as the existing layout control.
    let payloads = [2048, 256, 64, 16, 8, 4]
        .into_iter()
        .enumerate()
        .map(|(index, size)| [index as u8, 40, 80, 120].repeat(size / 4))
        .collect::<Vec<_>>();
    let mut bytes = texture(0x0100_0000, Format::B8G8R8A8, [8, 32, 2], 0, &payloads);
    bytes[80 + 1024..80 + 2048].copy_from_slice(&[9, 40, 80, 120].repeat(256));
    bytes[14] |= 0x80;
    put32(&mut bytes, 20, 1);
    put32(&mut bytes, 24, 5);
    put32(&mut bytes, 76, 0x1234_5678); // Preserve inactive offset words too.
    bytes
}

fn assert_same_mip(selected: Mip<'_>, full: Mip<'_>) {
    assert_eq!(selected.source_index(), full.source_index());
    assert_eq!(selected.source_mip_count(), full.source_mip_count());
    assert_eq!(selected.width(), full.width());
    assert_eq!(selected.height(), full.height());
    assert_eq!(selected.depth(), full.depth());
    assert_eq!(selected.layer_count(), full.layer_count());
    assert_eq!(selected.face_count(), full.face_count());
    assert_eq!(selected.format(), full.format());
    assert_eq!(selected.data(), full.data());
    assert_eq!(
        selected.decode_rgba8(4096).unwrap(),
        full.decode_rgba8(4096).unwrap()
    );
    if selected.format().float_channels().is_some() {
        assert_eq!(
            selected
                .decode_rgba32f(16384)
                .unwrap()
                .into_iter()
                .map(f32::to_bits)
                .collect::<Vec<_>>(),
            full.decode_rgba32f(16384)
                .unwrap()
                .into_iter()
                .map(f32::to_bits)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(selected.images().len(), full.images().len());
    for (selected, full) in selected.images().zip(full.images()) {
        assert_eq!(selected.data(), full.data());
        assert_eq!(selected.width(), full.width());
        assert_eq!(selected.height(), full.height());
    }
}

#[test]
fn six_level_volume_header_binds_base_without_requiring_the_missing_tail() {
    let bytes = volume();
    let short = &bytes[..2128];
    let original: [u8; 80] = short[..80].try_into().unwrap();
    assert_eq!(
        Tex::parse(short).unwrap_err(),
        Error::InvalidData("truncated mip payload")
    );

    // A bound mip borrows only its payload and outlives the Header value.
    let base = {
        let header = {
            let mut temporary = original;
            let header = Header::parse(&temporary).unwrap();
            temporary.fill(0);
            header
        };
        assert_eq!(header.raw_bytes(), &original);
        assert_eq!(Header::parse(short).unwrap().raw_bytes(), &original);
        assert_eq!(header.layout(), Layout::Volume);
        assert_eq!(header.format(), Format::B8G8R8A8);
        assert_eq!(header.mip_count(), 6);
        assert_eq!(header.encoded_len(), 2476);
        assert_eq!(header.mip_range(0).unwrap(), 80..2128);
        assert_eq!(header.mip_range(5).unwrap(), 2472..2476);
        header.mip(0, &short[80..]).unwrap()
    };
    assert_eq!((base.source_index(), base.source_mip_count()), (0, 6));
    assert_eq!((base.width(), base.height(), base.depth()), (8, 32, 2));
    assert_eq!(base.image_count(), 2);
    assert_eq!(base.data().as_ptr(), short[80..].as_ptr());
    assert_eq!(base.image(0, 0, 1).unwrap().data().len(), 1024);
    let rgba = base.decode_rgba8(2048).unwrap();
    assert_eq!(&rgba[..1024], [80, 40, 0, 120].repeat(256));
    assert_eq!(&rgba[1024..], [80, 40, 9, 120].repeat(256));
    assert_eq!(
        base.decode_rgba8(2047),
        Err(Error::OutputTooLarge {
            requested: 2048,
            limit: 2047,
        })
    );

    let full = Tex::parse(&bytes).unwrap();
    assert!(full.mip_unknown_flag());
    assert_eq!(full.lod_indices(), [0, 1, 5]);
    assert_same_mip(base, full.mip(0).unwrap());
    let header = Header::parse(&bytes[..80]).unwrap();
    for (index, full_mip) in full.mips().enumerate() {
        let payload = &bytes[header.mip_range(index).unwrap()];
        let selected = header.mip(index, payload).unwrap();
        assert_eq!(
            (selected.source_index(), selected.source_mip_count()),
            (index, 6)
        );
        assert_same_mip(selected, full_mip);
    }
}

#[test]
fn mip_binding_requires_exact_length_and_preserves_index_errors() {
    let bytes = volume();
    let header = Header::parse(&bytes).unwrap();
    for (index, expected) in [2048, 256, 64, 16, 8, 4].into_iter().enumerate() {
        let payload = &bytes[header.mip_range(index).unwrap()];
        let mut extra = payload.to_vec();
        extra.push(0);
        for (input, actual) in [
            (&payload[..expected - 1], expected - 1),
            (&extra[..], expected + 1),
        ] {
            assert_eq!(
                header.mip(index, input).unwrap_err(),
                Error::MipSizeMismatch {
                    index,
                    expected,
                    actual,
                }
            );
        }
    }
    assert_eq!(
        header.mip(0, &bytes[80..1104]).unwrap_err(),
        Error::MipSizeMismatch {
            index: 0,
            expected: 2048,
            actual: 1024,
        }
    );
    for index in [6, 7, usize::MAX] {
        let expected = Error::MipOutOfBounds { index, count: 6 };
        assert_eq!(header.mip_range(index).unwrap_err(), expected);
        assert_eq!(header.mip(index, &[]).unwrap_err(), expected);
        assert_eq!(
            Tex::parse(&bytes).unwrap().mip(index).unwrap_err(),
            expected
        );
    }
}

#[test]
fn packed_volume_profile_is_explicit_and_computes_consecutive_ranges() {
    let mut bytes = volume();
    let declared = Header::parse(&bytes).unwrap();
    assert_eq!(
        Header::parse_packed_volume(&bytes).unwrap_err(),
        Error::InvalidData("not the packed volume header profile")
    );
    for index in 0..6 {
        put32(&mut bytes, 28 + index * 4, 80);
    }
    assert_eq!(
        Header::parse(&bytes[..80]).unwrap_err(),
        Error::InvalidData("mip overlaps header or previous mip")
    );
    assert!(Tex::parse(&bytes).is_err());
    let header = Header::parse_packed_volume(&bytes[..80]).unwrap();
    assert_eq!(header.raw_bytes().as_slice(), &bytes[..80]);
    assert_eq!(header.mip_count(), 6);
    assert_eq!(header.encoded_len(), declared.encoded_len());
    let full = Tex::parse_packed_volume(&bytes).unwrap();
    assert!(full.uses_packed_volume_layout());
    assert_eq!(full.declared_mip_offsets(), [80; 6]);
    for (index, full_mip) in full.mips().enumerate() {
        let range = header.mip_range(index).unwrap();
        assert_eq!(range, declared.mip_range(index).unwrap());
        assert_same_mip(header.mip(index, &bytes[range]).unwrap(), full_mip);
        assert_eq!(
            (full_mip.source_index(), full_mip.source_mip_count()),
            (index, 6)
        );
    }
    let short = &bytes[..2128];
    let short_header = Header::parse_packed_volume(short).unwrap();
    assert_same_mip(
        short_header.mip(0, &short[80..]).unwrap(),
        full.mip(0).unwrap(),
    );
    assert_eq!(
        Tex::parse_packed_volume(short).unwrap_err(),
        Error::InvalidData("truncated mip payload")
    );

    for offset in [0, 79, 81, u32::MAX] {
        let mut invalid = bytes.clone();
        put32(&mut invalid, 32, offset);
        assert_eq!(
            Header::parse_packed_volume(&invalid).unwrap_err(),
            Error::InvalidData("not the packed volume header profile")
        );
    }
    put32(&mut bytes, 0, 0x0080_0000);
    put16(&mut bytes, 12, 1);
    assert_eq!(
        Header::parse_packed_volume(&bytes).unwrap_err(),
        Error::InvalidData("not the packed volume header profile")
    );
}

#[test]
fn declared_ranges_include_gaps_in_logical_length_but_not_in_mip_payloads() {
    let mut bytes = texture(
        0x0080_0000,
        Format::L8,
        [4, 2, 1],
        0,
        &[vec![3; 8], vec![5; 2], vec![7]],
    );
    bytes.resize(128, 0xaa);
    for (index, offset) in [96, 112, 120].into_iter().enumerate() {
        put32(&mut bytes, 28 + index * 4, offset);
    }
    bytes[96..104].fill(3);
    bytes[112..114].fill(5);
    bytes[120] = 7;
    let header = Header::parse(&bytes[..80]).unwrap();
    assert_eq!(header.encoded_len(), 121);
    let full = Tex::parse(&bytes).unwrap();
    assert_eq!(full.mips().len(), 3);
    for (index, range) in [96..104, 112..114, 120..121].into_iter().enumerate() {
        assert_eq!(header.mip_range(index).unwrap(), range);
        let mip = header.mip(index, &bytes[range]).unwrap();
        assert_same_mip(mip, full.mip(index).unwrap());
    }
    assert_eq!(
        header.mip(1, &bytes[112..120]).unwrap_err(),
        Error::MipSizeMismatch {
            index: 1,
            expected: 2,
            actual: 8,
        }
    );
    assert!(Tex::parse(&bytes[..121]).is_ok());
    assert_eq!(
        Tex::parse(&bytes[..120]).unwrap_err(),
        Error::InvalidData("truncated mip payload")
    );
}

#[test]
fn array_float_mips_keep_all_layers_and_match_full_texture_and_surface_decoders() {
    let first: Vec<_> = [0x3c00u16, 0xbc00, 0x4000, 0x3800]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect();
    let second: Vec<_> = [0x7c00u16, 0xfc00, 0x7e00, 0x8000]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect();
    let payloads = [
        [first.repeat(4), second.repeat(4)].concat(),
        [second, first].concat(),
    ];
    let bytes = texture(0x1000_0000, Format::R16G16B16A16F, [2, 2, 1], 2, &payloads);
    let header = Header::parse(&bytes[..80]).unwrap();
    let full = Tex::parse(&bytes).unwrap();
    assert_eq!(header.layout(), Layout::D2Array);
    for (index, payload) in payloads.iter().enumerate() {
        assert_same_mip(
            header.mip(index, payload).unwrap(),
            full.mip(index).unwrap(),
        );
    }
    let mip = header.mip(1, &payloads[1]).unwrap();
    assert_eq!((mip.source_index(), mip.source_mip_count()), (1, 2));
    assert_eq!((mip.width(), mip.height(), mip.depth()), (1, 1, 1));
    assert_eq!((mip.layer_count(), mip.face_count()), (2, 1));
    assert_eq!(mip.data().as_ptr(), payloads[1].as_ptr());
    assert_eq!(
        mip.decode_rgba8(8).unwrap(),
        [255, 0, 0, 0, 255, 0, 255, 128]
    );
    let values = mip.decode_rgba32f(32).unwrap();
    assert_eq!(values[0], f32::INFINITY);
    assert_eq!(values[1], f32::NEG_INFINITY);
    assert!(values[2].is_nan());
    assert_eq!(values[3].to_bits(), (-0f32).to_bits());
    assert_eq!(&values[4..], [1., -1., 2., 0.5]);
    assert_eq!(
        mip.image(1, 0, 0).unwrap().decode_rgba32f(16).unwrap(),
        [1., -1., 2., 0.5]
    );
    assert_eq!(
        mip.decode_rgba32f(31),
        Err(Error::OutputTooLarge {
            requested: 32,
            limit: 31
        })
    );
    assert_eq!(
        mip.decode_rgba8(7),
        Err(Error::OutputTooLarge {
            requested: 8,
            limit: 7
        })
    );
    assert_eq!(
        mip.image(1, 0, 0).unwrap().decode_rgba32f(15),
        Err(Error::OutputTooLarge {
            requested: 16,
            limit: 15
        })
    );
    assert_eq!(
        header.mip(1, &payloads[1][..8]).unwrap_err(),
        Error::MipSizeMismatch {
            index: 1,
            expected: 16,
            actual: 8
        }
    );
    assert_eq!(
        mip.image(2, 0, 0).unwrap_err(),
        Error::SurfaceOutOfBounds {
            layer: 2,
            face: 0,
            z: 0
        }
    );
    assert_eq!(
        mip.cube_face(CubeFace::PositiveX).unwrap_err(),
        Error::NotCubeTexture
    );
}

#[test]
fn cube_bc_mips_bind_every_face_and_keep_block_rounding_at_small_levels() {
    let colors = [0xf800u16, 0x07e0, 0x001f, 0xffff, 0, 0xffe0];
    let rgba = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255; 4],
        [0, 0, 0, 255],
        [255, 255, 0, 255],
    ];
    let payloads = (0..3)
        .map(|index| {
            (0..6)
                .flat_map(|face| {
                    let mut block = [0; 8];
                    put16(&mut block, 0, colors[(index + face) % 6]);
                    block
                })
                .collect()
        })
        .collect::<Vec<_>>();
    for count in [0, 6] {
        let bytes = texture(0x0200_0000, Format::Bc1, [4, 4, 1], count, &payloads);
        let header = Header::parse(&bytes[..80]).unwrap();
        let full = Tex::parse(&bytes).unwrap();
        assert_eq!(header.layout(), Layout::Cube);
        for (index, payload) in payloads.iter().enumerate() {
            let mip = header.mip(index, payload).unwrap();
            assert_eq!((mip.source_index(), mip.source_mip_count()), (index, 3));
            assert_eq!(mip.image_count(), 6);
            assert_eq!(mip.data().len(), 48);
            assert_same_mip(mip, full.mip(index).unwrap());
            for face in CubeFace::ALL {
                let surface = mip.cube_face(face).unwrap();
                let pixels = usize::from(surface.width()) * usize::from(surface.height());
                assert_eq!(
                    surface.decode_rgba8(pixels * 4).unwrap(),
                    rgba[(index + face.index()) % 6].repeat(pixels)
                );
            }
        }
        assert_eq!(
            header.mip(2, &payloads[2][..8]).unwrap_err(),
            Error::MipSizeMismatch {
                index: 2,
                expected: 48,
                actual: 8
            }
        );
        assert!(Tex::parse(&bytes[..bytes.len() - 8]).is_err());
    }
}

#[test]
fn header_and_whole_texture_share_header_range_and_dimension_validation() {
    let bytes = texture(
        0x0080_0000,
        Format::L8,
        [4, 4, 1],
        0,
        &[vec![0; 16], vec![0; 4], vec![0]],
    );
    for end in 0..80 {
        assert_eq!(
            Header::parse(&bytes[..end]).unwrap_err(),
            Error::InvalidData("truncated header")
        );
        assert_eq!(
            Header::parse_packed_volume(&bytes[..end]).unwrap_err(),
            Error::InvalidData("truncated header")
        );
    }
    let reject = |input: &[u8]| {
        assert_eq!(
            Header::parse(&input[..80]).unwrap_err(),
            Tex::parse(input).unwrap_err()
        );
    };
    for (offset, value) in [
        (28, 0),
        (28, 79),
        (28, u32::MAX),
        (32, 0),
        (32, 80),
        (32, 95),
        (36, 99),
    ] {
        let mut invalid = bytes.clone();
        put32(&mut invalid, offset, value);
        reject(&invalid);
    }
    for (offset, value) in [(8, 0), (10, 0), (12, 0), (12, 2)] {
        let mut invalid = bytes.clone();
        put16(&mut invalid, offset, value);
        reject(&invalid);
    }
    for count in [0, 4, 13, 14, 127, 128, 255] {
        let mut invalid = bytes.clone();
        invalid[14] = count;
        reject(&invalid);
    }
    for attributes in [0, 0x0040_0000, 0x0480_0000, 0x0180_0000] {
        let mut invalid = bytes.clone();
        put32(&mut invalid, 0, attributes);
        assert_eq!(
            Header::parse(&invalid).unwrap_err(),
            Error::UnsupportedLayout(attributes)
        );
        reject(&invalid);
    }
    let mut invalid = bytes.clone();
    invalid[15] = 1;
    reject(&invalid);
    put32(&mut invalid, 0, 0x0200_0000);
    assert_eq!(
        Header::parse(&invalid).unwrap_err(),
        Error::UnsupportedCubeImageCount(1)
    );
    reject(&invalid);
    invalid[15] = 6;
    put16(&mut invalid, 10, 2);
    reject(&invalid);
    let mut invalid = bytes;
    put32(&mut invalid, 4, u32::MAX);
    assert_eq!(
        Header::parse(&invalid).unwrap_err(),
        Error::UnsupportedFormat(u32::MAX)
    );
    reject(&invalid);
}

#[test]
fn header_ranges_use_checked_logical_offsets_without_allocating_payloads() {
    let mut raw = [0; 80];
    put32(&mut raw, 0, 0x0080_0000);
    put32(&mut raw, 4, Format::L8 as u32);
    put16(&mut raw, 8, 1);
    put16(&mut raw, 10, 1);
    put16(&mut raw, 12, 1);
    raw[14] = 1;
    put32(&mut raw, 28, u32::MAX);
    let start = u32::MAX as usize;
    if let Some(end) = start.checked_add(1) {
        let header = Header::parse(&raw).unwrap();
        assert_eq!(header.encoded_len(), end);
        assert_eq!(header.mip_range(0).unwrap(), start..end);
        assert_eq!(
            header.mip(0, &[7]).unwrap().decode_rgba8(4).unwrap(),
            [7, 7, 7, 255]
        );
        assert_eq!(
            Tex::parse(&raw).unwrap_err(),
            Error::InvalidData("truncated mip payload")
        );
    } else {
        assert_eq!(
            Header::parse(&raw).unwrap_err(),
            Error::InvalidData("mip range overflow")
        );
    }
}

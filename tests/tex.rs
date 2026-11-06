//! Newly authored synthetic TEX fixtures, using direct little-endian byte writes.
//! No extracted game data or external implementation's fixtures are used.

use icexiv_data::tex::{Error, Format, Tex};

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn texture(format: Format, width: u16, height: u16, payloads: &[&[u8]]) -> Vec<u8> {
    let mut bytes = vec![0; 80];
    put32(&mut bytes, 0, 0x0080_0000);
    put32(&mut bytes, 4, format as u32);
    put16(&mut bytes, 8, width);
    put16(&mut bytes, 10, height);
    put16(&mut bytes, 12, 1);
    bytes[14] = payloads.len() as u8;
    for (mip, payload) in payloads.iter().enumerate() {
        let start = bytes.len() as u32;
        put32(&mut bytes, 28 + mip * 4, start);
        bytes.extend_from_slice(payload);
    }
    bytes
}

fn decode(format: Format, width: u16, height: u16, payload: &[u8]) -> Vec<u8> {
    let bytes = texture(format, width, height, &[payload]);
    Tex::parse(&bytes)
        .unwrap()
        .mip(0)
        .unwrap()
        .decode_rgba8(4096)
        .unwrap()
}

fn pixels(bytes: &[u8]) -> Vec<[u8; 4]> {
    bytes
        .chunks_exact(4)
        .map(|p| p.try_into().unwrap())
        .collect()
}

fn color_block(a: u16, b: u16, indices: &[u8; 16]) -> [u8; 8] {
    let mut block = [0; 8];
    put16(&mut block, 0, a);
    put16(&mut block, 2, b);
    let packed = indices
        .iter()
        .enumerate()
        .fold(0, |bits, (i, &index)| bits | (u32::from(index) << (2 * i)));
    put32(&mut block, 4, packed);
    block
}

fn bc3_block(a: u8, b: u8, indices: &[u8; 16]) -> Vec<u8> {
    let mut block = vec![a, b];
    let packed = indices.iter().enumerate().fold(0u64, |bits, (i, &index)| {
        bits | (u64::from(index) << (3 * i))
    });
    block.extend_from_slice(&packed.to_le_bytes()[..6]);
    block.extend_from_slice(&color_block(0xf800, 0x001f, &[0; 16]));
    block
}

#[test]
fn bgra_is_rgba_in_row_order_without_premultiplication() {
    assert_eq!(
        pixels(&decode(
            Format::B8G8R8A8,
            2,
            2,
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0],
        )),
        [[3, 2, 1, 4], [7, 6, 5, 8], [11, 10, 9, 12], [15, 14, 13, 0]]
    );
    assert_eq!(
        decode(Format::B8G8R8X8, 1, 1, &[1, 2, 3, 0]),
        [3, 2, 1, 255]
    );
}

#[test]
fn luminance_and_alpha_have_explicit_channel_semantics() {
    assert_eq!(
        pixels(&decode(Format::L8, 3, 1, &[0, 128, 255])),
        [[0, 0, 0, 255], [128, 128, 128, 255], [255; 4]]
    );
    assert_eq!(
        pixels(&decode(Format::A8, 3, 1, &[0, 128, 255])),
        [[255, 255, 255, 0], [255, 255, 255, 128], [255; 4]]
    );
}

#[test]
fn packed_integer_formats_expand_channels_and_alpha() {
    assert_eq!(
        pixels(&decode(Format::B4G4R4A4, 2, 1, &[0x21, 0x43, 0xff, 0xff])),
        [[51, 34, 17, 68], [255; 4]]
    );
    let words = [0x8000 | (17 << 10) | (9 << 5) | 3, 0x7fff];
    let payload: Vec<_> = words.into_iter().flat_map(u16::to_le_bytes).collect();
    assert_eq!(
        pixels(&decode(Format::B5G5R5A1, 2, 1, &payload)),
        [[140, 74, 24, 255], [255, 255, 255, 0]]
    );
}

#[test]
fn bc1_four_colors_are_selected_in_little_endian_row_order() {
    let indices = std::array::from_fn(|i| (i % 4) as u8);
    let block = color_block(0xf800, 0x001f, &indices);
    let expected = [
        [255, 0, 0, 255],
        [0, 0, 255, 255],
        [170, 0, 85, 255],
        [85, 0, 170, 255],
    ];
    let result = pixels(&decode(Format::Bc1, 4, 4, &block));
    for row in result.chunks_exact(4) {
        assert_eq!(row, expected);
    }
}

#[test]
fn bc1_three_color_mode_includes_transparent_black_and_equal_endpoints() {
    let indices = std::array::from_fn(|i| (i % 4) as u8);
    let block = color_block(0x001f, 0xf800, &indices);
    assert_eq!(
        &pixels(&decode(Format::Bc1, 4, 4, &block))[..4],
        [
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [127, 0, 127, 255],
            [0; 4]
        ]
    );
    let block = color_block(0xffff, 0xffff, &[3; 16]);
    assert_eq!(decode(Format::Bc1, 1, 1, &block), [0; 4]);
}

#[test]
fn bc2_explicit_alpha_and_forced_four_color_mode() {
    let mut block = Vec::new();
    let alpha = (0..16).fold(0u64, |bits, i| bits | (i << (4 * i)));
    block.extend_from_slice(&alpha.to_le_bytes());
    block.extend_from_slice(&color_block(0x001f, 0xf800, &[3; 16]));
    let result = pixels(&decode(Format::Bc2, 4, 4, &block));
    for (index, pixel) in result.iter().enumerate() {
        assert_eq!(*pixel, [170, 0, 85, index as u8 * 17]);
    }
}

#[test]
fn bc3_interpolates_all_eight_alpha_codes_in_both_modes() {
    let indices = std::array::from_fn(|i| (i % 8) as u8);
    for (a, b, expected) in [
        (240, 30, [240, 30, 210, 180, 150, 120, 90, 60]),
        (10, 110, [10, 110, 30, 50, 70, 90, 0, 255]),
        (40, 40, [40, 40, 40, 40, 40, 40, 0, 255]),
    ] {
        let block = bc3_block(a, b, &indices);
        let result = pixels(&decode(Format::Bc3, 4, 4, &block));
        for (i, pixel) in result.iter().enumerate() {
            assert_eq!(*pixel, [255, 0, 0, expected[i % 8]]);
        }
    }
}

#[test]
fn bc3_color_is_four_color_even_with_reversed_endpoints() {
    let mut block = bc3_block(17, 17, &[0; 16]);
    block[8..].copy_from_slice(&color_block(0x001f, 0xf800, &[3; 16]));
    assert_eq!(decode(Format::Bc3, 1, 1, &block), [170, 0, 85, 17]);
}

#[test]
fn rgb565_green_and_non_extreme_channels_use_bit_replication() {
    let word = (17 << 11) | (33 << 5) | 3;
    let block = color_block(word, 0, &[0; 16]);
    assert_eq!(decode(Format::Bc1, 1, 1, &block), [140, 134, 24, 255]);
}

#[test]
fn partial_blocks_clip_to_dimensions_and_blocks_are_row_major() {
    let mut payload = Vec::new();
    for color in [0xf800, 0x07e0, 0x001f, 0xffff] {
        payload.extend_from_slice(&color_block(color, 0, &[0; 16]));
    }
    let result = pixels(&decode(Format::Bc1, 5, 5, &payload));
    for y in 0..5 {
        for x in 0..5 {
            let expected = match (x < 4, y < 4) {
                (true, true) => [255, 0, 0, 255],
                (false, true) => [0, 255, 0, 255],
                (true, false) => [0, 0, 255, 255],
                (false, false) => [255; 4],
            };
            assert_eq!(result[y * 5 + x], expected);
        }
    }
    assert_eq!(decode(Format::Bc1, 1, 1, &payload[..8]), [255, 0, 0, 255]);
}

#[test]
fn mip_metadata_borrows_exact_spans_and_separates_the_high_count_flag() {
    let bytes = texture(Format::L8, 4, 2, &[&[3; 8], &[5; 2], &[7]]);
    let mut padded = bytes[..80].to_vec();
    padded[14] |= 0x80;
    put32(&mut padded, 16, 0); // LOD entries are mip indices, not byte offsets.
    put32(&mut padded, 20, 1);
    put32(&mut padded, 24, 2);
    put32(&mut padded, 28, 96);
    put32(&mut padded, 32, 112);
    put32(&mut padded, 36, 120);
    padded.resize(128, 0xaa);
    padded[96..104].copy_from_slice(&[3; 8]);
    padded[112..114].copy_from_slice(&[5; 2]);
    padded[120] = 7;
    let tex = Tex::parse(&padded).unwrap();
    assert_eq!((tex.width(), tex.height(), tex.mip_count()), (4, 2, 3));
    assert_eq!(tex.attributes(), 0x0080_0000);
    assert_eq!(tex.format(), Format::L8);
    assert!(tex.mip_unknown_flag());
    assert_eq!(tex.mips().len(), 3);
    let mips: Vec<_> = tex.mips().collect();
    assert_eq!((mips[1].width(), mips[1].height()), (2, 1));
    assert_eq!((mips[2].width(), mips[2].height()), (1, 1));
    assert_eq!(mips[1].data(), [5; 2]);
    assert_eq!(mips[1].data().as_ptr(), padded[112..].as_ptr());
    assert_eq!(mips[1].format(), Format::L8);
    assert_eq!(mips[2].decode_rgba8(4).unwrap(), [7, 7, 7, 255]);
    for index in [3, usize::MAX] {
        assert_eq!(
            tex.mip(index).unwrap_err(),
            Error::MipOutOfBounds { index, count: 3 }
        );
    }
}

#[test]
fn small_compressed_mips_still_require_a_whole_block() {
    let block = color_block(0x07e0, 0, &[0; 16]);
    let bytes = texture(Format::Bc1, 4, 4, &[&block, &block, &block]);
    let tex = Tex::parse(&bytes).unwrap();
    assert_eq!(tex.mip(2).unwrap().data().len(), 8);
    assert_eq!(
        tex.mip(2).unwrap().decode_rgba8(4).unwrap(),
        [0, 255, 0, 255]
    );
    assert!(Tex::parse(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn decode_budget_is_checked_before_allocating_and_includes_alpha() {
    let bytes = texture(Format::L8, 2, 1, &[&[1, 2]]);
    let mip = Tex::parse(&bytes).unwrap().mip(0).unwrap();
    for limit in [0, 7] {
        assert_eq!(
            mip.decode_rgba8(limit),
            Err(Error::OutputTooLarge {
                requested: 8,
                limit
            })
        );
    }
    assert_eq!(mip.decode_rgba8(8).unwrap().len(), 8);
}

#[test]
fn every_truncated_prefix_is_rejected() {
    for (format, payload) in [
        (Format::B8G8R8A8, vec![0; 64]),
        (Format::Bc1, vec![0; 8]),
        (Format::Bc2, vec![0; 16]),
        (Format::Bc3, vec![0; 16]),
    ] {
        let bytes = texture(format, 4, 4, &[&payload]);
        for end in 0..bytes.len() {
            assert!(
                Tex::parse(&bytes[..end]).is_err(),
                "format {format:?}, end {end}"
            );
        }
    }
}

#[test]
fn unsupported_formats_and_layouts_are_not_guessed() {
    let original = texture(Format::L8, 1, 1, &[&[0]]);
    for id in [0, 0x1452, u32::MAX] {
        let mut bytes = original.clone();
        put32(&mut bytes, 4, id);
        assert_eq!(
            Tex::parse(&bytes).unwrap_err(),
            Error::UnsupportedFormat(id)
        );
    }
    for attributes in [0, 0x0040_0000, 0x0480_0000] {
        let mut bytes = original.clone();
        put32(&mut bytes, 0, attributes);
        assert_eq!(
            Tex::parse(&bytes).unwrap_err(),
            Error::UnsupportedLayout(attributes)
        );
    }
}

#[test]
fn invalid_dimensions_depth_array_and_mip_counts_are_rejected() {
    let original = texture(Format::L8, 2, 2, &[&[0; 4]]);
    for (offset, value) in [(8, 0), (10, 0), (12, 0), (12, 2)] {
        let mut bytes = original.clone();
        put16(&mut bytes, offset, value);
        assert!(matches!(Tex::parse(&bytes), Err(Error::InvalidData(_))));
    }
    for count in [0, 3, 13, 14, 127, 128, 255] {
        let mut bytes = original.clone();
        bytes[14] = count;
        assert!(matches!(Tex::parse(&bytes), Err(Error::InvalidData(_))));
    }
    let mut bytes = original;
    bytes[15] = 1;
    assert!(matches!(Tex::parse(&bytes), Err(Error::InvalidData(_))));
}

#[test]
fn bad_offsets_and_overlapping_or_missing_later_mips_are_rejected() {
    let original = texture(Format::L8, 2, 2, &[&[0; 4], &[1]]);
    for (offset, value) in [
        (28, 0),
        (28, 79),
        (28, u32::MAX),
        (32, 0),
        (32, 80),
        (32, 83),
        (32, 85),
    ] {
        let mut bytes = original.clone();
        put32(&mut bytes, offset, value);
        assert!(matches!(Tex::parse(&bytes), Err(Error::InvalidData(_))));
    }
}

#[test]
fn header_mutations_and_arbitrary_block_bits_do_not_panic() {
    let original = texture(Format::Bc3, 4, 4, &[&[0; 16]]);
    for offset in 0..80 {
        for value in [0, 1, 0x7f, 0x80, 0xff] {
            let mut bytes = original.clone();
            bytes[offset] = value;
            if let Ok(tex) = Tex::parse(&bytes) {
                for mip in tex.mips() {
                    let _ = mip.decode_rgba8(1024);
                }
            }
        }
    }
    let mut seed = 0x12345678u32;
    for format in [Format::Bc1, Format::Bc2, Format::Bc3] {
        for _ in 0..128 {
            let mut payload = [0; 64];
            for byte in &mut payload {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *byte = seed as u8;
            }
            let length = if format == Format::Bc1 { 32 } else { 64 };
            assert_eq!(decode(format, 5, 7, &payload[..length]).len(), 5 * 7 * 4);
        }
    }
}

fn bc4_block(a: u8, b: u8, indices: &[u8; 16]) -> Vec<u8> {
    let mut block = vec![a, b];
    let bits = indices.iter().enumerate().fold(0u64, |bits, (i, &index)| {
        bits | (u64::from(index) << (3 * i))
    });
    block.extend_from_slice(&bits.to_le_bytes()[..6]);
    block
}

#[test]
fn bc4_uses_the_single_channel_palette_for_grey_output() {
    let indices = [0, 1, 2, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        pixels(&decode(Format::Bc4, 4, 1, &bc4_block(100, 50, &indices))),
        [
            [100, 100, 100, 255],
            [50, 50, 50, 255],
            [92, 92, 92, 255],
            [57, 57, 57, 255]
        ]
    );
    // a <= b adds the reserved zero and full values instead of six steps.
    let indices = [0, 1, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        pixels(&decode(Format::Bc4, 4, 1, &bc4_block(0, 255, &indices))),
        [[0, 0, 0, 255], [255; 4], [0, 0, 0, 255], [255; 4]]
    );
}

#[test]
fn bc5_decodes_red_and_green_blocks_with_a_full_alpha() {
    let mut block = bc4_block(0, 255, &[0; 16]);
    block.extend_from_slice(&bc4_block(255, 0, &[0; 16]));
    assert_eq!(
        pixels(&decode(Format::Bc5, 1, 1, &block)),
        [[0, 255, 0, 255]]
    );
}
#[test]
fn bc5_interpolation_modes_and_selectors_are_independent_per_channel() {
    // Matches the generic Pillow DDS CPU oracle; see BC4/BC5 provenance.
    let indices = std::array::from_fn(|i| (i % 8) as u8);
    let mut block = bc4_block(10, 200, &indices);
    block.extend_from_slice(&bc4_block(200, 10, &indices.map(|i| 7 - i)));
    let red = [10, 200, 48, 86, 124, 162, 0, 255];
    let green = [37, 64, 91, 118, 145, 172, 10, 200];
    let expected: Vec<_> = (0..16)
        .map(|i| [red[i % 8], green[i % 8], 0, 255])
        .collect();
    assert_eq!(pixels(&decode(Format::Bc5, 4, 4, &block)), expected);
}

#[test]
fn float_formats_clamp_and_round_to_eight_bit_channels() {
    assert_eq!(
        pixels(&decode(Format::R32F, 1, 1, &0.5f32.to_le_bytes())),
        [[128, 128, 128, 255]]
    );
    let mut payload = Vec::new();
    for value in [1.0f32, 0.0f32] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        pixels(&decode(Format::R32G32F, 1, 1, &payload)),
        [[255, 0, 0, 255]]
    );
    let mut payload = Vec::new();
    for value in [1.0f32, 0.5f32, -1.0f32, 2.0f32] {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        pixels(&decode(Format::R32G32B32A32F, 1, 1, &payload)),
        [[255, 128, 0, 255]]
    );
    let mut payload = Vec::new();
    for half in [0x3c00u16, 0x3800, 0x0000, 0x3c00] {
        payload.extend_from_slice(&half.to_le_bytes());
    }
    assert_eq!(
        pixels(&decode(Format::R16G16B16A16F, 1, 1, &payload)),
        [[255, 128, 0, 255]]
    );
}

#[test]
fn half_float_specials_and_non_finite_samples_are_deterministic() {
    let mut payload = Vec::new();
    for half in [0x7e00u16, 0x7c00] {
        payload.extend_from_slice(&half.to_le_bytes());
    }
    // NaN becomes zero and infinity clamps.
    assert_eq!(
        pixels(&decode(Format::R16G16F, 1, 1, &payload)),
        [[0, 255, 0, 255]]
    );
    let payload = [0xfc00u16, 0x0001]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(
        pixels(&decode(Format::R16G16F, 1, 1, &payload)),
        [[0, 0, 0, 255]]
    );
}

#[test]
fn two_half_channels_use_four_bytes_per_pixel_across_mips() {
    let first = [0x00, 0x3c, 0x00, 0x38].repeat(4);
    let second = [0x00, 0x38, 0x00, 0x3c];
    let bytes = texture(Format::R16G16F, 2, 2, &[&first, &second]);
    let tex = Tex::parse(&bytes).unwrap();
    assert_eq!(tex.mip(0).unwrap().data(), first);
    assert_eq!(tex.mip(1).unwrap().data(), second);
    assert_eq!(
        pixels(&tex.mip(0).unwrap().decode_rgba8(16).unwrap()),
        [[255, 128, 0, 255]; 4]
    );
    assert_eq!(
        pixels(&tex.mip(1).unwrap().decode_rgba8(4).unwrap()),
        [[128, 255, 0, 255]]
    );
}

fn bc7_mode6(endpoints: [u8; 8], p_bits: [u8; 2], indices: &[u8; 16]) -> Vec<u8> {
    let mut bits = 1u128 << 6;
    let mut position = 7;
    let mut write = |value: u32, count: usize| {
        bits |= u128::from(value) << position;
        position += count;
    };
    for (channel, pair) in endpoints.chunks_exact(2).enumerate() {
        let _ = channel;
        for &value in pair {
            write(u32::from(value), 7);
        }
    }
    for value in p_bits {
        write(u32::from(value), 1);
    }
    for (pixel, &index) in indices.iter().enumerate() {
        write(u32::from(index), if pixel == 0 { 3 } else { 4 });
    }
    assert_eq!(position, 128);
    bits.to_le_bytes().to_vec()
}

#[test]
fn bc7_single_subset_endpoints_and_indices_decode() {
    let zero = bc7_mode6([0; 8], [0, 0], &[0; 16]);
    assert!(
        pixels(&decode(Format::Bc7, 4, 4, &zero))
            .iter()
            .all(|pixel| *pixel == [0, 0, 0, 0])
    );
    let white = bc7_mode6([127; 8], [1, 1], &[0; 16]);
    assert!(
        pixels(&decode(Format::Bc7, 4, 4, &white))
            .iter()
            .all(|pixel| *pixel == [255; 4])
    );
    // The anchor pixel stores one bit less, so it cannot use the last index.
    let mut indices = [15u8; 16];
    indices[0] = 7;
    let gradient = bc7_mode6([0, 127, 0, 127, 0, 127, 0, 127], [0, 1], &indices);
    assert!(
        pixels(&decode(Format::Bc7, 4, 4, &gradient))[1..]
            .iter()
            .all(|pixel| *pixel == [255; 4])
    );
    let gradient = bc7_mode6([0, 127, 0, 127, 0, 127, 0, 127], [0, 1], &[0; 16]);
    assert!(
        pixels(&decode(Format::Bc7, 4, 4, &gradient))
            .iter()
            .all(|pixel| *pixel == [0, 0, 0, 0])
    );
}

#[test]
fn bc7_invalid_mode_is_an_error_not_noise() {
    assert!(matches!(
        Tex::parse(&texture(Format::Bc7, 4, 4, &[&[0u8; 16]]))
            .unwrap()
            .mip(0)
            .unwrap()
            .decode_rgba8(4096),
        Err(Error::InvalidData(_))
    ));
}

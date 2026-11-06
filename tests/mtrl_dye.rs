//! Independent synthetic half-row overrides; no game bytes embedded.
use icexiv_data::{
    mtrl::{
        ColorLayout, DyeOverrideProfile as Profile, DyeRow,
        dye::{self, Error},
    },
    stm::{Layout, Stm},
};

fn template(layout: Layout, specular: [u16; 3]) -> Vec<u8> {
    let n = layout.column_count();
    let mut bytes = vec![0; 16 + n * 2];
    bytes[..2].copy_from_slice(b"MS");
    bytes[2..4].copy_from_slice(
        &(if layout == Layout::Legacy {
            0x0101u16
        } else {
            0x0201
        })
        .to_le_bytes(),
    );
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    if layout == Layout::Dawntrail {
        bytes[6..8].copy_from_slice(&[3, 9]);
    }
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    for column in 0..n {
        let bits = if column == 1 {
            specular.to_vec()
        } else if column < 3 {
            vec![0x7d01 + column as u16, 0x8000, 0x4400]
        } else {
            vec![0x3000 + column as u16]
        };
        bytes.extend(bits.into_iter().flat_map(u16::to_le_bytes));
        let end = ((bytes.len() - 16 - n * 2) / 2) as u16;
        bytes[16 + column * 2..18 + column * 2].copy_from_slice(&end.to_le_bytes());
    }
    bytes
}

fn row(layout: ColorLayout, flags: u32) -> DyeRow {
    DyeRow {
        layout,
        raw: flags
            | if layout == ColorLayout::Legacy {
                1 << 5
            } else {
                1 << 16
            },
    }
}

#[test]
fn actual_writes_preserve_unselected_bits_and_profile_is_not_file_layout() {
    let base = std::array::from_fn(|i| 0x7100 + i as u16);
    for (profile, layout, starts) in [
        (Profile::Legacy, Layout::Legacy, &[0, 4, 8, 3, 7][..]),
        (
            Profile::Dawntrail,
            Layout::Dawntrail,
            &[0, 4, 8, 11, 18, 16, 12, 13, 14, 19, 27, 21][..],
        ),
    ] {
        let bytes = template(layout, [0x8000, 0x7d03, 0x7c00]);
        let stm = Stm::parse(&bytes).unwrap();
        for source in [ColorLayout::Legacy, ColorLayout::Dawntrail] {
            for column in 0..starts.len() {
                let dye = row(
                    source,
                    if source == ColorLayout::Legacy && column >= 5 {
                        0
                    } else {
                        1 << column
                    },
                );
                let result = dye::apply_selected_row(&base, dye, profile, stm, 254).unwrap();
                let width = if column < 3 { 3 } else { 1 };
                let start = starts[column];
                let active = column < if source == ColorLayout::Legacy { 5 } else { 12 };
                let mut expected = base;
                let mut mask = 0;
                if active {
                    let values = stm
                        .template(0)
                        .unwrap()
                        .column(column)
                        .unwrap()
                        .half_bits(254)
                        .unwrap();
                    expected[start..start + width].copy_from_slice(&values[..width]);
                    mask = ((1 << width) - 1) << start;
                }
                assert_eq!(result.half_bits, expected);
                assert_eq!(result.written_lane_mask, mask);
            }
        }
    }
    assert_eq!(base, std::array::from_fn(|i| 0x7100 + i as u16));
}

#[test]
fn zero_specular_is_a_bit_sentinel_not_a_numeric_zero_test() {
    let base = [0x3c00; 32];
    for specular in [[0, 0, 0], [0x8000, 0, 0], [0, 0x7d01, 0], [0, 0, 0x0001]] {
        let bytes = template(Layout::Dawntrail, specular);
        let stm = Stm::parse(&bytes).unwrap();
        let result = dye::apply_selected_row(
            &base,
            row(ColorLayout::Dawntrail, 2),
            Profile::Dawntrail,
            stm,
            1,
        )
        .unwrap();
        if specular == [0; 3] {
            assert_eq!(result.half_bits, base);
            assert_eq!(result.written_lane_mask, 0);
        } else {
            assert_eq!(&result.half_bits[4..7], &specular);
            assert_eq!(result.written_lane_mask, 0x70);
        }
    }
    let mut bytes = template(Layout::Dawntrail, [0x3c00; 3]);
    let start = 16 + 12 * 2;
    bytes[start..start + 6].fill(0);
    let stm = Stm::parse(&bytes).unwrap();
    let result = dye::apply_selected_row(
        &base,
        row(ColorLayout::Dawntrail, 3),
        Profile::Dawntrail,
        stm,
        1,
    )
    .unwrap();
    assert_eq!(&result.half_bits[..3], &[0; 3]);
    assert_eq!(&result.half_bits[4..7], &[0x3c00; 3]);
    // Diffuse zero is written; identical nonzero specular is still a write.
    assert_eq!(result.written_lane_mask, 0x77);
}

#[test]
fn channels_and_errors_do_not_silently_choose_a_fallback() {
    let bytes = template(Layout::Dawntrail, [0; 3]);
    let stm = Stm::parse(&bytes).unwrap();
    let base = [0x7d55; 32];
    for channel in 0..4 {
        let mut word = row(ColorLayout::Dawntrail, 4095);
        word.raw |= u32::from(channel) << 27;
        let applied = dye::apply_selected_row(&base, word, Profile::Dawntrail, stm, 1).unwrap();
        assert_eq!(
            dye::apply_channel_row(&base, word, Profile::Dawntrail, stm, 1, channel).unwrap(),
            Some(applied)
        );
        // A mismatching channel is skipped before profile/template/stain checks.
        assert_eq!(
            dye::apply_channel_row(&base, word, Profile::Legacy, stm, 0, (channel + 1) % 4)
                .unwrap(),
            None
        );
    }
    let modern = row(ColorLayout::Dawntrail, 4095);
    let legacy = DyeRow {
        raw: 0xffff0000 | (1 << 5) | 31,
        layout: ColorLayout::Legacy,
    };
    assert_eq!(legacy.template(), 1);
    assert_eq!(
        dye::apply_selected_row(&base, legacy, Profile::Dawntrail, stm, 1).unwrap(),
        dye::apply_selected_row(
            &base,
            row(ColorLayout::Legacy, 31),
            Profile::Dawntrail,
            stm,
            1
        )
        .unwrap()
    );
    assert_eq!(
        dye::apply_selected_row(&base, modern, Profile::Legacy, stm, 1),
        Err(Error::TemplateLayoutMismatch)
    );
    assert_eq!(
        dye::apply_selected_row(
            &base,
            DyeRow {
                raw: 2 << 16,
                ..modern
            },
            Profile::Dawntrail,
            stm,
            1
        ),
        Err(Error::MissingTemplate(2))
    );
    for id in [0, 255, u16::MAX] {
        assert!(matches!(
            dye::apply_selected_row(&base, modern, Profile::Dawntrail, stm, id),
            Err(Error::Stm(_))
        ));
    }
    assert_eq!(
        dye::apply_channel_row(&base, modern, Profile::Dawntrail, stm, 1, 4),
        Err(Error::InvalidChannel(4))
    );
    assert_eq!(
        dye::apply_channel_row(
            &base,
            row(ColorLayout::Legacy, 31),
            Profile::Dawntrail,
            stm,
            1,
            0
        ),
        Err(Error::ChannelNotEncoded)
    );
}

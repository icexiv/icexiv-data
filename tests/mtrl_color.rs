//! Synthetic full tables and an independent ordered-value numeric oracle.
use icexiv_data::{
    mtrl::{DyeOverrideProfile as Profile, Mtrl, dye},
    stm::Stm,
};

fn material(modern: bool, colors: &[u16], dyes: &[u32], shader: &str) -> Vec<u8> {
    let mut bytes = vec![0; 16];
    bytes[..4].copy_from_slice(&0x0103_0000u32.to_le_bytes());
    bytes.extend(shader.bytes().chain([0]));
    bytes[8..10].copy_from_slice(&((shader.len() + 1) as u16).to_le_bytes());
    bytes[15] = 4;
    let flags = if modern { 0x534u32 } else { 4 } | if dyes.is_empty() { 0 } else { 8 };
    bytes.extend(flags.to_le_bytes());
    bytes.extend(colors.iter().flat_map(|v| v.to_le_bytes()));
    for &word in dyes {
        if modern {
            bytes.extend(word.to_le_bytes());
        } else {
            bytes.extend((word as u16).to_le_bytes());
        }
    }
    let dataset = colors.len() * 2 + dyes.len() * if modern { 4 } else { 2 };
    bytes[6..8].copy_from_slice(&(dataset as u16).to_le_bytes());
    bytes.extend([0; 12]);
    let size = bytes.len() as u16;
    bytes[4..6].copy_from_slice(&size.to_le_bytes());
    bytes
}

fn template(modern: bool) -> Vec<u8> {
    let columns = if modern { 12 } else { 5 };
    let mut bytes = vec![0; 16 + columns * 2];
    bytes[..2].copy_from_slice(b"MS");
    bytes[2..4].copy_from_slice(&(if modern { 0x0201u16 } else { 0x0101 }).to_le_bytes());
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    if modern {
        bytes[6..8].copy_from_slice(&[3, 9]);
    }
    bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
    for column in 0..columns {
        let values: &[u16] = match column {
            0 => &[0x8000, 0x7d55, 0x4d00],
            1 => &[0, 0, 0],
            2 => &[0x3c00, 0x4000, 0x4200],
            _ => &[0x4400],
        };
        bytes.extend(values.iter().flat_map(|v| v.to_le_bytes()));
        let end = ((bytes.len() - 16 - columns * 2) / 2) as u16;
        bytes[16 + column * 2..18 + column * 2].copy_from_slice(&end.to_le_bytes());
    }
    bytes
}

#[test]
fn legacy_full_row_mapping_and_tail_dye_are_distinct() {
    let source = [
        0x8000, 0x7d55, 0x4400, 0x4d00, 0x3000, 0x3400, 0x7fff, 0x3800, 0x3c00, 0x4000, 0x4200,
        0x2400, 0xbc00, 0, 1, 0x7c00,
    ];
    let expected = [
        0x8000, 0x7d55, 0x4400, 0x3c00, 0x3c00, 0x3c00, 0x3c00, 0xccc0, 0x3c00, 0x4000, 0x4200,
        0x3c00, 0x3000, 0x3400, 0x3800, 0, 0x3800, 0, 0, 0, 0, 0, 0, 0, 0, 0x2400, 0x3c00, 0,
        0xbc00, 0, 1, 0x7c00,
    ];
    let mut colors = source.repeat(16);
    colors[16] = 0x4500;
    let bytes = material(false, &colors, &[63; 16], "characterlegacy.shpk");
    let mtrl = Mtrl::parse(&bytes).unwrap();
    let base = mtrl.canonical_inline_color_table().unwrap().unwrap();
    assert_eq!(base.half_bits()[0], expected);
    assert_eq!(base.half_bits()[1][0], 0x4500);
    for row in &base.half_bits()[16..] {
        assert_eq!(row, &expected);
    }
    assert!(base.dye_rows()[..16].iter().all(Option::is_some));
    assert!(base.dye_rows()[16..].iter().all(Option::is_none));
    let stm_bytes = template(false);
    let stm = Stm::parse(&stm_bytes).unwrap();
    let applied = dye::apply_table(&base, Profile::Legacy, Some(stm), [1, 2]).unwrap();
    assert_eq!(applied.half_bits[0][3], 0x4400);
    assert_eq!(applied.half_bits[0][7], 0x4400);
    assert_eq!(applied.half_bits[0][4..7], [0x3c00; 3]);
    assert_eq!(&applied.half_bits[16..], &[expected; 16]);
    assert_eq!(base.half_bits()[0], expected);
    let reset = dye::apply_table(&base, Profile::Legacy, None, [0, 2]).unwrap();
    assert_eq!(reset.half_bits, *base.half_bits());
    assert_eq!(reset.written_lane_masks, [0; 32]);
    assert_eq!(base.values()[0][7], -19.0);
    assert_eq!(base.to_le_bytes()[14..16], 0xccc0u16.to_le_bytes());
}

// Numerical definition independent of the implementation's fixed-point method.
fn finite_half(bits: u16) -> f64 {
    let exponent = (bits >> 10) & 31;
    let fraction = bits & 1023;
    let magnitude = if exponent == 0 {
        f64::from(fraction) * 2f64.powi(-24)
    } else {
        (1.0 + f64::from(fraction) / 1024.0) * 2f64.powi(i32::from(exponent) - 15)
    };
    if bits & 0x8000 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

#[test]
fn all_half_inputs_match_ordered_value_oracle() {
    let positive: Vec<f64> = (0..0x7c00).map(finite_half).collect();
    let mut colors = vec![0; 256];
    for first in (0..=u16::MAX).step_by(16) {
        for row in 0..16 {
            colors[row * 16 + 3] = first + row as u16;
        }
        let bytes = material(false, &colors, &[], "character.shpk");
        let base = Mtrl::parse(&bytes)
            .unwrap()
            .canonical_inline_color_table()
            .unwrap()
            .unwrap();
        for row in 0..16 {
            let bits = first + row as u16;
            let expected = if bits & 0x7c00 == 0x7c00 {
                if bits & 1023 == 0 {
                    bits ^ 0x8000
                } else {
                    (bits & 0x8000) | 0x7e00
                }
            } else {
                let rounded = f64::from((1.0 - finite_half(bits)) as f32);
                let magnitude = positive.partition_point(|&v| v <= rounded.abs()) - 1;
                magnitude as u16 | if rounded < 0.0 { 0x8000 } else { 0 }
            };
            assert_eq!(base.half_bits()[row][7], expected, "source {bits:04x}");
        }
        assert_eq!(base.half_bits()[16], base.half_bits()[0]);
    }
}

#[test]
fn two_slots_reset_and_missing_templates_produce_complete_tables() {
    let colors: Vec<_> = (0..1024).map(|i| 0x7000 + i).collect();
    let mut dyes = [0; 32];
    dyes[0] = (1 << 16) | 4095;
    dyes[1] = (1 << 27) | (1 << 16) | 4095;
    dyes[2] = (2 << 27) | (1 << 16) | 4095;
    dyes[3] = (3 << 27) | (1 << 16) | 4095;
    dyes[4] = (2 << 16) | 4095; // Confirmed absent ID.
    dyes[5] = 2 << 16; // No flags; no lookup needed.
    let bytes = material(true, &colors, &dyes, "character.shpk");
    let mtrl = Mtrl::parse(&bytes).unwrap();
    let profile = Profile::for_shader_package(mtrl.shader_package().unwrap());
    assert_eq!(profile, Profile::Dawntrail);
    let base = mtrl.canonical_inline_color_table().unwrap().unwrap();
    assert_eq!(
        base.to_le_bytes().to_vec(),
        colors
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>()
    );
    let stm_bytes = template(true);
    let stm = Stm::parse(&stm_bytes).unwrap();
    let both = dye::apply_table(&base, profile, Some(stm), [1, 2]).unwrap();
    assert_eq!(both.missing_template_rows, 1 << 4);
    assert_eq!(both.half_bits[0][..3], [0x8000, 0x7d55, 0x4d00]);
    assert_eq!(both.half_bits[1][..3], [0x8000, 0x7d55, 0x4d00]);
    assert_eq!(&both.half_bits[2..], &base.half_bits()[2..]);
    assert_eq!(&both.half_bits[0][4..7], &base.half_bits()[0][4..7]);
    assert_eq!(both.values()[0][2], 20.0);
    assert_eq!(both.to_le_bytes()[..2], 0x8000u16.to_le_bytes());
    let first_reset = dye::apply_table(&base, profile, Some(stm), [0, 2]).unwrap();
    assert_eq!(first_reset.half_bits[0], base.half_bits()[0]);
    assert_eq!(first_reset.half_bits[1], both.half_bits[1]);
    assert_eq!(first_reset.missing_template_rows, 0);
    let reset = dye::apply_table(&base, profile, None, [0, 0]).unwrap();
    assert_eq!(reset.half_bits, *base.half_bits());
    assert_eq!(
        dye::apply_table(&base, profile, None, [1, 0]),
        Err(dye::Error::MissingTemplates)
    );
    assert_eq!(
        dye::apply_table(&base, Profile::Legacy, Some(stm), [1, 0]),
        Err(dye::Error::TemplateLayoutMismatch)
    );
    for id in [254, 255] {
        assert_eq!(
            dye::apply_table(&base, profile, Some(stm), [id, 0]),
            Err(dye::Error::InvalidStainId(id))
        );
    }
    // A 32x32 dataset can use the five-column shader profile.
    assert_eq!(
        Profile::for_shader_package(b"characterlegacy.shpk"),
        Profile::Legacy
    );
}

#[test]
fn inline_declaration_and_dataset_are_both_required() {
    let mut bytes = material(false, &[0; 256], &[], "x");
    bytes[18] = 0;
    let mtrl = Mtrl::parse(&bytes).unwrap();
    assert!(mtrl.canonical_inline_color_table().unwrap().is_none());
    assert!(mtrl.color_table().unwrap().is_some());
    let empty = material(false, &[], &[], "x");
    assert!(
        Mtrl::parse(&empty)
            .unwrap()
            .canonical_inline_color_table()
            .is_err()
    );
    let mut truncated = material(false, &[0; 256], &[], "x");
    truncated.drain(19..22);
    truncated[15] = 1;
    let size = truncated.len() as u16;
    truncated[4..6].copy_from_slice(&size.to_le_bytes());
    assert!(
        Mtrl::parse(&truncated)
            .unwrap()
            .canonical_inline_color_table()
            .is_err()
    );
}

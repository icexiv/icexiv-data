use icexiv_data::imc::{Error, Imc};

#[test]
fn observed_weapon_records() {
    // Byte transcription of the independently extracted 16-byte resource;
    // identity and reproduction are recorded in docs/provenance/imc.md.
    let bytes = [1, 0, 1, 0, 1, 0, 255, 3, 0, 0, 1, 0, 255, 51, 0, 0];
    let file = Imc::parse(&bytes).unwrap();
    assert_eq!(file.variant_count(), 1);
    assert_eq!(file.part_mask(), 1);
    assert_eq!(file.parts().collect::<Vec<_>>(), [0]);
    let default = file.entry(0, 0).unwrap();
    let variant = file.entry(1, 0).unwrap();
    assert_eq!(default.material_id, 1);
    assert_eq!(default.attribute_mask(), 1023);
    assert_eq!(default.sound_id(), 0);
    assert_eq!(variant.raw_mask, 0x33ff);
    assert_eq!(variant.attribute_mask(), 1023);
    assert_eq!(variant.sound_id(), 12);
    assert_eq!(variant.vfx_id, 0);
    assert_eq!(variant.material_animation_id(), 0);
    assert_eq!(file.entry(2, 0), Err(Error::SubsetOutOfBounds));
    assert_eq!(file.entry(0, 1), Err(Error::PartNotPresent));
}

#[test]
fn observed_nonzero_decal_and_animation_fields_preserve_other_record_bits() {
    // Controlled one-part tables using numeric records observed in e0002 and
    // e0137, not copies of those full resources. See IMC provenance.
    let mut bytes = [0, 0, 2, 0, 50, 201, 255, 11, 0, 0];
    for animation in 0..=255u8 {
        bytes[9] = animation;
        let e = Imc::parse(&bytes).unwrap().entry(0, 1).unwrap();
        assert_eq!(e.material_id, 50);
        assert_eq!(e.decal_id, 201);
        assert_eq!(e.raw_mask, 3071);
        assert_eq!(e.attribute_mask(), 1023);
        assert_eq!(e.sound_id(), 2);
        assert_eq!(e.raw_animation, animation);
        assert_eq!(e.material_animation_id(), animation % 16);
    }
    bytes[4..].copy_from_slice(&[2, 0, 255, 27, 0, 10]);
    let e = Imc::parse(&bytes).unwrap().entry(0, 1).unwrap();
    assert_eq!(e.material_animation_id(), 10);
    assert_eq!(e.sound_id(), 6);
}
#[test]
fn sparse_part_bits_and_all_six_record_fields() {
    // Controlled test, not evidence for installed-game sparse masks.
    let bytes = [0, 0, 0x80, 0x80, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    let file = Imc::parse(&bytes).unwrap();
    assert_eq!(file.parts().collect::<Vec<_>>(), [7, 15]);
    let first = file.entry(0, 7).unwrap();
    assert_eq!((first.material_id, first.decal_id), (2, 3));
    assert_eq!(first.raw_mask, 0x0504);
    assert_eq!((first.vfx_id, first.raw_animation), (6, 7));
    assert_eq!(file.entry(0, 15).unwrap().material_id, 8);
    let mut animation = first;
    animation.raw_animation = 0xa7;
    assert_eq!(animation.material_animation_id(), 7);
    for bit in [0, 8, 16, 255] {
        assert_eq!(file.entry(0, bit), Err(Error::PartNotPresent));
    }
}

#[test]
fn truncation_trailing_bytes_and_extreme_counts_are_rejected() {
    let bytes = [1, 0, 1, 0, 1, 0, 255, 3, 0, 0, 1, 0, 255, 51, 0, 0];
    for end in 0..bytes.len() {
        assert!(Imc::parse(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(Imc::parse(&trailing).is_err());
    assert!(Imc::parse(&[0, 0, 0, 0]).is_err());
    assert!(Imc::parse(&[255, 255, 255, 255]).is_err());
}

#[test]
fn arbitrary_small_inputs_never_panic() {
    let mut state = 0x1729u32;
    for len in 0..2048 {
        let bytes: Vec<_> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        if let Ok(file) = Imc::parse(&bytes) {
            for part in 0..=255 {
                let _ = file.entry(file.variant_count(), part);
            }
        }
    }
}

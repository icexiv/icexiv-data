use icexiv_data::eqp::{ENTRY_COUNT, Eqp, Error, Flags};
#[test]
fn region_source_and_coupled_fields_preserve_codes_not_visibility() {
    for bit in 0..64 {
        let f = Flags(1u64 << bit);
        assert_eq!(f.body().uses_head_parameters(), bit == 10);
        assert_eq!(f.body().uses_hand_parameters(), bit == 9);
        assert_eq!(f.body().uses_leg_parameters(), bit == 8);
        assert_eq!(f.legs().uses_foot_parameters(), bit == 21);
        assert_eq!(f.body().glove_culling_code(), ((f.0 >> 3) & 3) as u8);
        assert_eq!(f.legs().boot_culling_code(), ((f.0 >> 18) & 3) as u8);
        assert_eq!(f.hands().coverage_selector(), ((f.0 >> 25) & 3) as u8);
        assert_eq!(f.feet().coverage_selector(), ((f.0 >> 33) & 3) as u8);
        assert_eq!(f.hands().coverage_comparison_bit(), bit == 27);
        assert_eq!(f.feet().coverage_comparison_bit(), bit == 35);
        assert_eq!(f.head().hair_coverage_code(), ((f.0 >> 41) & 7) as u8);
    }
    for code in 0..4u64 {
        let f = Flags(code << 3 | code << 18 | code << 25 | code << 33);
        assert_eq!(f.body().glove_culling_code(), code as u8);
        assert_eq!(f.legs().boot_culling_code(), code as u8);
        assert_eq!(f.hands().coverage_selector(), code as u8);
        assert_eq!(f.feet().coverage_selector(), code as u8);
    }
}
#[test]
fn hair_code_preserves_all_three_bits_without_boolean_precedence() {
    for byte in 0..=255u64 {
        for unrelated in [0, u64::MAX & !(255u64 << 40)] {
            let f = Flags(unrelated | byte << 40);
            let h = f.head();
            assert_eq!(h.hair_coverage_code(), ((byte / 2) % 8) as u8);
            assert_eq!(h.hide_scalp(), byte & 2 != 0);
            assert_eq!(h.hide_hair(), byte & 4 != 0);
            assert_eq!(h.show_hair_override(), byte & 8 != 0);
        }
    }
}
#[test]
fn ear_and_tail_declarations_do_not_apply_enabling_or_cross_slot_policy() {
    // Exercise each raw getter with every unrelated field set and cleared.
    // Enabling and the two tail declarations remain separate file fields.
    for bit in 0..64 {
        for inverted in [false, true] {
            let raw = if inverted {
                !(1u64 << bit)
            } else {
                1u64 << bit
            };
            let f = Flags(raw);
            for (at, actual) in [
                (0, f.body().enabled()),
                (13, f.body().show_tail()),
                (16, f.legs().enabled()),
                (22, f.legs().show_tail()),
                (40, f.head().enabled()),
                (50, f.head().show_human_ears()),
                (51, f.head().show_miqote_ears()),
                (52, f.head().show_aura_ears()),
                (53, f.head().show_viera_ears()),
                (56, f.head().show_hrothgar_hat()),
                (57, f.head().show_viera_hat()),
            ] {
                assert_eq!(actual, (bit == at) != inverted);
            }
        }
    }
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0; 3 * 160 * 8];
    b[..8].copy_from_slice(&((1u64 << 63) | 5).to_le_bytes());
    b[8..16].copy_from_slice(&0x123456789abcdef0u64.to_le_bytes());
    b[160 * 8..160 * 8 + 8].copy_from_slice(&7u64.to_le_bytes());
    b[3 * 160 * 8 - 8..].copy_from_slice(&u64::MAX.to_le_bytes());
    b
}
#[test]
fn rank_lookup_missing_blocks_and_reserved_bitmap() {
    let b = fixture();
    let e = Eqp::parse(&b).unwrap();
    assert_eq!(e.stored_block_count(), 3);
    assert_eq!(e.entry(1).unwrap().unwrap().0, 0x123456789abcdef0);
    assert!(e.entry(160).unwrap().is_none());
    assert_eq!(e.entry(320).unwrap(), Some(Flags(7)));
    assert_eq!(e.entry(ENTRY_COUNT - 1).unwrap(), Some(Flags(u64::MAX)));
    assert_eq!(e.entry(0).unwrap_err(), Error::ReservedEntry);
    assert_eq!(e.entry(usize::MAX).unwrap_err(), Error::IndexOutOfBounds);
}
#[test]
fn slot_slices_and_each_bit_are_independent() {
    for bit in 0..64u8 {
        let f = Flags(1u64 << bit);
        assert_eq!(f.bit(bit), Some(true));
        assert_eq!(f.bit(64), None);
        let rebuilt = u64::from(f.body().0)
            | u64::from(f.legs().0) << 16
            | u64::from(f.hands().0) << 24
            | u64::from(f.feet().0) << 32
            | u64::from(f.head().0) << 40;
        assert_eq!(rebuilt, f.0);
    }
    let f = Flags((1 << 13) | (1 << 22) | (1 << 29) | (1 << 30) | (1 << 53) | (1 << 57));
    assert!(f.body().show_tail());
    assert!(f.legs().show_tail());
    assert!(f.hands().show_left_ring());
    assert!(f.hands().show_right_ring());
    assert!(f.head().show_viera_ears());
    assert!(f.head().show_viera_hat());
    assert!(!f.body().enabled());
}
#[test]
fn sizes_and_bitmap_mutations_are_checked() {
    let b = fixture();
    for end in 0..b.len() {
        assert!(Eqp::parse(&b[..end]).is_err());
    }
    let mut v = b.clone();
    v.push(0);
    assert!(Eqp::parse(&v).is_err());
    let mut v = b.clone();
    v[0] &= !1;
    assert!(Eqp::parse(&v).is_err());
    for bit in 0..64 {
        let mut v = b.clone();
        v[bit / 8] ^= 1 << (bit % 8);
        let _ = Eqp::parse(&v);
    }
}

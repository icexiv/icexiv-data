use icexiv_data::eqdp::{AccessorySlot, Entry, Eqdp, EquipmentSlot, Error, Limits, Slot};
#[test]
fn domain_slots_select_distinct_pairs_without_inventing_routing() {
    let equipment = [
        EquipmentSlot::Head,
        EquipmentSlot::Body,
        EquipmentSlot::Hands,
        EquipmentSlot::Legs,
        EquipmentSlot::Feet,
    ];
    let accessories = [
        AccessorySlot::Ears,
        AccessorySlot::Neck,
        AccessorySlot::Wrists,
        AccessorySlot::RightRing,
        AccessorySlot::LeftRing,
    ];
    for position in 0..5 {
        for pair in 0..4u16 {
            let e = Entry {
                raw: (pair << (position * 2)) | 0xfc00,
                implicit_zero: false,
            };
            assert_eq!(e.unknown_bits(), 0xfc00);
            for slot in 0..5 {
                let expected = if slot == position { pair as u8 } else { 0 };
                let a = e.accessory_slot(accessories[slot]);
                let q = e.equipment_slot(equipment[slot]);
                assert_eq!(a.raw(), expected);
                assert_eq!(q.raw(), expected);
                assert_eq!(a.material(), expected & 1 != 0);
                assert_eq!(a.model(), expected & 2 != 0);
            }
        }
    }
    // Observed a0001's left model has a model-only declaration and names the
    // right-ring material. The view must not infer material presence from model.
    let left = Entry {
        raw: 2 << 8,
        implicit_zero: false,
    }
    .accessory_slot(AccessorySlot::LeftRing);
    assert!(left.model());
    assert!(!left.material());
}
fn fixture() -> Vec<u8> {
    // Three logical blocks of two entries; middle block is absent. Physical
    // blocks are intentionally reversed, so pointer units/order are exercised.
    [0x0101u16, 2, 3, 2, 0xffff, 0, 0xaaaa, 0x155, 0x3ff, 0]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect()
}
#[test]
fn sparse_random_access_preserves_flags_and_explicit_zero() {
    let b = fixture();
    let q = Eqdp::parse(&b).unwrap();
    assert_eq!(
        (q.block_size(), q.block_count(), q.entry_count()),
        (2, 3, 6)
    );
    assert_eq!(q.entry(0).unwrap().raw, 0x3ff);
    assert!(!q.entry(1).unwrap().implicit_zero);
    assert!(q.entry(2).unwrap().implicit_zero);
    assert_eq!(q.entry(3).unwrap().raw, 0);
    let e = q.entry(4).unwrap();
    assert_eq!(e.raw, 0xaaaa);
    assert_eq!(e.unknown_bits(), 0xa800);
    assert!(e.slot(Slot::First).model());
    assert!(!e.slot(Slot::First).material());
    let e = q.entry(5).unwrap();
    for slot in [
        Slot::First,
        Slot::Second,
        Slot::Third,
        Slot::Fourth,
        Slot::Fifth,
    ] {
        assert_eq!(e.slot(slot).raw(), 1);
        assert!(e.slot(slot).material());
        assert!(!e.slot(slot).model());
    }
    assert_eq!(q.block_entry_offset(1).unwrap(), None);
    assert!(q.block_entry_offset(3).is_err());
    assert!(matches!(
        q.entry(usize::MAX),
        Err(Error::EntryOutOfBounds { .. })
    ));
}
#[test]
fn budgets_padding_and_checked_blocks() {
    let b = fixture();
    for end in 0..b.len() {
        assert!(Eqdp::parse(&b[..end]).is_err());
    }
    let mut padded = b.clone();
    padded.extend_from_slice(&[7, 8, 9]);
    assert_eq!(Eqdp::parse(&padded).unwrap().trailing_bytes(), [7, 8, 9]);
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_blocks: 2,
            ..Limits::default()
        },
    ] {
        assert_eq!(
            Eqdp::parse_with_limits(&b, limits).unwrap_err(),
            Error::BudgetExceeded
        );
    }
    let mut zero = b.clone();
    zero[2..4].fill(0);
    assert!(Eqdp::parse(&zero).is_err());
    let mut state = 0x4871u32;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut v = b.clone();
        let at = state as usize % v.len();
        v[at] ^= (state >> 16) as u8;
        if let Ok(q) = Eqdp::parse(&v) {
            let _ = q.entry(state as usize);
        }
    }
}

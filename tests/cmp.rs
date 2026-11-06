use icexiv_data::cmp::{Cmp, Error, FILE_SIZE, Range, Sex, VectorRange};
#[test]
fn range_percentages_are_checked_math_not_customize_sentinels() {
    let r = Range { min: -2., max: 6. };
    assert_eq!(r.at_percent(0).unwrap(), -2.);
    assert_eq!(r.at_percent(25).unwrap(), 0.);
    assert_eq!(r.at_percent(50).unwrap(), 2.);
    assert_eq!(r.at_percent(100).unwrap(), 6.);
    for p in 101..=255 {
        assert_eq!(r.at_percent(p), Err(Error::InvalidPercentage(p)));
    }
    assert_eq!(
        Range {
            min: -f32::MAX,
            max: f32::MAX
        }
        .at_percent(50)
        .unwrap(),
        0.
    );
    for p in 0..=100 {
        assert!(
            Range {
                min: -f32::MAX,
                max: f32::MAX
            }
            .at_percent(p)
            .unwrap()
            .is_finite()
        );
    }
    assert_eq!(
        Range { min: -0., max: 1. }.at_percent(0).unwrap().to_bits(),
        (-0f32).to_bits()
    );
    assert_eq!(
        Range { min: -1., max: -0. }
            .at_percent(100)
            .unwrap()
            .to_bits(),
        (-0f32).to_bits()
    );
    for r in [
        Range { min: 2., max: 1. },
        Range {
            min: f32::NAN,
            max: 1.,
        },
        Range {
            min: 1.,
            max: f32::INFINITY,
        },
    ] {
        assert!(matches!(r.at_percent(0), Err(Error::InvalidData(_))));
    }
    let vector = VectorRange {
        min: [-2., 0., 2.],
        max: [6., 4., 2.],
    };
    assert_eq!(vector.at_percent(25).unwrap(), [0., 1., 2.]);
    assert_eq!(vector.at_percent(255), Err(Error::InvalidPercentage(255)));
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn fixture() -> Vec<u8> {
    let mut b = vec![0; FILE_SIZE];
    for (i, c) in b[..0x2c800].chunks_exact_mut(4).enumerate() {
        c.copy_from_slice(&[(i & 255) as u8, (i >> 8) as u8, 7, 128]);
    }
    for record in 0..80 {
        for lane in 0..14 {
            put32(
                &mut b,
                0x2c800 + record * 56 + lane * 4,
                (record as f32 + lane as f32 / 16.).to_bits(),
            );
        }
    }
    b
}
#[test]
fn full_grid_colors_and_ranges() {
    let b = fixture();
    let c = Cmp::parse(&b).unwrap();
    assert_eq!(
        (c.parameter_count(), c.palette_count(), c.color_count()),
        (80, 178, 45568)
    );
    for r in 0..8 {
        for body in 1..=5 {
            for clan in 0..2 {
                let p = c.parameters(r, body, clan).unwrap();
                let id = r * 10 + usize::from(body - 1) * 2 + clan;
                assert_eq!(p.raw[0], id as f32);
                assert_eq!(p.height(Sex::Female).min, id as f32 + 0.25);
                assert_eq!(p.tail(Sex::Male).max, id as f32 + 3. / 16.);
                assert_eq!(p.bust().max[2], id as f32 + 13. / 16.);
            }
        }
    }
    assert_eq!(c.palette_color(177, 255).unwrap(), c.color(45567).unwrap());
    assert_eq!(c.color(257).unwrap(), [1, 1, 7, 128]);
    assert!(c.parameters(8, 1, 0).is_err());
    assert!(c.parameters(0, 0, 0).is_err());
    assert!(c.parameters(0, 6, 0).is_err());
    assert!(c.parameters(0, 1, 2).is_err());
    assert!(c.color(usize::MAX).is_err());
    assert!(c.palette_color(0, 256).is_err());
}
#[test]
fn sizes_nan_and_reversed_ranges_are_explicit_errors() {
    let b = fixture();
    for n in [0, 1, 0x2c800, FILE_SIZE - 1] {
        assert!(matches!(
            Cmp::parse(&b[..n]),
            Err(Error::UnsupportedLayout { .. })
        ));
    }
    let mut v = b.clone();
    v.push(0);
    assert!(Cmp::parse(&v).is_err());
    for bits in [
        f32::NAN.to_bits(),
        f32::INFINITY.to_bits(),
        100f32.to_bits(),
    ] {
        let mut v = b.clone();
        put32(&mut v, 0x2c800, bits);
        assert!(Cmp::parse(&v).is_err());
    }
    let mut state = 0x1753u32;
    for _ in 0..1024 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut v = b.clone();
        let at = state as usize % v.len();
        v[at] ^= (state >> 16) as u8;
        let _ = Cmp::parse(&v);
    }
}

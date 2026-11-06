use icexiv_data::mdl::{Element, Error, Limits, Mdl, MeshRole};

fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn append(b: &mut Vec<u8>, size: usize) -> usize {
    let p = b.len();
    b.resize(p + size, 0);
    p
}

struct Fixture {
    bytes: Vec<u8>,
    runtime: usize,
    model_header: usize,
    meshes: usize,
    palettes: usize,
    shape_value: usize,
    vertex_buffer: usize,
    index_buffer: usize,
}

fn fixture() -> Fixture {
    let mut bytes = vec![0; 68 + 272];
    put32(&mut bytes, 0, 0x0100_0006);
    put32(&mut bytes, 4, 272);
    put16(&mut bytes, 12, 2);
    put16(&mut bytes, 14, 1);
    bytes[64] = 1;
    for m in 0..2 {
        let p = 68 + m * 136;
        bytes[p..p + 5].copy_from_slice(&[0, 0, 2, 0, 0]);
        bytes[p + 8..p + 13].copy_from_slice(&[0, 12, 5, 1, 0]);
        bytes[p + 16..p + 21].copy_from_slice(&[0, 16, 5, 2, 0]);
        bytes[p + 24] = 255;
    }
    let runtime = bytes.len();
    let mut strings = Vec::new();
    let mut name = |s: &[u8]| {
        let p = strings.len() as u32;
        strings.extend_from_slice(s);
        strings.push(0);
        p
    };
    let attr = name(b"atr_a");
    let material = name(b"/mt_test.mtrl");
    let bone = name(b"n_root");
    let shape = name(b"shp_a");
    let p = append(&mut bytes, 8);
    put16(&mut bytes, p, 4);
    put32(&mut bytes, p + 4, strings.len() as u32);
    bytes.extend_from_slice(&strings);
    let model_header = append(&mut bytes, 56);
    put32(&mut bytes, model_header, 3f32.to_bits());
    for (off, value) in [
        (4, 2),
        (6, 1),
        (8, 2),
        (10, 1),
        (12, 1),
        (14, 1),
        (16, 1),
        (18, 1),
        (20, 1),
        (24, 1),
        (44, 2),
    ] {
        put16(&mut bytes, model_header + off, value);
    }
    bytes[model_header + 22] = 1;
    bytes[model_header + 27] = 16;
    let e = append(&mut bytes, 32);
    put32(&mut bytes, e, 17);
    put32(&mut bytes, e + 4, bone);
    put32(&mut bytes, e + 8, 0.5f32.to_bits());
    let lods = append(&mut bytes, 180);
    put16(&mut bytes, lods + 2, 1);
    let extra = append(&mut bytes, 120);
    put16(&mut bytes, extra + 4, 1);
    put16(&mut bytes, extra + 6, 1);
    let meshes = append(&mut bytes, 72);
    for m in 0..2 {
        let p = meshes + m * 36;
        put16(&mut bytes, p, 4);
        put32(&mut bytes, p + 4, 6);
        put16(&mut bytes, p + 10, m as u16);
        put16(&mut bytes, p + 12, 1);
        put32(&mut bytes, p + 16, m as u32 * 6);
        put32(&mut bytes, p + 20, m as u32 * 80);
        bytes[p + 32] = 20;
        bytes[p + 35] = 1;
    }
    let a = append(&mut bytes, 4);
    put32(&mut bytes, a, attr);
    let submeshes = append(&mut bytes, 32);
    for m in 0..2 {
        let p = submeshes + m * 16;
        put32(&mut bytes, p, m as u32 * 6);
        put32(&mut bytes, p + 4, 6);
        put32(&mut bytes, p + 8, 1);
        put16(&mut bytes, p + 12, m as u16);
        put16(&mut bytes, p + 14, 1);
    }
    let p = append(&mut bytes, 4);
    put32(&mut bytes, p, material);
    let p = append(&mut bytes, 4);
    put32(&mut bytes, p, bone);
    let palettes = append(&mut bytes, 8);
    put16(&mut bytes, palettes, 1);
    put16(&mut bytes, palettes + 2, 1);
    let p = append(&mut bytes, 16);
    put32(&mut bytes, p, shape);
    put16(&mut bytes, p + 10, 1);
    let p = append(&mut bytes, 12);
    put32(&mut bytes, p + 4, 1);
    let shape_value = append(&mut bytes, 4);
    put16(&mut bytes, shape_value, 4);
    put16(&mut bytes, shape_value + 2, 3);
    let p = append(&mut bytes, 8);
    put32(&mut bytes, p, 4);
    bytes.extend_from_slice(&[3, 0, 0, 0]);
    let p = append(&mut bytes, 160);
    for i in 0..5 {
        for j in 0..4 {
            put32(&mut bytes, p + i * 32 + 16 + j * 4, 20f32.to_bits());
        }
    }
    let runtime_size = bytes.len() - runtime;
    put32(&mut bytes, 8, runtime_size as u32);
    let vertex_buffer = append(&mut bytes, 160);
    for m in 0..2 {
        for v in 0..4 {
            let p = vertex_buffer + m * 80 + v * 20;
            for c in 0..3 {
                put32(&mut bytes, p + c * 4, ((m * 10 + v + c) as f32).to_bits());
            }
            bytes[p + 12] = 255;
        }
    }
    let index_buffer = append(&mut bytes, 24);
    for (i, v) in [0, 1, 0, 2, 0, 1, 0, 1, 2, 0, 2, 3].into_iter().enumerate() {
        put16(&mut bytes, index_buffer + i * 2, v);
    }
    put32(&mut bytes, 16, vertex_buffer as u32);
    put32(&mut bytes, 28, index_buffer as u32);
    put32(&mut bytes, 40, 160);
    put32(&mut bytes, 52, 24);
    for (off, v) in [
        (44, 160),
        (48, 24),
        (52, vertex_buffer as u32),
        (56, index_buffer as u32),
    ] {
        put32(&mut bytes, lods + off, v);
    }
    Fixture {
        bytes,
        runtime,
        model_header,
        meshes,
        palettes,
        shape_value,
        vertex_buffer,
        index_buffer,
    }
}

#[test]
fn attribute_name_lookup_is_budgeted_local_and_checked_at_32_bits() {
    fn model(names: &[&[u8]]) -> Vec<u8> {
        let mut b = vec![0; 68];
        put32(&mut b, 0, 0x01000006);
        b[64] = 1;
        let mut pool = Vec::new();
        let mut offsets = Vec::new();
        for name in names {
            offsets.push(pool.len() as u32);
            pool.extend_from_slice(name);
            pool.push(0);
        }
        let p = append(&mut b, 8);
        put16(&mut b, p, names.len() as u16);
        put32(&mut b, p + 4, pool.len() as u32);
        b.extend(pool);
        let h = append(&mut b, 56);
        b[h + 22] = 1;
        put16(&mut b, h + 6, names.len() as u16);
        append(&mut b, 180);
        for off in offsets {
            b.extend_from_slice(&off.to_le_bytes());
        }
        append(&mut b, 8);
        append(&mut b, 128);
        let size = b.len() - 68;
        put32(&mut b, 8, size as u32);
        b
    }
    let a = model(&[b"atr_nek", b"atr_hij", b"atr_ude"]);
    let a = Mdl::parse(&a).unwrap();
    let b = model(&[b"atr_ude", b"atr_hij"]);
    let b = Mdl::parse(&b).unwrap();
    assert_eq!(a.attribute_bit(b"atr_ude", 256).unwrap(), Some(4));
    assert_eq!(b.attribute_bit(b"atr_ude", 256).unwrap(), Some(1));
    assert_eq!(a.attribute_bit(b"missing", 256).unwrap(), None);
    assert!(matches!(
        a.attribute_bit(b"atr_nek", 0),
        Err(Error::BudgetExceeded(_))
    ));
    let duplicate = model(&[b"atr_hij", b"atr_hij"]);
    let d = Mdl::parse(&duplicate).unwrap();
    assert!(d.attribute_bit(b"atr_hij", 256).is_err());
    let names = (0..33).map(|i| format!("atr_{i}")).collect::<Vec<_>>();
    let refs = names.iter().map(|v| v.as_bytes()).collect::<Vec<_>>();
    let data = model(&refs);
    let m = Mdl::parse(&data).unwrap();
    assert_eq!(m.attribute_bit(b"atr_31", 4096).unwrap(), Some(1 << 31));
    assert_eq!(m.attribute(32).unwrap(), b"atr_32");
    assert!(m.attribute_bit(b"atr_32", 4096).is_err());
}
#[test]
fn terrain_shadow_buffers_have_separate_ranges_and_half4_lanes() {
    let mut b = vec![0; 68];
    put32(&mut b, 0, 0x01000006);
    b[64] = 1;
    append(&mut b, 8);
    let h = append(&mut b, 56);
    b[h + 22] = 1;
    b[h + 26] = 1;
    put16(&mut b, h + 38, 1);
    let lods = append(&mut b, 180);
    put16(&mut b, lods + 22, 1);
    let terrain = append(&mut b, 20);
    put32(&mut b, terrain, 3);
    put16(&mut b, terrain + 12, 3);
    put16(&mut b, terrain + 16, 1);
    b[terrain + 18] = 8;
    let sub = append(&mut b, 12);
    put32(&mut b, sub + 4, 3);
    append(&mut b, 8);
    append(&mut b, 128);
    let runtime = b.len() - 68;
    put32(&mut b, 8, runtime as u32);
    let vo = b.len();
    for value in [0xbc00u16, 0, 0, 0, 0x3c00, 0, 0, 0, 0, 0x3c00, 0, 0] {
        b.extend_from_slice(&value.to_le_bytes());
    }
    let io = b.len();
    for index in [0u16, 1, 2] {
        b.extend_from_slice(&index.to_le_bytes());
    }
    for (p, v) in [
        (16, vo),
        (28, io),
        (40, 24),
        (52, 6),
        (lods + 44, 24),
        (lods + 48, 6),
        (lods + 52, vo),
        (lods + 56, io),
    ] {
        put32(&mut b, p, v as u32);
    }
    let m = Mdl::parse(&b).unwrap();
    let g = m.terrain_shadow_geometry(0, 0).unwrap();
    assert_eq!(g.position_half4(0).unwrap(), [-1., 0., 0., 0.]);
    assert_eq!(g.position_half4(2).unwrap(), [0., 1., 0., 0.]);
    assert_eq!(g.indices().collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(g.raw_vertex(0).unwrap().len(), 8);
    assert!(g.raw_vertex(3).is_err());
    assert!(g.raw_vertex(usize::MAX).is_err());
    assert!(m.terrain_shadow_geometry(1, 0).is_err());
    assert!(m.terrain_shadow_geometry(0, 1).is_err());
    let mut bad = b.clone();
    put16(&mut bad, io, 3);
    assert!(Mdl::parse(&bad).is_err());
    let mut other = b.clone();
    other[terrain + 18] = 4;
    let m = Mdl::parse(&other).unwrap();
    let g = m.terrain_shadow_geometry(0, 0).unwrap();
    assert_eq!(g.raw_vertex(0).unwrap().len(), 4);
    assert!(g.position_half4(0).is_err());
    let mut bad = b.clone();
    put32(&mut bad, terrain + 8, u32::MAX);
    assert!(Mdl::parse(&bad).is_err());
    let mut signed_zero = b.clone();
    put16(&mut signed_zero, vo + 6, 0x8000);
    assert_eq!(
        Mdl::parse(&signed_zero)
            .unwrap()
            .terrain_shadow_geometry(0, 0)
            .unwrap()
            .position_half4(0)
            .unwrap()[3]
            .to_bits(),
        0x80000000
    );
    assert!(matches!(
        Mdl::parse_with_limits(
            &b,
            Limits {
                max_indices: 2,
                ..Limits::default()
            }
        ),
        Err(Error::BudgetExceeded(_))
    ));
    for end in 0..b.len() {
        assert!(Mdl::parse(&b[..end]).is_err());
    }
    let mut state = 0x5facdfu32;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let mut bytes = b.clone();
        let at = state as usize % bytes.len();
        bytes[at] ^= (state >> 16) as u8;
        if let Ok(m) = Mdl::parse(&bytes) {
            if let Ok(g) = m.terrain_shadow_geometry(0, 0) {
                let _ = g.position_half4(state as usize % 4);
                let _ = g.raw_vertex(usize::MAX);
            }
        }
    }
}
#[test]
fn uv_declaration_indices_preserve_half4_half2_and_normalized_byte_layouts() {
    let mut f = fixture();
    for (i, element) in [[0, 0, 14, 4, 0], [0, 8, 14, 4, 1], [0, 16, 13, 4, 2]]
        .into_iter()
        .enumerate()
    {
        f.bytes[68 + i * 8..73 + i * 8].copy_from_slice(&element);
    }
    for (i, bits) in [
        0x3c00, 0x4000, 0x4200, 0x4400, 0x4500, 0x4600, 0x4700, 0x4800, 0x4880, 0x4900,
    ]
    .into_iter()
    .enumerate()
    {
        put16(&mut f.bytes, f.vertex_buffer + i * 2, bits);
    }
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert_eq!(
        g.attribute(0, 4, 0).unwrap().unwrap(),
        [1., 2., 3., 4., 0., 0., 0., 0.]
    );
    assert_eq!(
        g.attribute(0, 4, 1).unwrap().unwrap(),
        [5., 6., 7., 8., 0., 0., 0., 0.]
    );
    assert_eq!(
        g.attribute(0, 4, 2).unwrap().unwrap(),
        [9., 10., 0., 0., 0., 0., 0., 0.]
    );
    let mut f = fixture();
    f.bytes[68..73].copy_from_slice(&[0, 0, 13, 4, 0]);
    f.bytes[76..81].copy_from_slice(&[0, 16, 8, 4, 5]);
    f.bytes[84] = 255;
    put16(&mut f.bytes, f.vertex_buffer, 0x3c00);
    put16(&mut f.bytes, f.vertex_buffer + 2, 0xc000);
    f.bytes[f.vertex_buffer + 16..f.vertex_buffer + 20].copy_from_slice(&[255, 128, 0, 64]);
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert_eq!(
        g.attribute(0, 4, 0).unwrap().unwrap(),
        [1., -2., 0., 0., 0., 0., 0., 0.]
    );
    assert_eq!(
        g.attribute(0, 4, 5).unwrap().unwrap(),
        [1., 128. / 255., 0., 64. / 255., 0., 0., 0., 0.]
    );
    for index in [1, 2, 3, 4, 255] {
        assert!(g.attribute(0, 4, index).unwrap().is_none());
    }
}
#[test]
fn tangent_flow_and_binormal_inputs_keep_their_encoded_scalars() {
    let mut f = fixture();
    for (i, element) in [[0, 0, 8, 5, 0], [0, 4, 8, 6, 0], [0, 8, 8, 5, 1]]
        .into_iter()
        .enumerate()
    {
        f.bytes[68 + i * 8..73 + i * 8].copy_from_slice(&element);
    }
    f.bytes[f.vertex_buffer..f.vertex_buffer + 12]
        .copy_from_slice(&[0, 128, 255, 64, 255, 0, 64, 128, 32, 64, 96, 255]);
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert_eq!(
        g.attribute(0, 5, 0).unwrap().unwrap(),
        [0., 128. / 255., 1., 64. / 255., 0., 0., 0., 0.]
    );
    assert_eq!(
        g.attribute(0, 6, 0).unwrap().unwrap(),
        [1., 0., 64. / 255., 128. / 255., 0., 0., 0., 0.]
    );
    // Raw usage-index preservation does not claim a native TANGENT1 input.
    assert_eq!(
        g.attribute(0, 5, 1).unwrap().unwrap(),
        [32. / 255., 64. / 255., 96. / 255., 1., 0., 0., 0., 0.]
    );
    assert!(g.attribute(0, 5, 2).unwrap().is_none());
    assert!(g.attribute(0, 6, 1).unwrap().is_none());
    assert!(matches!(
        g.attribute(4, 5, 0),
        Err(Error::OutOfBounds("vertex"))
    ));
}
#[test]
fn normalized_color0_and_color1_remain_distinct_without_channel_fallback() {
    let mut f = fixture();
    f.bytes[68 + 8 + 2..68 + 8 + 5].copy_from_slice(&[8, 7, 0]);
    f.bytes[68 + 16 + 2..68 + 16 + 5].copy_from_slice(&[8, 7, 1]);
    f.bytes[f.vertex_buffer + 12..f.vertex_buffer + 20]
        .copy_from_slice(&[255, 128, 1, 0, 0, 64, 200, 255]);
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert_eq!(
        g.attribute(0, 7, 0).unwrap().unwrap()[..4],
        [1., 128. / 255., 1. / 255., 0.]
    );
    assert_eq!(
        g.attribute(0, 7, 1).unwrap().unwrap()[..4],
        [0., 64. / 255., 200. / 255., 1.]
    );
    f.bytes[68 + 8 + 3] = 6;
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert!(g.attribute(0, 7, 0).unwrap().is_none());
    assert_eq!(
        g.attribute(0, 7, 1).unwrap().unwrap()[..4],
        [0., 64. / 255., 200. / 255., 1.]
    );
}
#[test]
fn semantic_queries_reject_ambiguity_and_do_not_hide_incomplete_skin_pairs() {
    let mut f = fixture();
    // Retain two independently accessible color channels in the same stream.
    f.bytes[68 + 8 + 3] = 7;
    f.bytes[68 + 16 + 3] = 7;
    f.bytes[68 + 16 + 4] = 1;
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert_eq!(g.attribute(0, 7, 0).unwrap().unwrap()[0], 255.);
    assert_eq!(g.attribute(0, 7, 1).unwrap().unwrap()[0], 0.);
    assert!(g.attribute(0, 7, 2).unwrap().is_none());
    assert!(g.skin_influences(0).unwrap().is_none());
    assert!(matches!(
        g.attribute(4, 7, 2),
        Err(Error::OutOfBounds("vertex"))
    ));
    assert!(matches!(
        g.skin_influences(4),
        Err(Error::OutOfBounds("vertex"))
    ));
    for removed in [1usize, 2] {
        let mut f = fixture();
        f.bytes[68 + removed * 8 + 3] = 7;
        let model = Mdl::parse(&f.bytes).unwrap();
        assert!(matches!(
            model.geometry(0, 0).unwrap().skin_influences(0),
            Err(Error::InvalidData("incomplete skin influence pair"))
        ));
    }
    for usage in [0, 1, 2] {
        let mut f = fixture();
        let source = f.bytes[68 + usage * 8..68 + usage * 8 + 8].to_vec();
        f.bytes[68 + 24..68 + 32].copy_from_slice(&source);
        f.bytes[68 + 32] = 255;
        let model = Mdl::parse(&f.bytes).unwrap();
        let g = model.geometry(0, 0).unwrap();
        assert_eq!(g.declaration().unwrap().count(), 4);
        assert!(matches!(
            g.attribute(0, usage as u8, 0),
            Err(Error::InvalidData("ambiguous vertex semantic"))
        ));
        if usage != 0 {
            assert!(matches!(
                g.skin_influences(0),
                Err(Error::InvalidData("ambiguous vertex semantic"))
            ));
        } else {
            // Ambiguity in another semantic does not damage the skin query.
            assert!(g.skin_influences(0).unwrap().is_some());
        }
    }
}
#[test]
fn all_tables_geometry_and_non_normal_roles_are_usable() {
    let f = fixture();
    let model = Mdl::parse(&f.bytes).unwrap();
    assert_eq!(model.material(0).unwrap(), b"/mt_test.mtrl");
    assert_eq!(model.attribute(0).unwrap(), b"atr_a");
    assert_eq!(model.bone_name(0).unwrap(), b"n_root");
    assert_eq!(model.bone_table(0).unwrap().collect::<Vec<_>>(), [0]);
    assert_eq!(model.element_id(0).unwrap().translation, [0.5, 0., 0.]);
    let lod = model.lod(0).unwrap();
    assert_eq!(lod.ranges[6].role, MeshRole::Glass);
    assert_eq!(lod.ranges[6].start, 1);
    let normal = model.geometry(0, 0).unwrap();
    let glass = model.geometry(0, 1).unwrap();
    assert_eq!(
        normal.indices().unwrap().collect::<Vec<_>>(),
        [0, 1, 0, 2, 0, 1]
    );
    assert_eq!(
        glass.attribute(0, 0, 0).unwrap().unwrap()[..3],
        [10., 11., 12.]
    );
    assert_eq!(
        normal.attribute(2, 0, 0).unwrap().unwrap()[..3],
        [2., 3., 4.]
    );
    assert_eq!(
        normal.skin_influences(0).unwrap().unwrap().weights,
        [1., 0., 0., 0., 0., 0., 0., 0.]
    );
    assert_eq!(model.submesh(1).unwrap().index_offset, 6);
    assert_eq!(model.shape(0).unwrap().name, b"shp_a");
    let reference = normal.shape_meshes(0).unwrap().next().unwrap().unwrap();
    let replacement = model.shape_value(reference.value_offset as usize).unwrap();
    assert_eq!(replacement.base_index, 4); // Occurrence 4 exceeds last vertex 3.
    assert_eq!(normal.vertex_for_occurrence(4).unwrap(), 0);
    let position = normal.declaration().unwrap().next().unwrap();
    assert_eq!(
        normal.shape_delta(replacement, position).unwrap()[..3],
        [3., 3., 3.]
    );
    assert_eq!(normal.decode_attribute(position, 128).unwrap().len(), 4);
    assert!(matches!(
        normal.decode_attribute(position, 127),
        Err(Error::BudgetExceeded(_))
    ));
    assert!(normal.attribute(0, 7, 1).unwrap().is_none());
    assert!(model.geometry(1, 0).is_err());
    assert!(model.geometry(0, 2).is_err());
    assert!(normal.raw_vertex(4, 0).is_err());
    assert!(normal.raw_vertex(0, 255).is_err());
    assert!(model.bone_table(usize::MAX).is_err());
    assert_eq!(model.runtime_tail(), [3, 0, 0, 0]);
    assert_eq!(model.bounding_box(4).unwrap().max, [20.; 4]);
}

#[test]
fn malformed_references_and_budgets_are_errors() {
    for which in 0..14 {
        let mut f = fixture();
        match which {
            0 => put32(&mut f.bytes, 4, u32::MAX),
            1 => put32(&mut f.bytes, f.runtime + 4, u32::MAX),
            2 => f.bytes[68..68 + 136].fill(0),
            3 => f.bytes[68] = 3,
            4 => f.bytes[68 + 1] = 19,
            5 => put16(&mut f.bytes, f.meshes + 8, 1),
            6 => put32(&mut f.bytes, f.meshes + 20, u32::MAX),
            7 => put16(&mut f.bytes, f.index_buffer, 4),
            8 => put16(&mut f.bytes, f.palettes, u16::MAX),
            9 => put16(&mut f.bytes, f.palettes + 4, 1),
            10 => put16(&mut f.bytes, f.shape_value, 6),
            11 => put16(&mut f.bytes, f.shape_value + 2, 4),
            12 => put32(&mut f.bytes, 16, f.vertex_buffer as u32 + 1),
            _ => f.bytes[f.model_header + 22] = 3,
        }
        assert!(Mdl::parse(&f.bytes).is_err(), "mutation {which}");
    }
    let f = fixture();
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_records: 1,
            ..Limits::default()
        },
        Limits {
            max_indices: 1,
            ..Limits::default()
        },
        Limits {
            max_validation_steps: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Mdl::parse_with_limits(&f.bytes, limits),
            Err(Error::BudgetExceeded(_))
        ));
    }
}

#[test]
fn all_truncations_and_deterministic_mutations_never_panic() {
    let f = fixture();
    for end in 0..f.bytes.len() {
        assert!(Mdl::parse(&f.bytes[..end]).is_err(), "length {end}");
    }
    let mut state = 0x1349u32;
    for _ in 0..4096 {
        let mut bytes = f.bytes.clone();
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let at = state as usize % bytes.len();
        bytes[at] ^= (state >> 16) as u8;
        let _ = Mdl::parse(&bytes);
    }
}

#[test]
fn float_scalar_layouts_preserve_bits_and_bound_each_read() {
    let cases = [
        [0x8000_0000, 0x0000_0001, 0x007f_ffff, 0x0080_0000],
        [0x7f80_0000, 0xff80_0000, 0x7f7f_ffff, 0xff7f_ffff],
        [0x7fc0_1234, 0xffc0_5678, 0x7f80_1234, 0xff80_5678],
        [0x3f80_0000, 0xc020_0000, 0x0000_0000, 0x1234_5678],
    ];
    for (kind, width, lanes) in [(0, 4, 1), (1, 8, 2), (2, 12, 3), (3, 16, 4)] {
        let e = Element {
            stream: 0,
            offset: 0,
            kind,
            usage: 4,
            usage_index: 5,
        };
        assert_eq!(e.byte_width(), Ok(width));
        for words in cases {
            // Rotate each special value into every populated component.
            for rotation in 0..4 {
                let mut input = vec![0xa5];
                let mut expected = [0u32; 8];
                for lane in 0..lanes {
                    expected[lane] = words[(lane + rotation) % 4];
                    input.extend_from_slice(&expected[lane].to_le_bytes());
                }
                let exact = e.decode(&input[1..]).unwrap();
                assert_eq!(exact.map(f32::to_bits), expected);
                for end in 0..width {
                    assert!(matches!(
                        e.decode(&input[1..1 + end]),
                        Err(Error::InvalidData("short vertex element"))
                    ));
                }
                // Adjacent attribute bytes must not populate unused lanes.
                input.extend_from_slice(&[0xff; 16]);
                assert_eq!(e.decode(&input[1..]).unwrap().map(f32::to_bits), expected);
            }
        }
    }
}

#[test]
fn one_and_two_float_attributes_use_their_declared_stream_ranges() {
    let mut f = fixture();
    let declaration = 68;
    // Two independent UV indices at unaligned positions inside a 20-byte stride.
    f.bytes[declaration..declaration + 5].copy_from_slice(&[0, 1, 0, 4, 0]);
    f.bytes[declaration + 8..declaration + 13].copy_from_slice(&[0, 9, 1, 4, 1]);
    f.bytes[declaration + 16] = 255;
    let expected = [
        [0x8000_0000, 0x7fc0_1234, 0x0000_0001],
        [0x3f80_0000, 0xbf80_0000, 0x7f80_0000],
        [0xff80_0000, 0x007f_ffff, 0x0080_0000],
        [0x7f80_1234, 0xffc0_5678, 0xff7f_ffff],
    ];
    for (vertex, words) in expected.iter().enumerate() {
        let start = f.vertex_buffer + vertex * 20;
        for (offset, bits) in [1, 9, 13].into_iter().zip(words) {
            put32(&mut f.bytes, start + offset, *bits);
        }
    }
    let mdl = Mdl::parse(&f.bytes).unwrap();
    let geometry = mdl.geometry(0, 0).unwrap();
    let pair = geometry
        .declaration()
        .unwrap()
        .find(|e| e.usage_index == 1)
        .unwrap();
    let bulk = geometry.decode_attribute(pair, 4 * 32).unwrap();
    assert!(matches!(
        geometry.decode_attribute(pair, 4 * 32 - 1),
        Err(Error::BudgetExceeded("vertex output"))
    ));
    for (vertex, words) in expected.into_iter().enumerate() {
        assert_eq!(
            geometry
                .attribute(vertex, 4, 0)
                .unwrap()
                .unwrap()
                .map(f32::to_bits),
            [words[0], 0, 0, 0, 0, 0, 0, 0]
        );
        let two = [words[1], words[2], 0, 0, 0, 0, 0, 0];
        assert_eq!(
            geometry
                .attribute(vertex, 4, 1)
                .unwrap()
                .unwrap()
                .map(f32::to_bits),
            two
        );
        assert_eq!(bulk[vertex].map(f32::to_bits), two);
        assert!(geometry.attribute(vertex, 4, 2).unwrap().is_none());
    }
    assert!(matches!(
        geometry.attribute(4, 4, 0),
        Err(Error::OutOfBounds("vertex"))
    ));
    // Parsing must reject a known layout that spills into the next vertex.
    for (kind, first_invalid_offset) in [(0, 17), (1, 13)] {
        let mut bad = f.bytes.clone();
        bad[declaration + 1] = first_invalid_offset;
        bad[declaration + 2] = kind;
        assert!(matches!(
            Mdl::parse(&bad),
            Err(Error::InvalidData("vertex element exceeds stride"))
        ));
    }
}

#[test]
fn scalar_conversion_preserves_half_values_and_eight_influence_bytes() {
    let e = |kind| Element {
        stream: 0,
        offset: 0,
        kind,
        usage: 0,
        usage_index: 0,
    };
    assert_eq!(
        e(17).decode(&[0, 1, 2, 3, 4, 5, 6, 255]).unwrap(),
        [0., 1., 2., 3., 4., 5., 6., 255.]
    );
    let decoded = e(14).decode(&[0, 0x3c, 0, 0xbc, 0, 0x40, 0, 0x7c]).unwrap();
    assert_eq!(decoded[..3], [1., -1., 2.]);
    assert_eq!(decoded[3], f32::INFINITY);
    assert!(e(14).decode(&[0; 7]).is_err());
    assert_eq!(e(8).decode(&[0, 255, 128, 64]).unwrap()[1], 1.);
    assert_eq!(e(5).decode(&[0, 255, 128, 64]).unwrap()[1], 255.);
    assert!(matches!(
        e(255).decode(&[0; 32]),
        Err(Error::UnsupportedVertexType(255))
    ));
}

#[test]
fn stream_flag_bits_do_not_hide_eight_influence_payloads() {
    let mut f = fixture();
    let indices = f.bytes[f.index_buffer..].to_vec();
    f.bytes.truncate(f.vertex_buffer);
    let start = append(&mut f.bytes, 224);
    for m in 0..2 {
        let d = 68 + m * 136;
        f.bytes[d + 10] = 17;
        f.bytes[d + 17] = 20;
        f.bytes[d + 18] = 17;
        let mesh = f.meshes + m * 36;
        put32(&mut f.bytes, mesh + 20, m as u32 * 112);
        f.bytes[mesh + 32] = 28;
        f.bytes[mesh + 35] = 5;
        for v in 0..4 {
            let p = start + m * 112 + v * 28;
            f.bytes[p + 12] = 127;
            f.bytes[p + 19] = 128;
        }
    }
    let index_start = f.bytes.len();
    f.bytes.extend_from_slice(&indices);
    let lod = f.model_header + 56 + 32;
    put32(&mut f.bytes, 28, index_start as u32);
    put32(&mut f.bytes, 40, 224);
    put32(&mut f.bytes, lod + 44, 224);
    put32(&mut f.bytes, lod + 56, index_start as u32);
    let model = Mdl::parse(&f.bytes).unwrap();
    let g = model.geometry(0, 0).unwrap();
    assert_eq!(g.metadata().stream_count, 1);
    assert_eq!(g.metadata().raw_stream_flags, 5);
    let skin = g.skin_influences(0).unwrap().unwrap();
    assert_eq!(skin.count, 8);
    assert_eq!(skin.weights[7], 128. / 255.);
    assert_eq!(skin.indices, [0; 8]);
}

#[test]
fn unknown_extension_bytes_and_additional_boxes_are_retained() {
    let mut f = fixture();
    let tail = f.vertex_buffer - 164;
    f.bytes.splice(tail..tail, [0xa5; 16]);
    let mut extra_box = [0; 32];
    put32(&mut extra_box, 16, 42f32.to_bits());
    f.bytes
        .splice(f.vertex_buffer + 16..f.vertex_buffer + 16, extra_box);
    let lod = f.model_header + 56 + 32;
    let runtime = u32::from_le_bytes(f.bytes[8..12].try_into().unwrap());
    put32(&mut f.bytes, 8, runtime + 48);
    put16(&mut f.bytes, f.model_header + 36, 1);
    for offset in [16, 28, lod + 52, lod + 56] {
        let old = u32::from_le_bytes(f.bytes[offset..offset + 4].try_into().unwrap());
        put32(&mut f.bytes, offset, old + 48);
    }
    let model = Mdl::parse(&f.bytes).unwrap();
    assert_eq!(&model.runtime_tail()[..16], &[0xa5; 16]);
    assert_eq!(model.bounding_box_count(), 6);
    assert_eq!(model.bounding_box(5).unwrap().max[0], 42.);
}

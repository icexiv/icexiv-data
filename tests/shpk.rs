use icexiv_data::shpk::{Error, KeyGroup, Limits, ResourceKind, Shpk, Stage, name_id};
fn put16(b: &mut [u8], p: usize, v: u16) {
    b[p..p + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], p: usize, v: u32) {
    b[p..p + 4].copy_from_slice(&v.to_le_bytes());
}
fn append(b: &mut Vec<u8>, n: usize) -> usize {
    let p = b.len();
    b.resize(p + n, 0);
    p
}
fn descriptor(b: &mut Vec<u8>, name: &[u8], offset: u32, flags: u16) {
    let p = append(b, 16);
    put32(b, p, name_id(name));
    put32(b, p + 4, offset);
    put16(b, p + 8, name.len() as u16);
    put16(b, p + 10, flags);
    put16(b, p + 14, 1);
}
struct Fixture {
    b: Vec<u8>,
    node: usize,
    pass: usize,
    alias: usize,
    parameter: usize,
}
fn fixture(defaults: bool) -> Fixture {
    let names = [
        b"g_MaterialParameter".as_slice(),
        b"g_SamplerNormal",
        b"g_Texture",
        b"g_RW",
    ];
    let mut pool = Vec::new();
    let mut offsets = [0; 4];
    for (i, name) in names.iter().enumerate() {
        offsets[i] = pool.len() as u32;
        pool.extend_from_slice(name);
        pool.push(0);
    }
    let mut b = vec![0; 88];
    b[..4].copy_from_slice(b"ShPk");
    b[8..12].copy_from_slice(b"DX11");
    put32(&mut b, 4, 0x0e01);
    for offset in [24, 28, 72, 76, 80] {
        put32(&mut b, offset, 1);
    }
    put32(&mut b, 32, 16);
    put16(&mut b, 36, 1);
    put16(&mut b, 38, u16::from(defaults));
    for offset in [40, 44, 46, 48] {
        put16(&mut b, offset, 1);
    }
    for offset in [52, 56, 60, 64, 68] {
        put32(&mut b, offset, 1);
    }
    let mut code = Vec::new();
    for stage in 0..5 {
        let offset = code.len();
        if stage == 0 {
            code.extend_from_slice(&[0x11; 8]);
        }
        code.extend_from_slice(b"DXBC");
        code.extend_from_slice(&[0; 24]);
        let p = append(&mut b, 20);
        put32(&mut b, p, offset as u32);
        put32(&mut b, p + 4, (code.len() - offset) as u32);
        for off in [8, 10, 12, 14] {
            put16(&mut b, p + off, 1);
        }
        put32(&mut b, p + 16, stage);
        for i in [0, 1, 3, 2] {
            descriptor(&mut b, names[i], offsets[i], i as u16);
        }
    }
    let parameter = append(&mut b, 8);
    put32(&mut b, parameter, 77);
    put16(&mut b, parameter + 4, 4);
    put16(&mut b, parameter + 6, 8);
    if defaults {
        for v in [11f32, 22., 33., 44.] {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    for i in 0..4 {
        descriptor(&mut b, names[i], offsets[i], i as u16);
    }
    for (id, value) in [(12, 34), (56, 78), (90, 123)] {
        b.extend_from_slice(&u32::to_le_bytes(id));
        b.extend_from_slice(&u32::to_le_bytes(value));
    }
    b.extend_from_slice(&11u32.to_le_bytes());
    b.extend_from_slice(&22u32.to_le_bytes());
    let node = append(&mut b, 32);
    put32(&mut b, node, 50);
    put32(&mut b, node + 4, 1);
    b[node + 8..node + 24].fill(255);
    b[node + 8] = 0;
    put32(&mut b, node + 24, 0x11223344);
    put32(&mut b, node + 28, 0x55667788);
    for v in [34u32, 78, 123, 11, 22] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    let pass = append(&mut b, 24);
    put32(&mut b, pass, 1234);
    let alias = append(&mut b, 8);
    put32(&mut b, alias, 77);
    let code_start = b.len();
    put32(&mut b, 16, code_start as u32);
    b.extend_from_slice(&code);
    let names_start = b.len();
    put32(&mut b, 20, names_start as u32);
    b.extend_from_slice(&pool);
    let len = b.len();
    put32(&mut b, 12, len as u32);
    Fixture {
        b,
        node,
        pass,
        alias,
        parameter,
    }
}

#[test]
fn v0d01_has_an_84_byte_header_without_a_cluster_count_field() {
    for defaults in [false, true] {
        let mut b = fixture(defaults).b;
        // Controlled relayout; actual 0x0d01 samples are independently observed.
        drop(b.drain(84..88));
        put32(&mut b, 4, 0x0d01);
        for offset in [12, 16, 20] {
            let value = u32::from_le_bytes(b[offset..offset + 4].try_into().unwrap());
            put32(&mut b, offset, value - 4);
        }
        let s = Shpk::parse(&b).unwrap();
        assert_eq!(s.version(), 0x0d01);
        assert_eq!(s.declared_cluster_count(), None);
        assert_eq!(s.cluster_count(), 0);
        assert!(s.raw_alias_clusters().is_empty());
        assert_eq!(s.shader_count(Stage::Vertex), 1);
        assert_eq!(s.shader(Stage::Vertex, 0).unwrap().prefix(), [0x11; 8]);
        assert_eq!(s.material_parameter(0).unwrap().offset, 4);
        assert_eq!(s.node(0).unwrap().pass(0).unwrap().id, 1234);
        assert_eq!(s.default_floats(0).unwrap().is_some(), defaults);
        for end in 0..b.len() {
            assert!(Shpk::parse(&b[..end]).is_err());
        }
        assert!(matches!(
            Shpk::parse_with_limits(
                &b,
                Limits {
                    max_metadata_bytes: 0,
                    ..Limits::default()
                }
            ),
            Err(Error::BudgetExceeded(_))
        ));
        let mut state = 0x6faceu32;
        for _ in 0..4096 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let mut bad = b.clone();
            let at = state as usize % bad.len();
            bad[at] ^= (state >> 16) as u8;
            let _ = Shpk::parse(&bad);
        }
    }
    let mut incorrectly_relabelled = fixture(false).b;
    put32(&mut incorrectly_relabelled, 4, 0x0d01);
    assert!(Shpk::parse(&incorrectly_relabelled).is_err());
}
#[test]
fn shader_id_vectors_are_independently_computed_not_path_hashes() {
    // Python: ~zlib.crc32(name, 0xffffffff) & 0xffffffff.
    assert_eq!(name_id(b""), 0);
    assert_eq!(name_id(b"g_MaterialParameter"), 0x64d12851);
    assert_eq!(name_id(b"g_SamplerNormal"), 0x0c5ec1f1);
}

#[test]
fn every_stage_resource_group_default_and_node_is_usable() {
    let f = fixture(true);
    let s = Shpk::parse(&f.b).unwrap();
    assert_eq!(s.version(), 0x0e01);
    assert_eq!(s.declared_cluster_count(), Some(0));
    for stage in [
        Stage::Vertex,
        Stage::Pixel,
        Stage::Hull,
        Stage::Domain,
        Stage::Geometry,
    ] {
        let shader = s.shader(stage, 0).unwrap();
        assert_eq!(shader.stage(), stage);
        assert!(shader.bytecode().starts_with(b"DXBC"));
        assert_eq!(
            shader.prefix().len(),
            if stage == Stage::Vertex { 8 } else { 0 }
        );
        for kind in [
            ResourceKind::Constant,
            ResourceKind::Sampler,
            ResourceKind::Texture,
            ResourceKind::UnorderedAccess,
        ] {
            assert_eq!(shader.resource_count(kind), 1);
            assert_eq!(s.resource_count(kind), 1);
            assert_eq!(
                shader.resource(kind, 0).unwrap().id,
                s.resource(kind, 0).unwrap().id
            );
        }
    }
    assert_eq!(
        s.default_floats(0).unwrap().unwrap().collect::<Vec<_>>(),
        [22., 33.]
    );
    assert_eq!(s.material_buffer_size(), 16);
    assert_eq!(s.key(KeyGroup::Material, 0).unwrap().default_value, 123);
    assert_eq!(s.key(KeyGroup::SubView, 1).unwrap().default_value, 22);
    let node = s.node(0).unwrap();
    assert_eq!(node.selector(), 50);
    assert_eq!(node.unknown(), [0x11223344, 0x55667788]);
    assert_eq!(node.key_value(KeyGroup::System, 0).unwrap(), 34);
    assert_eq!(node.key_value(KeyGroup::Material, 0).unwrap(), 123);
    assert_eq!(node.pass(0).unwrap().id, 1234);
    assert_eq!(s.alias(0).unwrap().node_index, 0);
    assert!(s.raw_alias_clusters().is_empty());
    assert!(s.node(usize::MAX).is_err());
    assert!(s.shader(Stage::Vertex, usize::MAX).is_err());
    assert!(s.resource(ResourceKind::Sampler, usize::MAX).is_err());
}

#[test]
fn no_default_data_is_distinct_from_zero_default_values() {
    let f = fixture(false);
    let s = Shpk::parse(&f.b).unwrap();
    assert!(s.default_bytes(0).unwrap().is_none());
    assert!(s.default_floats(0).unwrap().is_none());
    assert!(s.default_bytes(1).is_err());
}

#[test]
fn malformed_counts_regions_names_and_references_are_errors() {
    for mutation in 0..15 {
        let mut f = fixture(true);
        match mutation {
            0 => put32(&mut f.b, 24, u32::MAX),
            1 => put32(&mut f.b, 16, 87),
            2 => put32(&mut f.b, 20, u32::MAX),
            3 => put32(&mut f.b, 88, u32::MAX),
            4 => put32(&mut f.b, 92, 7),
            5 => put16(&mut f.b, 88 + 20 + 8, u16::MAX),
            6 => put16(&mut f.b, 38, 2),
            7 => put32(&mut f.b, f.node + 4, u32::MAX),
            8 => f.b[f.node + 8] = 254,
            9 => put32(&mut f.b, f.pass + 4, 1),
            10 => put32(&mut f.b, f.alias + 4, 1),
            11 => put16(&mut f.b, f.parameter + 6, 17),
            12 => put32(&mut f.b, 84, 1),
            13 => f.b[8] = b'X',
            _ => put32(&mut f.b, 4, 0x0c01),
        }
        assert!(Shpk::parse(&f.b).is_err(), "mutation {mutation}");
    }
    let f = fixture(true);
    for limit in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_records: 1,
            ..Limits::default()
        },
        Limits {
            max_metadata_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_string_bytes: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Shpk::parse_with_limits(&f.b, limit),
            Err(Error::BudgetExceeded(_))
        ));
    }
}

#[test]
fn truncations_and_deterministic_mutations_never_panic() {
    let f = fixture(true);
    for n in 0..f.b.len() {
        assert!(Shpk::parse(&f.b[..n]).is_err());
    }
    let mut state = 0x1931u32;
    for _ in 0..4096 {
        let mut b = f.b.clone();
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let i = state as usize % b.len();
        b[i] ^= (state >> 16) as u8;
        if let Ok(s) = Shpk::parse(&b) {
            let _ = s.alias(usize::MAX);
            let _ = s.key(KeyGroup::Material, usize::MAX);
        }
    }
}

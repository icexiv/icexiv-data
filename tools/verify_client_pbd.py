"""Pinned offline PBD scalar/relationship evidence plus exhaustive file lookup.

Args: authorized-ffxiv_dx11.exe extracted-human.pbd new-output-directory.
No target execution, emulation or native algorithm port. Disassembly remains
outside the repository; Python wire observation is independent of the Rust API.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_pbd import observe


def main():
    exe, source, out = map(Path, sys.argv[1:])
    assert exe.stat().st_size <= 128 << 20 and source.stat().st_size <= 64 << 20
    assert not out.resolve().is_relative_to(exe.parent.resolve())
    assert not out.resolve().is_relative_to(source.resolve())
    out.mkdir(parents=True, exist_ok=False)
    fixtures = Path(__file__).resolve().parent.parent / 'tests/fixtures'
    pin = json.loads((fixtures / 'client-pbd-observations.json').read_text())
    b = exe.read_bytes()
    assert hashlib.sha256(b).hexdigest() == pin['binary_sha256']
    pe, = struct.unpack_from('<I', b, 60)
    assert b[:2] == b'MZ' and b[pe:pe+4] == b'PE\0\0'
    machine, count, _, _, _, size, _ = struct.unpack_from('<HHIIIHH', b, pe+4)
    opt = pe+24
    assert machine == 0x8664 and struct.unpack_from('<H', b, opt)[0] == 0x20b
    image, = struct.unpack_from('<Q', b, opt+24)
    assert image == int(pin['image_base'], 0)
    sections = [struct.unpack_from('<4I', b, opt+size+40*i+8) for i in range(count)]

    def raw(rva, length):
        for _, start, size, offset in sections:
            if start <= rva and rva+length <= start+size:
                assert offset+size <= len(b)
                return b[offset+rva-start:offset+rva-start+length]
        raise ValueError('unbacked RVA')

    for span in pin['ranges']:
        start = int(span['rva'], 0)
        assert span['bytes'] <= 65536
        assert hashlib.sha256(raw(start, span['bytes'])).hexdigest() == span['sha256']
        asm = subprocess.check_output(['objdump', '-d', '-Mintel',
            f'--start-address={image+start:#x}',
            f'--stop-address={image+start+span["bytes"]:#x}', str(exe)], text=True, timeout=30)
        (out / (span['label']+'.asm')).write_text(asm)
    for anchor in pin['anchors']:
        expected = bytes.fromhex(anchor['hex'])
        assert raw(int(anchor['rva'], 0), len(expected)) == expected
    for rva, target in [(0x442100, 0x17fa2d0), (0x442117, 0x17fa2d0),
                        (0x442880, 0x42c740), (0x17f9a04, 0x341430),
                        (0x268102, 0x17fa000), (0x268124, 0x17fa110),
                         (0x26828d, 0x341220), (0x17f9883, 0x17f94b0),
                         (0x4453f2, 0x2b97f0), (0x2b986b, 0x267af0),
                         (0x267b4e, 0x17f9fc0), (0x267c26, 0x17fa000),
                         (0x267c3f, 0x17fa110), (0x268139,0x25aa70),
                         (0x25aa94,0x25b970), (0x25aaac,0x20bbb0), (0x25aaba,0x20bc90),
                         (0x313b93,0x178c0c0), (0x178c1ed,0x1efab70),
                         (0x1efab83,0x1efb090), (0x178c223,0x1f41be0),
                         (0x178c230,0x33ae00), (0x33ae09,0x33a7d0)]:
        ins = raw(rva, 5)
        assert ins[0] in (0xe8, 0xe9)
        assert rva+5+struct.unpack_from('<i', ins, 1)[0] == target
    # Native reflection names establish field identity separately from offsets
    # coincidentally reused by unrelated runtime objects.
    for rva,name,offset in[(0x253e6f0,b'skeletons',0x10),
            (0x25403b8,b'parentIndices',0x18),(0x2540408,b'referencePose',0x38)]:
        target=struct.unpack('<Q',raw(rva,8))[0]-image
        assert raw(target,len(name)+1)==name+b'\0'
        assert struct.unpack('<H',raw(rva+30,2))[0]==offset
    assert raw(0x253e800,22)==b'hkaAnimationContainer\0'
    for rva, target in [(0x17fa303, 0x211bc38), (0x42d290, 0x216bd38),
                        (0x4420e6, 0x216bd38)]:
        ins = raw(rva, 8)
        assert ins[:3] == bytes.fromhex('f30f10') and ins[3] & 0xc7 == 5
        assert rva+8+struct.unpack_from('<i', ins, 4)[0] == target
    ins = raw(0x4267b2, 7)
    assert ins[:3] == bytes.fromhex('4c8d05')
    path = b'chara/xls/boneDeformer/human.pbd\0'
    assert raw(0x4267b2+7+struct.unpack_from('<i', ins, 3)[0], len(path)) == path

    data = source.read_bytes()
    resource_pin = json.loads((fixtures / 'pbd-observations.json').read_text())
    assert hashlib.sha256(data).hexdigest() == resource_pin['sha256']
    rows = []
    stats = dict.fromkeys(['pbd_tables', 'deformers', 'bones', 'float_coefficients',
        'nonidentity_matrices', 'translated_matrices', 'offdiagonal_matrices', 'extended_entries'], 0)
    observe(data, 'root', rows, stats)
    assert hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest() == resource_pin['rows_sha256']
    tables = []
    for row in rows:
        fields = row.split('\t')
        if fields[0] == 'pbd':
            tables.append((fields[1], {}))
        elif fields[0] == 'deformer':
            _, _, identity, connection, bits, payload, bones = fields
            table = tables[-1][1]
            assert int(identity) not in table
            table[int(identity)] = f'{bits}\t{payload}\t{bones}\t{connection}'
    expected = []
    for path, table in tables:
        expected.append(f'table\t{path}')
        expected.extend(f'lookup\t{i}\t{table.get(i, "absent")}' for i in range(65536))
    expected = '\n'.join(expected)+'\n'
    rust = subprocess.check_output(['target/debug/examples/validate_pbd_lookup', str(source)], text=True, timeout=120)
    assert expected == rust
    (out / 'lookup.python.tsv').write_text(expected)
    (out / 'lookup.rust.tsv').write_text(rust)
    result = {'binary_sha256': pin['binary_sha256'], 'resource_sha256': resource_pin['sha256'],
        'static_spans': len(pin['ranges']), 'static_anchors': len(pin['anchors']),
        'tables': len(tables), 'id_queries': 65536*len(tables),
        'present_queries': sum(len(t) for _, t in tables),
        'present_without_payload': sum(v.split('\t')[1] == '0' for _, t in tables for v in t.values()),
        'lookup_rows_sha256': hashlib.sha256(rust.encode()).hexdigest(),
        'objdump': subprocess.check_output(['objdump', '--version'], text=True).splitlines()[0],
        'limitations': 'Static instructions, not execution or REA Evidence. Ancestor-branch product order and one pose-product convention observed; no complete source/target-space, route or EPBD evaluation established.'}
    if 'lookup_rows_sha256' in pin:
        assert result['lookup_rows_sha256'] == pin['lookup_rows_sha256']
    text = json.dumps(result, indent=2)+'\n'
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

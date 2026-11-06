"""Independent EPBD root/selector observations and root-field controls.

Args: extracted-human.pbd new-output-directory. No external implementation or
native execution. All names, relations and matrix rows must match the original
wire observer for root-only mutations; controls are not game-produced files.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_pbd import observe


def envelope(b):
    assert b[-24:-20] == b'PACK'
    total, = struct.unpack_from('<Q', b, len(b)-8)
    start = len(b)-total
    assert 0 <= start and b[start:start+4] == b'EPBD'
    return start, b[start+16:-24]


def field(b, table, index):
    v = table-struct.unpack_from('<i', b, table)[0]
    length, size = struct.unpack_from('<HH', b, v)
    assert 4 <= length and v+length <= len(b) and 4 <= size and table+size <= len(b)
    if 4+index*2 >= length:
        return None
    off, = struct.unpack_from('<H', b, v+4+index*2)
    return table+off if off else None


def vector(b, table, index, stride):
    p = field(b, table, index)
    if p is None:
        return b''
    p += struct.unpack_from('<I', b, p)[0]
    n, = struct.unpack_from('<I', b, p)
    assert n <= 1_000_000 and p+4+n*stride <= len(b)
    return b[p+4:p+4+n*stride]


def scalar_rows(data):
    _, b = envelope(data)
    root, = struct.unpack_from('<I', b)
    values = [None if (p := field(b, root, i)) is None else struct.unpack_from('<H', b, p)[0] for i in range(2)]
    p = field(b, root, 2)
    assert p is not None
    p += struct.unpack_from('<I', b, p)[0]
    count, = struct.unpack_from('<I', b, p)
    assert count == 4
    option = lambda v: 'None' if v is None else f'Some({v})'
    rows = [f'root\t{option(values[0])}\t{option(values[1])}\t{count}']
    associations = []
    entries = []
    for i in range(count):
        q = p+4+i*4
        table = q+struct.unpack_from('<I', b, q)[0]
        child, parent = [struct.unpack_from('<I', b, field(b, table, k))[0] for k in range(2)]
        inner = vector(b, table, 3, 1)
        entries.append((child, parent, len(inner)))
        n, = struct.unpack_from('<I', inner)
        headers = list(struct.iter_unpack('<HHII', inner[4:4+12*n]))
        links = list(struct.iter_unpack('<4h', inner[4+12*n:4+20*n]))
        for (raw,) in struct.iter_unpack('<I', vector(b, table, 2, 4)):
            local = raw & 65535
            matches = [(j, h) for j, h in enumerate(headers) if h[0] == local]
            assert len(matches) == 1
            _, h = matches[0]
            bones = struct.unpack_from('<I', inner, h[2])[0] if h[2] else 0
            rows.append(f'selector\t{i}\t{child}\t{parent}\t{raw}\t{local}\tSome(({local}, {h[1]}, {bones}))')
            cursor = links[h[1]][0]
            chain = []
            while cursor >= 0:
                assert len(chain) <= n
                ancestor = headers[links[cursor][3]][0]
                chain.append(ancestor)
                cursor = links[cursor][0]
            assert parent in chain
            associations.append({'entry':i, 'child':child, 'parent':parent, 'raw_selector':raw,
                'local_skeleton_id':local, 'bones':bones, 'ancestor_ids':chain})
    pairs = []
    for child, parent in [(1501,901), (1601,201), (1701,101), (1801,201),
            (901,1501), (0,0), (2**32-1,2**32-1), (0x10000+1801,201)]:
        matches = [e for e in entries if e[:2] == (child, parent)]
        assert len(matches) <= 1
        value = f'Some(({matches[0][0]}, {matches[0][1]}, {matches[0][2]}))' if matches else 'None'
        pairs.append(f'pair\t{child}\t{parent}\t{value}')
    rows[1:1] = pairs
    return '\n'.join(rows)+'\n', values, associations


def main():
    source, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and not out.is_relative_to(source.parent)
    assert source.stat().st_size <= 64 << 20
    data = source.read_bytes()
    fixtures = repo/'tests/fixtures'
    pin = json.loads((fixtures/'pbd-observations.json').read_text())
    assert hashlib.sha256(data).hexdigest() == pin['sha256']
    start, payload = envelope(data)
    root, = struct.unpack_from('<I', payload)
    vt = root-struct.unpack_from('<i', payload, root)[0]
    assert [field(payload, root, k) for k in (0, 1)] == [None, None]
    assert struct.unpack_from('<HH', payload, vt) == (10, 8)
    assert field(payload, root, 2) == root+4
    controls = [('original', data)]
    for version, unknown in [(1, 0), (1, 65535), (2, 0xbeef), (0, 65535), (65535, 1)]:
        f = bytearray(payload)
        f[root+4:root+4] = struct.pack('<HH', version, unknown)
        struct.pack_into('<H', f, vt+2, 12)
        struct.pack_into('<3H', f, vt+4, 4, 6, 8)
        assert f[root+12:] == payload[root+8:]
        pack = (data[:start]+struct.pack('<4sHHQ', b'EPBD', 1, 0, len(f))+f+
            struct.pack('<4sHHqQ', b'PACK', 1, 1, -16-len(f), 40+len(f)))
        controls.append((f'root-{version}-{unknown}', pack))
    out.mkdir(parents=True, exist_ok=False)
    reports = []
    for label, b in controls:
        path = out/(label+'.pbd')
        path.write_bytes(b)
        expected, values, associations = scalar_rows(b)
        rust = subprocess.check_output(['target/debug/examples/validate_epbd_scalars', str(path)], text=True, timeout=120)
        assert rust == expected
        (out/(label+'.python.tsv')).write_text(expected)
        (out/(label+'.rust.tsv')).write_text(rust)
        rows = []
        stats = dict.fromkeys(['pbd_tables', 'deformers', 'bones', 'float_coefficients',
            'nonidentity_matrices', 'translated_matrices', 'offdiagonal_matrices', 'extended_entries'], 0)
        observe(b, 'root', rows, stats)
        text = '\n'.join(rows)+'\n'
        assert hashlib.sha256(text.encode()).hexdigest() == pin['rows_sha256']
        assert subprocess.check_output(['target/debug/examples/validate_pbd', str(path)], text=True, timeout=120) == text
        reports.append({'label':label, 'sha256':hashlib.sha256(b).hexdigest(),
            'root_scalars':values, 'scalar_rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),
            'unchanged_geometry_rows_sha256':pin['rows_sha256'], 'associations':associations})
    result = {'version':'2026.10.06.0000.0000', 'platform':'Win32', 'samples':reports,
        'limitations':'One actual EPBD with absent root scalars plus five root-only controls; local selector/ancestor observations, not native dispatch execution, complete route policy or matrix space.'}
    text = json.dumps(result, indent=2)+'\n'
    report_pin = json.loads((fixtures/'epbd-scalar-observations.json').read_text())
    assert hashlib.sha256(text.encode()).hexdigest() == report_pin['report_sha256'], hashlib.sha256(text.encode()).hexdigest()
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

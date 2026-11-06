"""Fixed Win64 vertex-input evidence; offline objdump and SHPK ISGN observations.

Args: authorized-exe new-output-directory extracted.shpk ...
Does not execute the target, external parsers or shaders. The signature reader
is an investigation tool for the pinned ISGN profile, not a general DXBC API.
"""
import collections
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def signatures(b):
    assert b[:4] == b'ShPk' and b[8:12] == b'DX11'
    version, = struct.unpack_from('<I', b, 4)
    assert version in (0x0d01, 0x0e01)
    assert struct.unpack_from('<I', b, 12)[0] == len(b)
    code_start, names_start, count = struct.unpack_from('<3I', b, 16)
    assert 0 < count <= 8192 and code_start <= names_start <= len(b)
    p = 88 if version == 0x0e01 else 84
    rows = []
    for shader in range(count):
        assert p+20 <= code_start
        offset, length, nc, ns, nr, nt, _ = struct.unpack_from('<II4HI', b, p)
        p += 20+16*(nc+ns+nr+nt)
        assert p <= code_start and length >= 40 and offset+length <= names_start-code_start
        c = b[code_start+offset+8:code_start+offset+length]
        assert c[:4] == b'DXBC' and struct.unpack_from('<I', c, 24)[0] == len(c)
        chunks, = struct.unpack_from('<I', c, 28)
        assert 0 < chunks <= 64 and 32+4*chunks <= len(c)
        found = 0
        for q in struct.unpack_from(f'<{chunks}I', c, 32):
            assert 32+4*chunks <= q and q+8 <= len(c)
            size, = struct.unpack_from('<I', c, q+4)
            assert q+8+size <= len(c)
            tag = c[q:q+4]
            if not tag.startswith(b'ISG'):
                continue
            assert tag == b'ISGN'
            found += 1
            s = c[q+8:q+8+size]
            n, first = struct.unpack_from('<II', s)
            assert n <= 256 and first == 8 and first+24*n <= len(s)
            for j in range(n):
                at = first+24*j
                name, index, system, component, register, mask, used, reserved = struct.unpack_from('<5I2BH', s, at)
                assert first+24*n <= name < len(s) and reserved == 0
                end = s.index(0, name)
                assert end-name <= 64 and component in (1, 2, 3)
                assert mask <= 15 and used & ~mask == 0
                rows.append([shader, s[name:end].decode('ascii'), index, system,
                    component, register, mask, used])
        assert found == 1
    return count, rows


def main():
    exe, out, *paths = map(lambda p: Path(p).resolve(), sys.argv[1:])
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and not out.is_relative_to(exe.parent)
    pin = json.loads((repo/'tests/fixtures/client-vertex-input-observations.json').read_text())
    assert exe.stat().st_size <= 128 << 20
    b = exe.read_bytes()
    assert hashlib.sha256(b).hexdigest() == pin['binary_sha256']
    out.mkdir(parents=True, exist_ok=False)

    # The fixed PE's .text RVA/file delta is 0xc00; .rdata is 0x1800.
    # Check section identities instead of silently using them on another binary.
    pe, = struct.unpack_from('<I', b, 0x3c)
    assert b[pe:pe+4] == b'PE\0\0'
    sections, = struct.unpack_from('<H', b, pe+6)
    optional, = struct.unpack_from('<H', b, pe+20)
    section_map = {}
    for i in range(sections):
        p = pe+24+optional+i*40
        name = b[p:p+8].rstrip(b'\0').decode('ascii')
        _, rva, size, file = struct.unpack_from('<4I', b, p+8)
        section_map[name] = (rva, size, file)
    assert section_map['.text'][0]-section_map['.text'][2] == 0xc00
    assert section_map['.rdata'][0]-section_map['.rdata'][2] == 0x1800

    for r in pin['ranges']:
        start = int(r['rva'], 16)
        data = b[start-0xc00:start-0xc00+r['bytes']]
        assert hashlib.sha256(data).hexdigest() == r['sha256']
        asm = subprocess.check_output(['objdump', '-d', '-Mintel',
            f'--start-address={0x140000000+start}',
            f'--stop-address={0x140000000+start+r["bytes"]}', str(exe)], text=True)
        (out/(r['label']+'.asm')).write_text(asm)
    targets = list(struct.unpack_from('<8I', b, 0x320a48-0xc00))
    assert targets == [int(v, 16) for v in pin['usage_branch_targets']]
    names = list(struct.unpack_from('<17I', b, 0x21c408-0xc00))
    decoded_names = []
    for target in names:
        p = target-0xc00
        assert b[p:p+3] == bytes.fromhex('488d05')
        string_rva = target+7+struct.unpack_from('<i', b, p+3)[0]
        q = string_rva-0x1800
        name = b[q:b.index(0, q)].decode('ascii')
        assert b[p+7:p+14] == bytes.fromhex('488902b00141c7')
        assert b[p+14] == 0
        index, = struct.unpack_from('<I', b, p+15)
        decoded_names.append([name, index])
    assert decoded_names == pin['internal_semantics']
    for anchor in pin['anchors']:
        p = int(anchor['rva'], 16)-0xc00
        raw = bytes.fromhex(anchor['hex'])
        assert b[p:p+len(raw)] == raw

    assert len(paths) == len(pin['shaders'])
    reports = []
    remaining = 512 << 20
    for path, sample in zip(paths, pin['shaders']):
        assert path.name == sample['file'] and path.stat().st_size <= 128 << 20
        remaining -= path.stat().st_size
        assert remaining >= 0
        data = path.read_bytes()
        assert hashlib.sha256(data).hexdigest() == sample['sha256']
        count, rows = signatures(data)
        text = json.dumps(rows, separators=(',', ':'))+'\n'
        (out/(path.name+'.signatures.json')).write_text(text)
        tangent = [r for r in rows if r[1] == 'TANGENT']
        assert all(r[2] == 0 and r[6] == 15 for r in tangent)
        counts = collections.Counter(tuple(r[1:]) for r in rows)
        reports.append({'file':path.name, 'sha256':sample['sha256'],
            'vertex_shaders':count, 'signature_records':len(rows),
            'signatures_sha256':hashlib.sha256(text.encode()).hexdigest(),
            'tangent_declarations':len(tangent), 'tangent_nonzero_read_mask':sum(r[7] != 0 for r in tangent),
            'unique_records':[{'record':list(k), 'count':v} for k,v in sorted(counts.items())]})
    result = {'version':pin['version'], 'platform':'Win32',
        'binary_sha256':pin['binary_sha256'], 'ranges':pin['ranges'],
        'usage_branch_targets':targets, 'internal_semantics':decoded_names, 'shaders':reports,
        'limitations':'Offline native bytes and pinned ISGN records, not execution, shader flow formula, model/permutation binding or higher usage-index equivalence.'}
    text = json.dumps(result, indent=2)+'\n'
    assert hashlib.sha256(text.encode()).hexdigest() == pin['report_sha256']
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

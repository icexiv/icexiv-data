"""Pinned PBD scalar/name/parameter packet observations; no target execution.

Args: authorized-ffxiv_dx11.exe new-output-directory outside repo/installation.
Addresses are preferred VAs and ranges are end-exclusive. Interpretation and
provenance: docs/provenance/pbd-scalar-consumer.md. Disassembly stays external.
"""
from pathlib import Path
import hashlib
import json
import struct
import subprocess
import sys


EXPECTED = '958b78e278ad896493c484eb313ff605ab41edefef74f92b0ee078e69b281bf9'
RANGES = [
    ('utility-buffer-initialization', 0x140425f90, 0x1404262b0),
    ('utility-model-name', 0x14042629a, 0x1404262b0),
    ('name-registry', 0x140236a30, 0x140236d26),
    ('buffer-factory', 0x140226100, 0x1402261e0),
    ('buffer-flags-initialization', 0x14021eec0, 0x14021f0f5),
    ('buffer-staging-allocation', 0x14021f3a0, 0x14021f495),
    ('scalar-parameter-write', 0x14042c740, 0x14042c7fe),
    ('scalar-ratio-source', 0x140441fa0, 0x140442139),
    ('scalar-slot-consumer', 0x1404427d0, 0x140442885),
    ('shpk-header-to-vs-construction', 0x1417ef690, 0x1417f02ca),
    ('shader-resource-name-registration', 0x1417eee90, 0x1417ef0cf),
    ('shader-descriptor-accessor', 0x14021bb60, 0x14021bb65),
    ('tls-packet-caller', 0x140244a20, 0x140244c8f),
    ('packet-builder', 0x14023c200, 0x14023c755),
    ('packet-resource-reader', 0x14023c760, 0x14023c7d0),
]
EDGES = [
    (0x14042603f, 0xe8, 0x140226100),
    (0x140226197, 0xe8, 0x14021eec0),
    (0x1404262a5, 0xe8, 0x140236a30),
    (0x14042c7c6, 0xe8, 0x14021f3a0),
    (0x14021f424, 0xe8, 0x14023fca0),
    (0x1417f027c, 0xe8, 0x1417eee90),
    (0x1417ef057, 0xe8, 0x14021bb60),
    (0x1417ef097, 0xe8, 0x140236a30),
    (0x140244c67, 0xe8, 0x14023c200),
    (0x14023c65d, 0xe8, 0x14023c760),
    (0x14023c679, 0xe8, 0x14023c760),
    (0x140442100, 0xe8, 0x1417fa2d0),
    (0x140442117, 0xe8, 0x1417fa2d0),
    (0x140442880, 0xe9, 0x14042c740),
]
REFERENCES = [
    (0x140425fb4, '488b2d', 0x1428f6780),
    (0x14042629a, '488d15', 0x142166cb8),
    (0x1417eeea1, '4c8b3d', 0x1428f6780),
    (0x14042c75b, '8b15', 0x142c208c0),
    (0x140244a33, '448b15', 0x142c208c0),
    (0x14021f3f9, '8b0d', 0x142c208c0),
    (0x1417ef6d7, '8b05', 0x1422daaf0),
    (0x1417ef6e9, '8b0d', 0x1422daaf4),
    (0x1417ef6ef, '8b05', 0x1422da9ac),
]
ANCHORS = [
    (0x1404262aa, '898330050000', 'utility name index +0x530'),
    (0x140426027, '488db310040000', 'utility parameter objects +0x410'),
    (0x14042c7d0, 'f30f1130', 'first f32 scalar write'),
    (0x14042c7e1, '48899cc640090000', 'context parameter pointer at name index'),
    (0x14021ef49, '89462044894624', 'buffer length +0x20 and flags +0x24'),
    (0x14021f448, '894334', 'staging byte length +0x34'),
    (0x14021f474, '48895328488d0432', 'staging pointer +0x28 and returned offset'),
    (0x14021bb60, '488b4128c3', 'shader descriptor table accessor'),
    (0x1417ef0a1, '428944f104', 'name index into descriptor +4'),
    (0x1417ef0a6, '0fb6450c428804f1', 'source resource slot to descriptor +0'),
    (0x1417ef0ae, '0fb7450e66428944f102', 'source resource size to descriptor +2'),
    (0x14023c7ab, '488b94cb40090000', 'context indexed parameter read'),
    (0x14023c7b6, '488b4a28488948e8', 'packet receives buffer data pointer'),
    (0x14023c7be, '8b4a34c1e9048948f0', 'packet receives 16-byte unit count'),
    (0x14044211f, 'f30f5ef7', 'character/model scalar division'),
    (0x140442128, 'f30f11b4bb680c0000', 'per-slot ratio store'),
    (0x140442855, 'f3420f108cb6680c0000', 'per-slot ratio load'),
    (0x140244a61, 'bd38020000', 'packet caller TLS field offset 0x238'),
    (0x140244a6b, '488b342b', 'packet caller TLS context read'),
    (0x14042c784, 'be38020000488b341e', 'scalar writer same TLS context field'),
]


def sha(b):
    return hashlib.sha256(b).hexdigest()


def main():
    exe, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and not out.is_relative_to(exe.parent)
    assert exe.stat().st_size == 51910400
    b = exe.read_bytes()
    assert sha(b) == EXPECTED and b[:2] == b'MZ'
    pe, = struct.unpack_from('<I', b, 0x3c)
    assert b[pe:pe + 4] == b'PE\0\0'
    machine, count = struct.unpack_from('<HH', b, pe + 4)
    opt_size, = struct.unpack_from('<H', b, pe + 20)
    opt = pe + 24
    assert machine == 0x8664 and struct.unpack_from('<H', b, opt)[0] == 0x20b
    image_base, = struct.unpack_from('<Q', b, opt + 24)
    assert image_base == 0x140000000 and count < 100
    sections = []
    for i in range(count):
        p = opt + opt_size + i * 40
        vs, rva, rs, raw = struct.unpack_from('<4I', b, p + 8)
        assert raw + rs <= len(b)
        sections.append({'name': b[p:p + 8].rstrip(b'\0').decode(), 'rva': rva,
                         'virtual_bytes': vs, 'raw_bytes': rs, 'raw_offset': raw})

    def offset(va, n):
        matches = [s for s in sections if s['rva'] <= va - image_base
                   and va - image_base + n <= s['rva'] + s['raw_bytes']]
        s, = matches
        return s['raw_offset'] + va - image_base - s['rva']

    def read(va, n):
        p = offset(va, n)
        return b[p:p + n]

    report = {'bytes': len(b), 'sha256': sha(b), 'image_base': hex(image_base),
              'section_mapping': sections, 'coordinate': 'preferred VA, end-exclusive ranges',
              'ranges': [], 'edges': [], 'rip_references': [], 'anchors': [], 'data': []}
    execution = {'target': str(exe), 'python': sys.version, 'tool_sha256': sha(Path(__file__).read_bytes()),
                 'objdump': subprocess.check_output(['objdump', '--version'], text=True, timeout=30).splitlines()[0],
                 'commands': []}
    out.mkdir(parents=True, exist_ok=False)
    for label, start, end in RANGES:
        raw = read(start, end - start)
        cmd = ['objdump', '-d', '-M', 'intel', '--start-address=' + hex(start),
               '--stop-address=' + hex(end), str(exe)]
        result = subprocess.run(cmd, check=True, capture_output=True, timeout=30)
        assert not result.stderr
        name = label + '.asm'
        (out / name).write_bytes(result.stdout)
        report['ranges'].append({'label': label, 'va_start': hex(start), 'va_end': hex(end),
                                 'file_offset': hex(offset(start, len(raw))), 'bytes': len(raw),
                                 'raw_sha256': sha(raw), 'objdump_file': name})
        execution['commands'].append({'command': cmd, 'output_sha256': sha(result.stdout)})
    for va, opcode, expected in EDGES:
        raw = read(va, 5)
        assert raw[0] == opcode
        target = va + 5 + struct.unpack_from('<i', raw, 1)[0]
        assert target == expected
        report['edges'].append({'va': hex(va), 'target': hex(target), 'bytes': raw.hex(),
                                'kind': 'call' if opcode == 0xe8 else 'jmp'})
    for va, prefix, expected in REFERENCES:
        prefix = bytes.fromhex(prefix)
        raw = read(va, len(prefix) + 4)
        assert raw.startswith(prefix)
        target = va + len(raw) + struct.unpack_from('<i', raw, len(prefix))[0]
        assert target == expected
        report['rip_references'].append({'va': hex(va), 'target': hex(target), 'bytes': raw.hex()})
    for va, expected, meaning in ANCHORS:
        raw = read(va, len(bytes.fromhex(expected)))
        assert raw.hex() == expected
        report['anchors'].append({'va': hex(va), 'file_offset': hex(offset(va, len(raw))),
                                  'bytes': raw.hex(), 'description': meaning})
    for va, expected in [(0x142166cb8, b'g_ModelParameter\0'), (0x1422daaf0, b'ShPk'),
                         (0x1422daaf4, struct.pack('<I', 0xe01)), (0x1422da9ac, struct.pack('<I', 0xd01))]:
        raw = read(va, len(expected))
        assert raw == expected
        report['data'].append({'va': hex(va), 'file_offset': hex(offset(va, len(raw))), 'bytes': raw.hex()})
    data = json.dumps(report, indent=2) + '\n'
    (out / 'report.json').write_text(data)
    execution['report_sha256'] = sha(data.encode())
    (out / 'execution.json').write_text(json.dumps(execution, indent=2) + '\n')
    pin = json.loads((repo / 'tests/fixtures/pbd-scalar-consumer-observations.json').read_text())
    assert sha(data.encode()) == pin['native_report_sha256']
    print('native observations:', len(RANGES), 'ranges,', len(EDGES), 'direct edges,',
          len(REFERENCES), 'RIP references,', len(ANCHORS), 'anchors; report', sha(data.encode()))


if __name__ == '__main__':
    main()

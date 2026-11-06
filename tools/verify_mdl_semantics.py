"""Independent semantic queries on actual MDL bytes and declaration controls.

Args: extracted-weapon.mdl new-output-directory. Controls change only usage and
usage-index bytes; all vertex/index payloads remain actual unmodified data.
Duplicates are query ambiguities, not a claimed MDL format prohibition.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def expected(b, lod=0, mesh=0, include_colors=False):
    stack, = struct.unpack_from('<I', b, 4)
    runtime = 68 + stack
    pool, = struct.unpack_from('<I', b, runtime + 4)
    header = runtime + 8 + pool
    elements, = struct.unpack_from('<H', b, header + 24)
    meshes = header + 56 + elements * 32 + 180 + (120 if b[header + 27] & 16 else 0)
    at = meshes + mesh * 36
    count, = struct.unpack_from('<H', b, at)
    vo, = struct.unpack_from('<I', b, 16 + lod * 4)
    offsets = struct.unpack_from('<3I', b, at + 20)
    strides = b[at + 32:at + 35]
    declarations = []
    for i in range(17):
        e = tuple(b[68 + mesh * 136 + i * 8:73 + mesh * 136 + i * 8])
        if e[0] == 255:
            break
        declarations.append(e)
    assert len(declarations) < 17 and 0 < count <= 65535
    groups = {}
    for e in declarations:
        groups.setdefault(e[3:], []).append(e)
    queries = sorted(set(groups) | {(255, 255)} | ({(7,0),(7,1)} if include_colors else set()))
    def scalar(vertex, e):
        stream, offset, kind, _, _ = e
        at = vo + offsets[stream] + vertex * strides[stream] + offset
        if kind in (0, 1, 2, 3):
            values = list(struct.unpack_from(f'<{kind+1}f', b, at))
        elif kind in (13, 14):
            values = list(struct.unpack_from(f'<{2 if kind == 13 else 4}e', b, at))
        else:
            assert kind in (5, 8, 17)
            values = list(b[at:at + (8 if kind == 17 else 4)])
            if kind == 8:
                values = [v / 255 for v in values]
        return values + [0] * (8-len(values))
    def bits(values):
        return ','.join(f'{struct.unpack("<I", struct.pack("<f", v))[0]:08x}' for v in values)
    def attribute_bits(vertex, e):
        stream, offset, kind, _, _ = e
        if kind in (0, 1, 2, 3):
            # Avoid Python float conversions quieting signaling NaNs.
            at = vo + offsets[stream] + vertex * strides[stream] + offset
            words = list(struct.unpack_from(f'<{kind+1}I', b, at))
            return ','.join(f'{v:08x}' for v in words + [0] * (8-len(words)))
        return bits(scalar(vertex, e))
    rows = []
    for v in range(count + 1):
        for usage, index in queries:
            matches = groups.get((usage, index), [])
            value = ('vertex' if v == count else 'ambiguous' if len(matches) > 1
                else attribute_bits(v, matches[0]) if matches else 'absent')
            rows.append(f'attribute\t{v}\t{usage}\t{index}\t{value}')
        weights, indices = groups.get((1, 0), []), groups.get((2, 0), [])
        if v == count:
            result = 'vertex'
        elif len(weights) > 1 or len(indices) > 1:
            result = 'ambiguous'
        elif not weights and not indices:
            result = 'absent'
        elif not weights or not indices:
            result = 'incomplete'
        else:
            w, i = weights[0], indices[0]
            assert w[2] in (5, 8, 17) and i[2] in (5, 17)
            n = 8 if w[2] == 17 else 4
            assert n == (8 if i[2] == 17 else 4)
            values = scalar(v, w)
            if w[2] != 8:
                values = [x / 255 for x in values]
            result = f'{n}\t{bits(values)}\t' + ','.join(str(int(x)) for x in scalar(v, i))
        rows.append(f'skin\t{v}\t{result}')
    return '\n'.join(rows) + '\n', declarations, count


def main():
    source, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and not out.is_relative_to(source.parent)
    assert source.stat().st_size <= 64 << 20
    raw = source.read_bytes()
    source_hash = hashlib.sha256(raw).hexdigest()
    assert source_hash == '66be81af7fbbd368b14052cde9dedcd047efdf12f87e5f522672a10ac9b4b425'
    out.mkdir(parents=True, exist_ok=False)
    _, declarations, count = expected(raw)
    positions = {(e[3], e[4]): i for i, e in enumerate(declarations)}
    assert len(positions) == len(declarations) and (240, 0) not in positions and (241, 0) not in positions
    a, b = positions[(3, 0)], positions[(6, 0)]
    w, i = positions[(1, 0)], positions[(2, 0)]
    controls = [
        ('original', []),
        ('two-usage-indices', [(a, 240, 0), (b, 240, 1)]),
        ('duplicate-semantic', [(a, 240, 0), (b, 240, 0)]),
        ('duplicate-weight', [(b, 1, 0)]),
        ('duplicate-index', [(b, 2, 0)]),
        ('missing-weight', [(w, 240, 0)]),
        ('missing-index', [(i, 240, 0)]),
        ('no-skin-pair', [(w, 240, 0), (i, 241, 0)]),
    ]
    reports = []
    for label, changes in controls:
        data = bytearray(raw)
        for element, usage, index in changes:
            data[68 + element * 8 + 3:68 + element * 8 + 5] = bytes([usage, index])
        assert data[68 + 136:] == raw[68 + 136:]
        path = out / (label + '.mdl')
        path.write_bytes(data)
        oracle, _, _ = expected(data)
        actual = subprocess.check_output(['target/debug/examples/validate_mdl_semantics', str(path)], text=True, timeout=120)
        assert actual == oracle
        path.with_suffix('.python.tsv').write_text(oracle)
        path.with_suffix('.rust.tsv').write_text(actual)
        reports.append({'label': label, 'sha256': hashlib.sha256(data).hexdigest(),
            'rows_sha256': hashlib.sha256(oracle.encode()).hexdigest(), 'queries': len(oracle.splitlines()),
            'controlled_declaration_changes': changes})
    report = {'source_sha256': source_hash, 'vertices_per_case': count, 'cases': reports,
        'comparison': 'Every requested attribute scalar bit, skin weight/index, ambiguity/incomplete/absence result and vertex boundary agrees.',
        'limitations': 'One actual LOD0 mesh with eight declaration cases; controls are not game-generated files or evidence of duplicate semantic usage in game resources.'}
    text = json.dumps(report, indent=2) + '\n'
    pin = json.loads((repo / 'tests/fixtures/mdl-semantic-query-observations.json').read_text())
    assert source_hash == pin['source_sha256']
    assert hashlib.sha256(text.encode()).hexdigest() == pin['report_sha256']
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

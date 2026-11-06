"""Generic full homogeneous 4x4 products from observed PBD coefficient words.

Args: extracted-human.pbd new-output-directory. Adjacent stored matrices are
arbitrary math test pairs, not connected bones, deformer routes or native calls.
Python uses f64 four-term dot products; Rust uses f32 affine operations.
"""
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
from verify_pbd import observe


def main():
    source, out = map(Path, sys.argv[1:])
    assert source.stat().st_size <= 64 << 20
    out.mkdir(parents=True, exist_ok=False)
    fixtures = Path(__file__).resolve().parent.parent / 'tests/fixtures'
    pin = json.loads((fixtures / 'pbd-observations.json').read_text())
    data = source.read_bytes()
    assert hashlib.sha256(data).hexdigest() == pin['sha256']
    rows = []
    stats = dict.fromkeys(['pbd_tables', 'deformers', 'bones', 'float_coefficients',
        'nonidentity_matrices', 'translated_matrices', 'offdiagonal_matrices', 'extended_entries'], 0)
    observe(data, 'root', rows, stats)
    assert hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest() == pin['rows_sha256']
    tables = []
    for row in rows:
        f = row.split('\t')
        if f[0] == 'pbd':
            tables.append((f[1], []))
        elif f[0] == 'deformer':
            tables[-1][1].append([])
        elif f[0] == 'bone':
            words = [int(v, 16) for v in f[3].split(',')]
            values = struct.unpack('<12f', struct.pack('<12I', *words))
            tables[-1][1][-1].append([list(values[i:i+4]) for i in (0, 4, 8)]+[[0., 0., 0., 1.]])

    def multiply(a, b):
        return [[sum(a[r][k]*b[k][c] for k in range(4)) for c in range(4)] for r in range(4)]

    expected = []
    noncommuting = 0
    for path, deformers in tables:
        for i, matrices in enumerate(deformers):
            for j, a in enumerate(matrices):
                b = matrices[(j+1) % len(matrices)]
                ab, ba = multiply(a, b), multiply(b, a)
                noncommuting += ab != ba
                values = [v for m in (ab, ba) for row in m[:3] for v in row]
                for p in ([0, 0, 0, 1], [1, -2, 3, 1], [-4, 5, .5, 1]):
                    values.extend(sum(ab[r][k]*p[k] for k in range(4)) for r in range(3))
                expected.append((['product', path, str(i), str(j)], values))
    actual = subprocess.check_output(['target/debug/examples/validate_pbd_composition', str(source)], text=True, timeout=120)
    lines = actual.splitlines()
    assert len(lines) == len(expected)
    max_scaled_error = max_absolute_error = 0.
    for line, (prefix, values) in zip(lines, expected):
        f = line.split('\t')
        assert f[:4] == prefix and len(f) == 5
        got = list(map(float, f[4].split(',')))
        assert len(got) == len(values) == 33
        for a, b in zip(got, values):
            assert math.isfinite(a) and math.isfinite(b)
            error = abs(a-b)
            scaled = error / max(1., abs(b))
            assert scaled <= 2e-6, (prefix, a, b, scaled)
            max_scaled_error = max(max_scaled_error, scaled)
            max_absolute_error = max(max_absolute_error, error)
    (out / 'rust.tsv').write_text(actual)
    oracle = '\n'.join('\t'.join(prefix)+ '\t'+','.join(format(v, '.17e') for v in values) for prefix, values in expected)+'\n'
    (out / 'python.tsv').write_text(oracle)
    result = {'resource_sha256': pin['sha256'], 'pairs': len(expected),
        'noncommuting_pairs': noncommuting, 'compared_components': 33*len(expected),
        'max_absolute_error': max_absolute_error, 'max_scaled_error': max_scaled_error,
        'scaled_tolerance': 2e-6, 'oracle_rows_sha256': hashlib.sha256(oracle.encode()).hexdigest(),
        'limitations': 'General matrix math on real coefficients; no native execution, chain routing or final deformation-space oracle.'}
    extra = fixtures / 'pbd-composition-observations.json'
    if extra.exists():
        saved = json.loads(extra.read_text())
        assert all(result[k] == saved[k] for k in ('resource_sha256', 'pairs', 'noncommuting_pairs', 'compared_components', 'oracle_rows_sha256'))
    text = json.dumps(result, indent=2)+'\n'
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

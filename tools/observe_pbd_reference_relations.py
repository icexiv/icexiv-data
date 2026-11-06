"""Test four explicit reference-pose hypotheses against actual PBD coefficients.

Args: human.pbd c0101.sklb c0201.sklb c0901.sklb c1801.sklb new-output-directory.
This uses ordinary normalized-quaternion TRS and f64 matrix algebra, not native
Havok pose evaluation. It neither constructs replacement PBD data nor establishes
the true application-space formula. Generic inverses use Gauss-Jordan elimination.
"""
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
from observe_tagfile import read
from verify_pbd import observe


IDENTITY = [[float(i == j) for j in range(4)] for i in range(4)]


def product(a, b):
    return [[sum(a[i][k]*b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def inverse(a):
    rows = [a[i][:]+IDENTITY[i][:] for i in range(4)]
    for column in range(4):
        pivot = max(range(column, 4), key=lambda i: abs(rows[i][column]))
        rows[column], rows[pivot] = rows[pivot], rows[column]
        value = rows[column][column]
        assert value != 0
        rows[column] = [v/value for v in rows[column]]
        for i in range(4):
            if i != column:
                factor = rows[i][column]
                rows[i] = [v-factor*w for v, w in zip(rows[i], rows[column])]
    return [row[4:] for row in rows]


def local(trs):
    norm = math.sqrt(sum(v*v for v in trs[4:8]))
    assert norm > 0
    x, y, z, w = [v/norm for v in trs[4:8]]
    rotation = [[1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)],
        [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)],
        [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)]]
    return [[rotation[i][j]*trs[8+j] for j in range(3)]+[trs[i]] for i in range(3)]+[[0., 0., 0., 1.]]


def skeleton(path):
    observer = read(path)
    containers = [o for o in observer.objects if o and observer.classes[o['class']]['name'] == b'hkaAnimationContainer']
    ids = [v['ref'] for o in containers for v in o['fields'].get(b'skeletons', []) if v['ref']]
    assert len(ids) == 1
    fields = observer.objects[ids[0]]['fields']
    parents, poses = fields[b'parentIndices'], fields[b'referencePose']
    names = [bone['fields'][b'name'].decode('ascii') for bone in fields[b'bones']]
    assert len(parents) == len(poses) == len(names) <= 4096 and len(set(names)) == len(names)
    # Iterative forest evaluation keeps hierarchy cycles and malformed parents
    # explicit; no recursion-depth assumption or nearest-name matching.
    global_pose, remaining = {}, set(range(len(names)))
    while remaining:
        ready = [i for i in sorted(remaining) if parents[i] == -1 or parents[i] in global_pose]
        assert ready, 'unresolved parent or cycle'
        for i in ready:
            transform = local(poses[i])
            global_pose[i] = product(global_pose[parents[i]], transform) if parents[i] != -1 else transform
            remaining.remove(i)
    return {name: global_pose[i] for i, name in enumerate(names)}, {
        'bones': len(names),
        'raw_quaternion_squared_length_max_deviation': max(abs(sum(v*v for v in t[4:8])-1) for t in poses),
        'raw_scale_max_deviation_from_one': max(abs(v-1) for t in poses for v in t[8:11])}


def main():
    # Controlled algebra checks are independent of any resource interpretation.
    simple = local([1., 2., 3., 0., 0., 0., 0., 1., 2., 3., 4., 0.])
    assert simple == [[2., 0., 0., 1.], [0., 3., 0., 2.], [0., 0., 4., 3.], [0., 0., 0., 1.]]
    roundtrip = product(simple, inverse(simple))
    assert max(abs(roundtrip[i][j]-IDENTITY[i][j]) for i in range(4) for j in range(4)) < 1e-12
    quarter_turn = local([0., 0., 0., 0., 0., 0., math.sqrt(.5), math.sqrt(.5), 1., 1., 1., 0.])
    assert abs(quarter_turn[0][0]) < 1e-12 and abs(quarter_turn[1][0]-1) < 1e-12
    pbd, *rest = [Path(p).resolve() for p in sys.argv[1:]]
    assert len(rest) == 5
    paths, out = rest[:4], rest[4]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and all(not out.is_relative_to(p.parent) for p in [pbd, *paths])
    fixture = repo/'tests/fixtures/pbd-reference-relation-observations.json'
    pin = json.loads(fixture.read_text())
    assert all(p.stat().st_size <= 16 << 20 for p in [pbd, *paths])
    hashes = [hashlib.sha256(p.read_bytes()).hexdigest() for p in [pbd, *paths]]
    assert hashes == pin['input_sha256']
    rows, stats = [], dict.fromkeys(['pbd_tables', 'deformers', 'bones', 'float_coefficients',
        'nonidentity_matrices', 'translated_matrices', 'offdiagonal_matrices', 'extended_entries'], 0)
    observe(pbd.read_bytes(), 'root', rows, stats)
    wire_pin = json.loads((repo/'tests/fixtures/pbd-observations.json').read_text())
    assert hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest() == wire_pin['rows_sha256']
    matrices, connections, deformer_connections = {}, {}, {}
    current = None
    for row in rows:
        fields = row.split('\t')
        if fields[0] == 'pbd' and fields[1] != 'root': break
        if fields[0] == 'deformer':
            current = int(fields[2]); matrices[current] = {}
            deformer_connections[current] = int(fields[3])
        if fields[0] == 'connection': connections[int(fields[1])] = list(map(int, fields[2:]))
        if fields[0] == 'bone':
            values = [struct.unpack('<f', struct.pack('<I', int(word, 16)))[0] for word in fields[3].split(',')]
            matrices[current][bytes.fromhex(fields[2]).decode('ascii')] = [values[i*4:i*4+4] for i in range(3)]+[IDENTITY[3][:]]
    globals_, samples = {}, []
    for identity, path, digest in zip([101, 201, 901, 1801], paths, hashes[1:]):
        globals_[identity], description = skeleton(path)
        samples.append({'id': identity, 'sha256': digest, **description})
    pairs, lines = [], []
    labels = ['A_times_inverse_B', 'inverse_A_times_B', 'B_times_inverse_A', 'inverse_B_times_A']
    for child, parent in [(201, 101), (901, 101), (1801, 201)]:
        # The parent identity is independently checked against wire connection
        # indices; it is not inferred from race-number arithmetic.
        connection = connections[deformer_connections[child]]
        assert connection[0] == deformer_connections[parent]
        errors = [[] for _ in labels]
        for name, actual in matrices[child].items():
            assert name in globals_[child] and name in globals_[parent]
            a, b = globals_[parent][name], globals_[child][name]
            hypotheses = [product(a, inverse(b)), product(inverse(a), b), product(b, inverse(a)), product(inverse(b), a)]
            for i, candidate in enumerate(hypotheses):
                error = max(abs(candidate[r][c]-actual[r][c]) for r in range(3) for c in range(4))
                assert math.isfinite(error)
                errors[i].append((error, name))
                lines.append(f'{child}\t{parent}\t{name}\t{labels[i]}\t'+','.join(v.hex() for row in candidate[:3] for v in row))
        summaries = []
        for label, observations in zip(labels, errors):
            error, name = max(observations)
            summaries.append({'hypothesis': label, 'maximum_absolute_coefficient_error': error,
                'maximum_error_bone': name, 'mean_per_bone_maximum_error': sum(v for v, _ in observations)/len(observations)})
        pairs.append({'child': child, 'parent': parent, 'matched_bones': len(matrices[child]), 'hypotheses': summaries})
    trace = '\n'.join(lines)+'\n'
    report = {'version': pin['version'], 'platform': 'Win32', 'pbd_sha256': hashes[0],
        'skeletons': samples, 'pairs': pairs, 'hypotheses_checked': len(lines),
        'coefficient_comparisons': len(lines)*12, 'hypothesis_rows_sha256': hashlib.sha256(trace.encode()).hexdigest(),
        'definition': 'A/B are parent/child skeleton hierarchy matrices using column vectors, normalized xyzw quaternions and local T*R*S. Ancestors multiply from root to child, in f64. Only identical bone names are matched.',
        'limitations': 'A controlled mathematical hypothesis check on actual resource coefficients, not native Havok pose reproduction or runtime observation. Rejecting these four formulas does not exclude other reference-derived formulas and does not establish the real PBD source/target/bind-space contract.'}
    text = json.dumps(report, indent=2)+'\n'
    digest = hashlib.sha256(text.encode()).hexdigest()
    assert digest == pin['report_sha256'], digest
    out.mkdir(parents=True, exist_ok=False)
    (out/'hypotheses.tsv').write_text(trace)
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__': main()

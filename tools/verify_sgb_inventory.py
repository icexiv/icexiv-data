"""Bounded furniture SGB inventory and structural dependency closure.

Args: game/sqpack new-output-directory. General indoor/outdoor IDs 1..2000,
then explicit SharedGroup pointers only. Independent type-2/struct observations
are compared with Rust; referenced model/collision presence is checked, not
their geometry or behavior. No guessed strings, target execution or install writes.
"""
from collections import Counter, deque
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
from find_vfx_samples import standard
from observe_sgb import observe
from verify_sgb import rows


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    version = (root.parent / 'ffxivgame.ver').read_text().strip()
    index = root / 'ffxiv/010000.win32.index'
    assert index.stat().st_size <= 128 << 20
    index_bytes = index.read_bytes()
    header, = struct.unpack_from('<I', index_bytes, 12)
    offset, size = struct.unpack_from('<II', index_bytes, header+8)
    assert size <= 64 << 20 and size % 16 == 0 and offset+size <= len(index_bytes)
    entries = {}
    for ordinal, (key, packed, _) in enumerate(struct.iter_unpack('<QII', index_bytes[offset:offset+size])):
        if key in entries:
            raise ValueError('duplicate index key requires explicit collision resolution')
        entries[key] = (packed, ordinal)

    def locate(path):
        assert path.isascii() and path.startswith('bgcommon/') and '..' not in path.split('/')
        directory, filename = path.rsplit('/', 1)
        jam = lambda s: ~zlib.crc32(s.lower().encode('ascii')) & 0xffffffff
        key = jam(directory) << 32 | jam(filename)
        if key not in entries:
            return None
        packed, ordinal = entries[key]
        if packed & 1:
            raise ValueError(f'synonym entry requires resolution: {path}')
        return f'ffxiv/010000.win32.dat{(packed >> 1) & 7}', (packed >> 4)*128, ordinal, f'{key:016x}'

    queue = deque()
    absent = []
    for domain, prefix in [('indoor', 'fun'), ('outdoor', 'gar')]:
        for i in range(1, 2001):
            path = f'bgcommon/hou/{domain}/general/{i:04}/asset/{prefix}_b0_m{i:04}.sgb'
            if locate(path) is None:
                absent.append(path)
            else:
                queue.append(path)
    roots = len(queue)
    seen, samples, missing, profiles = set(), [], [], []
    counts = Counter()
    while queue:
        path = queue.popleft()
        if path in seen:
            continue
        assert len(seen) < 8192
        seen.add(path)
        location = locate(path)
        if location is None:
            missing.append({'kind': 'shared_group', 'path': path})
            continue
        dat, entry, ordinal, key = location
        with (root/dat).open('rb') as f:
            raw = standard(f, entry, max_bytes=64 << 20)
        if raw is None:
            raise ValueError(f'non-type-2 or oversized SGB: {path}')
        number = len(samples)
        target = out / f'{number:04}.sgb'
        target.write_bytes(raw)
        rust_target = out / f'{number:04}.rust.sgb'
        extraction = subprocess.check_output(['target/debug/examples/local_sqpack', str(root), path, str(rust_target)], text=True, timeout=120)
        assert rust_target.read_bytes() == raw
        (out/f'{number:04}.extraction.txt').write_text(extraction)
        sample = {'path': path, 'dat': dat, 'offset': entry, 'index_ordinal': ordinal,
            'index_hash': key, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
        samples.append(sample)
        assert len(raw) >= 84 and struct.unpack_from('<4sI', raw) == (b'SGB1', len(raw))
        profile = (struct.unpack_from('<I', raw, 8)[0], raw[12:16].decode('ascii'),
            struct.unpack_from('<I', raw, 16)[0], struct.unpack_from('<I', raw, 24)[0])
        counts['profile:'+','.join(map(str, profile))] += 1
        if profile != (1, 'SCN1', 72, 1):
            profiles.append({'path': path, 'profile': profile})
            continue
        observation = observe(raw)
        expected, deps = rows(raw, observation)
        actual = subprocess.check_output(['target/debug/examples/validate_sgb', str(target)], text=True, timeout=120)
        assert actual == expected, path
        (out/f'{number:04}.python.tsv').write_text(expected)
        (out/f'{number:04}.rust.tsv').write_text(actual)
        sample['rows_sha256'] = hashlib.sha256(expected.encode()).hexdigest()
        sample['layers'] = len(observation['layers'])
        sample['instances'] = sum(len(l['instances']) for l in observation['layers'])
        sample['dependencies'] = []
        counts['compared_resources'] += 1
        counts['layers'] += sample['layers']
        counts['instances'] += sample['instances']
        counts['multi_layer_resources'] += sample['layers'] > 1
        for layer in observation['layers']:
            for instance in layer['instances']:
                counts[f'instance_type:{instance["type"]}'] += 1
        for kind, value, *_ in deps:
            dependency = value.decode('ascii')
            sample['dependencies'].append({'kind': kind, 'path': dependency})
            counts[f'dependency:{kind}'] += 1
            if locate(dependency) is None:
                missing.append({'owner': path, 'kind': kind, 'path': dependency})
            elif kind == 'shared_group':
                queue.append(dependency)
    result = {'version': version, 'platform': 'Win32', 'index_sha256': hashlib.sha256(index_bytes).hexdigest(),
        'candidate_paths': 4000, 'indexed_roots': roots, 'confirmed_absent_candidates': len(absent),
        'statistics': dict(sorted(counts.items())), 'samples': samples,
        'unsupported_profiles': profiles, 'missing_dependencies': missing,
        'limitations': 'Bounded general-ID range and explicit SharedGroup closure in base bgcommon index; no complete housing catalog, other-type payload, transform application or geometry claim.'}
    result['samples_sha256'] = hashlib.sha256(json.dumps(samples, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    summary = {k: v for k, v in result.items() if k != 'samples'}
    pin = repo/'tests/fixtures/sgb-inventory-observations.json'
    if pin.exists():
        assert json.loads(pin.read_text()) == summary
    text = json.dumps(result, indent=2)+'\n'
    (out/'report.json').write_text(text)
    (out/'absent-candidates.json').write_text(json.dumps(absent, indent=2)+'\n')
    print(json.dumps(summary, indent=2))
    assert not profiles and not missing, 'inventory exposes unresolved profiles or references; see report'


if __name__ == '__main__':
    main()

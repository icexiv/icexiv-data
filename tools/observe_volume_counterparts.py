"""Bounded negative evidence, not recovery of incomplete volume bytes.

Args: game/sqpack pinned-volume-inventory-report.json new-output-directory.
The inventory digest is checked before using its observations. Main index hash
tables are read independently, without reading DATs or resolving resource names.
"""
import collections
import hashlib
import json
from pathlib import Path
import struct
import sys


def main():
    root, inventory, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    assert inventory.stat().st_size <= 8 << 20
    raw = inventory.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    pin = json.loads((repo/'tests/fixtures/volume-inventory-observations.json').read_text())
    full_pin = json.loads((repo/'tests/fixtures/volume-full-inventory-observations.json').read_text())
    assert digest in (pin['report_sha256'], full_pin['report_sha256'])
    full_inventory = digest == full_pin['report_sha256']
    observed = json.loads(raw)
    version = (root.parent/'ffxivgame.ver').read_text().strip()
    assert observed['version'] == version
    assert len(observed['samples']) <= 2048
    complete = collections.defaultdict(list)
    gaps = []
    for sample in observed['samples']:
        key = (sample['width'], sample['height'], sample['depth'], sample['format'],
            sample['group_sha256'][0])
        if sample['status'] == 'complete':
            complete[key].append(sample)
        else:
            assert sample['status'] == 'incomplete'
            gaps.append((key, sample))
    matches = [{'gap': [s['dat'], s['offset']],
        'complete': [[f['dat'], f['offset']] for f in complete[key]]}
        for key, s in gaps if key in complete]
    indexes = sorted(root.glob('*/*.win32.index'))
    assert 0 < len(indexes) <= 128
    tables = []
    budget = 128 << 20
    for path in indexes:
        with path.open('rb') as f:
            f.seek(12)
            header, = struct.unpack('<I', f.read(4))
            assert 0 < header <= 2 << 20
            f.seek(header+8)
            offset, size = struct.unpack('<II', f.read(8))
            assert size <= 64 << 20 and size % 16 == 0
            budget -= size
            assert budget >= 0
            f.seek(offset)
            table = f.read(size)
            assert len(table) == size
        seen = set()
        flagged = duplicates = 0
        for key, packed, _ in struct.iter_unpack('<QII', table):
            flagged += bool(packed & 1)
            duplicates += key in seen
            seen.add(key)
        tables.append({'index': str(path.relative_to(root)), 'records': size//16,
            'table_sha256': hashlib.sha256(table).hexdigest(),
            'flagged_records': flagged, 'duplicate_hash_records': duplicates})
    result = {'version': version, 'platform': 'Win32', 'inventory_sha256': digest,
        'gap_entries': len(gaps), 'gap_base_groups': len({key for key, _ in gaps}),
        'same_base_complete_counterparts': matches,
        'main_index_table_bytes': (128 << 20)-budget, 'indexes': tables,
        'limitations': 'Counterpart comparison covers only the pinned selected-volume inventory. Equal base digests would not prove equal lower mips. Main-table flags and duplicate hashes do not decode synonym tables, index2, resource names or unselected DAT records. Negative observations do not recover bytes or count as decoding success.'}
    text = json.dumps(result, indent=2)+'\n'
    expected = json.loads((repo/'tests/fixtures/volume-counterpart-observations.json').read_text())
    expected_digest = full_pin['counterpart_report_sha256'] if full_inventory else expected['report_sha256']
    assert hashlib.sha256(text.encode()).hexdigest() == expected_digest
    out.mkdir(parents=True, exist_ok=False)
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

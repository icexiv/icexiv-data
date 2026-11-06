"""Validate index2-only lookup against independently observed primary locations.

Args: game/sqpack index2-inventory-report new-output-directory.
Extraction comparison reuses the library's DAT decoder; it is not a new decoder
oracle. Location/name/hash observations use the independent Python inventory.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    root, inventory, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    pin = json.loads((repo/'tests/fixtures/index2-collision-observations.json').read_text())
    assert hashlib.sha256(inventory.read_bytes()).hexdigest() == pin['report_sha256']
    observation = json.loads(inventory.read_text())
    out.mkdir(parents=True, exist_ok=False)
    lines, entries = [], []
    for pack in observation['indexes']:
        fields = [pack['index'] + '2']
        for row in pack['collision_records']:
            dat = pack['index'].removesuffix('.index') + f'.dat{row["dat"]}'
            fields += [row['path'], dat, str(row['offset'])]
            entries.append(row | {'dat_file': dat})
        lines.append('\t'.join(fields))
    manifest = out/'manifest.tsv'
    manifest.write_text('\n'.join(lines)+'\n')
    result = subprocess.check_output(['target/debug/examples/validate_index2', str(root),
        str(manifest), str(out/'decoded')], text=True, timeout=120)
    (out/'lookup.log').write_text(result)
    assert len(result.splitlines()) == len(entries) == 62
    for i, row in enumerate(entries):
        data = (out/'decoded'/f'{i:03}.bin').read_bytes()
        row['decoded_bytes'] = len(data)
        row['decoded_sha256'] = hashlib.sha256(data).hexdigest()
    report = {'version': observation['version'], 'platform': 'Win32',
        'inventory_sha256': pin['report_sha256'], 'containers_compared': len(lines),
        'collision_paths_verified': len(entries), 'entries': entries,
        'scope': 'Complete borrowed and async index2 table/auxiliary byte equality across 59 containers. All 62 collision paths located against independently reconciled primary locations and extracted through a backend that confirms primary index absence. DAT bytes compared with explicit-location library extraction, not an independent decoder oracle.'}
    text = json.dumps(report, indent=2)+'\n'
    expected = json.loads((repo/'tests/fixtures/index2-lookup-observations.json').read_text())
    digest = hashlib.sha256(text.encode()).hexdigest()
    assert digest == expected['report_sha256'], digest
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__': main()

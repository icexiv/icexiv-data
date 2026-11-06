"""EQP hair field against independent sequential table expansion and byte cases.

Args: extracted-equipmentparameter.eqp new-output-directory. Field values only;
does not evaluate the native hair switch, ear classification or visibility.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    source, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and not out.is_relative_to(source.parent)
    assert source.stat().st_size <= 64 * 160 * 8
    raw = source.read_bytes()
    fixture = json.loads((repo / 'tests/fixtures/eqp-observations.json').read_text())
    assert hashlib.sha256(raw).hexdigest() == fixture['sha256']
    bitmap, = struct.unpack_from('<Q', raw)
    stored, rows, histogram = 0, [], [0] * 8
    for block in range(64):
        values = struct.unpack_from('<160Q', raw, stored * 1280) if bitmap >> block & 1 else None
        stored += values is not None
        for i in range(160):
            identity = block * 160 + i
            if identity == 0:
                continue
            if values is None:
                rows.append(f'hair\t{identity}\tabsent')
            else:
                head = values[i] // (2**40)
                code = (head // 2) % 8
                histogram[code] += 1
                rows.append(f'hair\t{identity}\t{head:06x}\t{code}')
    assert stored * 1280 == len(raw)
    for byte in range(256):
        rows.append(f'control\t{byte}\t{byte//2%8}\t{byte//2%2}\t{byte//4%2}\t{byte//8%2}')
    expected = '\n'.join(rows) + '\n'
    actual = subprocess.check_output(['target/debug/examples/validate_eqp_hair', str(source)], text=True, timeout=120)
    assert expected == actual
    out.mkdir(parents=True, exist_ok=False)
    (out / 'hair.python.tsv').write_text(expected)
    (out / 'hair.rust.tsv').write_text(actual)
    report = {'eqp_sha256': fixture['sha256'], 'rows_sha256': hashlib.sha256(expected.encode()).hexdigest(),
        'present_entries': sum(histogram), 'hair_code_histogram': histogram, 'controlled_bytes': 256,
        'limitations': 'Field extraction on actual records plus controlled raw head bytes, not native visibility or race-state evaluation.'}
    text = json.dumps(report, indent=2) + '\n'
    pin = json.loads((repo / 'tests/fixtures/eqp-hair-observations.json').read_text())
    assert hashlib.sha256(text.encode()).hexdigest() == pin['report_sha256']
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

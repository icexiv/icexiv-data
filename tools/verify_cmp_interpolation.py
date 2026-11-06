"""Exact rational interpolation of actual CMP ranges versus public math helper.

Args: extracted-human.cmp new-output-directory. Fraction arithmetic is applied
to the independent struct-decoded endpoints. No client function is simulated.
"""
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    source, out = map(Path, sys.argv[1:])
    assert source.stat().st_size == 186752
    out.mkdir(parents=True,exist_ok=False)
    fixtures = Path(__file__).resolve().parent.parent/'tests/fixtures'
    pin = json.loads((fixtures/'cmp-observations.json').read_text())
    b = source.read_bytes()
    assert hashlib.sha256(b).hexdigest() == pin['sha256']
    rows = []
    positions = [(0,1),(2,3),(4,5),(6,7),(8,11),(9,12),(10,13)]
    for i, values in enumerate(struct.iter_unpack('<14f', b[-80*56:])):
        for percent in range(101):
            words = []
            for low, high in positions:
                exact = (Fraction(values[low])*(100-percent)+Fraction(values[high])*percent)/100
                bits, = struct.unpack('<I',struct.pack('<f',float(exact)))
                words.append(f'{bits:08x}')
            rows.append(f'sample\t{i}\t{percent}\t'+','.join(words))
    expected = '\n'.join(rows)+'\n'
    rust = subprocess.check_output(['target/debug/examples/validate_cmp_interpolation',str(source)],text=True)
    assert rust == expected
    (out/'rust.tsv').write_text(rust)
    (out/'python.tsv').write_text(expected)
    result = {'resource_sha256': pin['sha256'], 'records':80, 'scalar_ranges':560,
        'percentages_per_range':101, 'scalar_results':56560, 'invalid_percentage_sets':80*155,
        'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),
        'limitations':'Defined mathematical range helper with strict percentages; not native sentinel/fallback policy, instruction-level rounding or pose scaling.'}
    extra = fixtures/'cmp-interpolation-observations.json'
    if extra.exists():
        saved = json.loads(extra.read_text())
        assert result['rows_sha256'] == saved['rows_sha256'] and result['resource_sha256'] == saved['resource_sha256']
    text = json.dumps(result,indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__ == '__main__':
    main()

"""Bounded deterministic mutation safety on fixed extracted resources.

Args: game/sqpack completed-or-in-progress-regression-directory new-output-dir.
No target execution, GPU, server, extra parser library or writes to installation.
Acceptance/rejection counts do not establish the semantic validity of mutations.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    root, suite, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    assert not out.is_relative_to(suite/'inputs')
    cases = [
        *[('mtrl', f'inputs/material-{i}.mtrl', None) for i in range(5)],
        *[('mdl', f'model-reconstruction/{i}.mdl', None) for i in range(2)],
        *[('shpk', f'shpk-variants/{i}.shpk', None) for i in (0, 1)],
        *[('tex', f'inputs/{name}.tex', None) for name in ('float-lut', 'icon', 'cube-rgba4', 'volume-l8', 'bg-array-bc7')],
        ('tex', 'bc6h/actual-zero-cube.tex', None),
        ('stm', 'inputs/stm-0.stm', None), ('stm', 'inputs/stm-1.stm', None),
        ('eqp', 'inputs/equipmentparameter.eqp', None), ('cmp', 'inputs/human.cmp', None),
        ('pbd', 'inputs/human.pbd', None), ('eqdp', 'eqdp/equipment-c0101.eqdp', None),
        ('imc', 'imc/0.imc', None), ('sgb', 'sgb/0.sgb', None),
        ('avfx', 'avfx-assets/0.avfx', None),
        ('sklb', 'skeleton/skl_c0101b0001.sklb', None), ('pap', 'pap/0/sample.pap', None),
        ('exh', 'inputs/stain.exh', None), ('exd', 'inputs/stain_0_chs.exd', 'inputs/stain.exh'),
        ('index2', '@ffxiv/010000.win32.index2', None)]
    out.mkdir(parents=True, exist_ok=False)
    lines, samples = [], []
    for kind, relative, aux in cases:
        path = root/relative[1:] if relative.startswith('@') else suite/relative
        auxiliary = suite/aux if aux else None
        assert path.stat().st_size <= 16 << 20
        lines.append('\t'.join([kind, str(path), str(auxiliary) if aux else '-']))
        samples.append({'kind': kind, 'source': relative, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'auxiliary_sha256': hashlib.sha256(auxiliary.read_bytes()).hexdigest() if auxiliary else None})
    fixture = repo/'tests/fixtures/mutation-safety-observations.json'
    pin = json.loads(fixture.read_text())
    version = (root.parent/'ffxivgame.ver').read_text().strip()
    assert version == pin['version']
    assert hashlib.sha256(json.dumps(samples, separators=(',', ':')).encode()).hexdigest() == pin['samples_sha256']
    manifest = out/'manifest.tsv'
    manifest.write_text('\n'.join(lines)+'\n')
    command = ['target/debug/examples/validate_mutations', str(manifest), str(out/'runs')]
    with (out/'rust.tsv').open('x') as output, (out/'stderr.txt').open('x') as error:
        subprocess.run(command, check=True, stdout=output, stderr=error, timeout=600)
    stats = []
    for i, line in enumerate((out/'rust.tsv').read_text().splitlines()):
        ordinal, kind, calls, parsed, queried = line.split('\t')
        assert int(ordinal) == i and kind == samples[i]['kind']
        stats.append({'source': samples[i]['source'], 'attempts': int(calls),
            'parse_accepted': int(parsed), 'selected_followup_accepted': int(queried)})
    assert len(stats) == len(samples)
    report = {'version': version, 'platform': 'Win32', 'samples': samples, 'results': stats,
        'total_attempts': sum(s['attempts'] for s in stats), 'panics': 0,
        'followups': 'TEX mip0 first surface RGBA8 and RGBA32F attempts (accepted count is RGBA8), SKLB skeleton extraction, PAP first binding/spline sample at 125ms, MTRL color-table query. Other cases exercise parsing only. EXD mutations use a fixed valid EXH.',
        'generator': 'Baseline; prefixes 0..min(len,128); quarter/half/three-quarter/end-minus-one cuts; aligned first-256-byte u32 writes of 0,FFFFFFFF,80000000,7FFFFFFF; 256 xorshift64 mutations alternating first512/body, widths1/2/4/8, seed8bd42f193a7e6501 XOR case index.',
        'limits': '64 cases, 16MiB/snapshot, 64MiB cumulative input. Havok 16MiB allocations/200000 values/arrays/1000000 validation steps; spline 8MiB allocations/4096 tracks/1024 blocks/100000 controls/1000000 decode steps; sample/TEX output1MiB, sample work100000. Other parsers use their documented defaults.',
        'scope': 'Controlled mutations of fixed real-resource seeds, not new production variants or independent format/pixel proof. Returning an error is expected for malformed or over-budget inputs. No exhaustive no-panic claim, DAT-backend mutation coverage, time guarantee per parser, or new successful-decoding count.'}
    text = json.dumps(report, indent=2)+'\n'
    digest = hashlib.sha256(text.encode()).hexdigest()
    assert digest == pin['report_sha256'], digest
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__': main()

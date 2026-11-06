"""Build a CPU-only Microsoft oracle from explicit, previously reviewed sources.

Args: DirectXTex-root DirectXMath-root SAL-directory new-output-directory.
No implicit downloads or source edits. SAL directory contains sal.h/LICENSE.TXT.
Output retains all source notices and exact build/source/binary identities.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    tex, math, sal, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo)
    assert all(not out.is_relative_to(p) for p in (tex, math, sal))
    pin = json.loads((repo/'tests/fixtures/bc6h-oracle-sources.json').read_text())
    for name, root in [('DirectXTex', tex), ('DirectXMath', math), ('SAL', sal)]:
        for path, digest in pin[name]['files'].items():
            assert hashlib.sha256((root/path).read_bytes()).hexdigest() == digest, (name, path)
    digest = hashlib.sha256()
    for path in sorted((math/'Inc').glob('*')):
        if path.is_file():
            digest.update(path.name.encode()+b'\0'+hashlib.sha256(path.read_bytes()).digest())
    assert digest.hexdigest() == pin['DirectXMath']['inc_sha256']
    out.mkdir(parents=True, exist_ok=False)
    for name in ['BC6HBC7.cpp', 'BC.h']:
        shutil.copyfile(tex/'DirectXTex'/name, out/name)
    for name, path in [('DirectXTex-LICENSE', tex/'LICENSE'), ('DirectXMath-LICENSE', math/'LICENSE'), ('SAL-LICENSE', sal/'LICENSE.TXT')]:
        shutil.copyfile(path, out/name)
    adapter = repo/'tools/bc6h_oracle'
    command = ['g++', '-std=c++17', '-O2', '-DNDEBUG', '-ffp-contract=off',
        '-I'+str(adapter), '-I'+str(out), '-I'+str(math/'Inc'), '-I'+str(sal),
        str(out/'BC6HBC7.cpp'), str(adapter/'main.cpp'), '-o', str(out/'oracle')]
    subprocess.run(command, check=True, timeout=120)
    float_command = ['g++', '-std=c++17', '-O2', '-DNDEBUG', '-ffp-contract=off',
        '-I'+str(adapter), '-I'+str(math/'Inc'), '-I'+str(sal),
        str(adapter/'float.cpp'), '-o', str(out/'float-oracle')]
    subprocess.run(float_command, check=True, timeout=120)
    report = {'sources': pin, 'command': command,
        'compiler': subprocess.check_output(['g++', '--version'], text=True).splitlines()[0],
        'adapter_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(adapter.iterdir()) if p.is_file()},
        'oracle_sha256': hashlib.sha256((out/'oracle').read_bytes()).hexdigest(),
        'float_command': float_command,
        'float_oracle_sha256': hashlib.sha256((out/'float-oracle').read_bytes()).hexdigest()}
    (out/'report.json').write_text(json.dumps(report, indent=2)+'\n')


if __name__ == '__main__':
    main()

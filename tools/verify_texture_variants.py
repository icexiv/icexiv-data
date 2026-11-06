"""Pinned type-4 texture variants: index identity, extraction, all mip pixels.

Args: game/sqpack new-output-directory [manifest.json]. Uses the independently reviewed generic
Pillow/struct oracles from verify_tex; no game execution or parser dependency.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import PIL
from verify_tex import reconstruct, observe


def main():
    root, out = map(Path, sys.argv[1:3])
    assert not out.resolve().is_relative_to(root.parent.resolve())
    manifest_path = Path(sys.argv[3]) if len(sys.argv) == 4 else Path(__file__).resolve().parent.parent / 'tests/fixtures/tex-variant-observations.json'
    manifest = json.loads(manifest_path.read_text())
    version = (root.parent / 'ffxivgame.ver').read_text().strip()
    assert version == manifest['version']
    out.mkdir(parents=True, exist_ok=False)
    paths = []
    sources = manifest['samples']
    assert 1 <= len(sources) <= 32
    for i, source in enumerate(sources):
        index = (root / source['index']).resolve()
        assert index.is_relative_to(root.resolve())
        with index.open('rb') as f:
            f.seek(12)
            hs, = struct.unpack('<I', f.read(4))
            f.seek(hs+8)
            offset, size = struct.unpack('<II', f.read(8))
            assert size <= 64 << 20 and 0 <= source['ordinal'] < size // 16
            f.seek(offset+source['ordinal']*16)
            identity, loc, _ = struct.unpack('<QII', f.read(16))
        assert loc & 1 == 0 and f'{identity:016x}' == source['hash']
        dat = index.with_suffix(f'.dat{(loc >> 1) & 7}')
        assert dat == (root / source['dat']).resolve() and (loc >> 4)*128 == source['offset']
        independent, _ = reconstruct(root, source)
        assert len(independent) <= 16 << 20
        assert hashlib.sha256(independent[:80]).hexdigest() == source['header_sha256']
        path = out / f'{i}.tex'
        log = subprocess.check_output(['target/debug/examples/extract_entry', str(root),
            source['dat'], str(source['offset']), str(path)], text=True)
        (out / f'{i}.extract.txt').write_text(log)
        assert path.read_bytes() == independent
        paths.append(path)
    pixels = out / 'pixels'
    rust = subprocess.check_output(['target/debug/examples/validate_tex', str(pixels), *map(str, paths)], text=True)
    (out / 'rust.tsv').write_text(rust)
    expected, reports = [], []
    for i, (source, path) in enumerate(zip(sources, paths)):
        b = path.read_bytes()
        rows, report = observe(b, i, pixels)
        expected.extend(rows)
        report.update({'index': source['index'], 'ordinal': source['ordinal'], 'hash': source['hash'],
            'dat': source['dat'], 'offset': source['offset'], 'bytes': len(b),
            'sha256': hashlib.sha256(b).hexdigest(),
            'rows_sha256': hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest()})
        for key in ('sha256', 'rows_sha256'):
            if key in source:
                assert report[key] == source[key]
        reports.append(report)
    assert rust == '\n'.join(expected)+'\n'
    result = {'version': version, 'platform': 'Win32', 'pillow': PIL.__version__, 'samples': reports,
        'comparison': 'Exact type-4 bytes, every mip/image RGBA8 pixel and float scalar bits; coordinate/index/sequential and per-surface/full-mip decode agree.'}
    text = json.dumps(result, indent=2)+'\n'
    if 'report_sha256' in manifest:
        assert hashlib.sha256(text.encode()).hexdigest() == manifest['report_sha256']
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

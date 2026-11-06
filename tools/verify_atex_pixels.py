"""Actual AVFX-linked A8/BC2/BC7 ATEX: type-2 bytes and all mip pixels.

Args: game/sqpack new-output-directory. Uses independent struct/raw DEFLATE
and previously reviewed Pillow DDS/image oracles; no third-party FFXIV parser.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
import PIL
from find_vfx_samples import standard
from verify_avfx import fields, label, observe as observe_avfx
from verify_tex import observe


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    pin = json.loads((repo/'tests/fixtures/atex-pixel-observations.json').read_text())
    assert (root.parent/'ffxivgame.ver').read_text().strip() == pin['version']
    index = root/'ffxiv/080000.win32.index'
    with index.open('rb') as f:
        f.seek(12)
        hs, = struct.unpack('<I', f.read(4))
        f.seek(hs+8)
        offset, size = struct.unpack('<II', f.read(8))
        assert size <= 64 << 20 and offset+size <= index.stat().st_size
        f.seek(offset)
        table = list(struct.iter_unpack('<QII', f.read(size)))
    lookup = {key: (loc, i) for i, (key, loc, _) in enumerate(table)}

    def extract(source, target):
        dat, entry = (root/source['dat']).resolve(), source['offset']
        assert dat.is_relative_to(root)
        with dat.open('rb') as f:
            raw = standard(f, entry, max_bytes=16 << 20)
        assert raw is not None
        log = subprocess.check_output(['target/debug/examples/extract_entry', str(root), source['dat'], str(entry), str(target)], text=True, timeout=120)
        assert target.read_bytes() == raw
        target.with_suffix(target.suffix+'.extract.txt').write_text(log)
        return raw

    textures, owners, paths = [], [], []
    for i, source in enumerate(pin['samples']):
        owner = source['owner']
        key, loc, _ = table[owner['index_ordinal']]
        assert not loc & 1 and f'{key:016x}' == owner['index_hash']
        assert owner['dat'] == f'ffxiv/080000.win32.dat{(loc >> 1) & 7}' and owner['offset'] == (loc >> 4)*128
        owner_file = out/f'{i}.avfx'
        raw = extract(owner, owner_file)
        assert hashlib.sha256(raw).hexdigest() == owner['sha256']
        refs = [v.rstrip(b'\0') for t, v, _, _ in fields(raw[8:], 8) if label(t) == b'Tex']
        assert source['path'].encode('ascii') in refs
        expected, stats = observe_avfx(raw)
        rust = subprocess.check_output(['target/debug/examples/validate_avfx', str(owner_file)], text=True, timeout=120)
        (out/f'{i}.avfx.python.tsv').write_text(expected)
        (out/f'{i}.avfx.rust.tsv').write_text(rust)
        assert rust == expected
        owner_report = {**owner, 'rows_sha256': hashlib.sha256(expected.encode()).hexdigest(), 'statistics': stats}
        if 'rows_sha256' in owner:
            assert owner['rows_sha256'] == owner_report['rows_sha256']
        owners.append(owner_report)
        directory, filename = source['path'].rsplit('/', 1)
        jam = lambda s: ~zlib.crc32(s.encode('ascii')) & 0xffffffff
        loc, ordinal = lookup[jam(directory) << 32 | jam(filename)]
        assert not loc & 1 and source['offset'] == (loc >> 4)*128
        assert source['dat'] == f'ffxiv/080000.win32.dat{(loc >> 1) & 7}'
        path = out/f'{i}.atex'
        raw = extract(source, path)
        textures.append(raw)
        paths.append(path)
    pixels = out/'pixels'
    rust = subprocess.check_output(['target/debug/examples/validate_tex', str(pixels), *map(str, paths)], text=True, timeout=120)
    expected, reports = [], []
    for i, (raw, source) in enumerate(zip(textures, pin['samples'])):
        rows, report = observe(raw, i, pixels)
        expected.extend(rows)
        report.update({'path': source['path'], 'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw),
            'rows_sha256': hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest()})
        for key in ['sha256', 'rows_sha256']:
            if key in source:
                assert report[key] == source[key]
        reports.append(report)
    assert rust == '\n'.join(expected)+'\n'
    (out/'rust.tsv').write_text(rust)
    (out/'python.tsv').write_text('\n'.join(expected)+'\n')
    result = {'version': pin['version'], 'platform': 'Win32', 'pillow': PIL.__version__,
        'owners': owners, 'samples': reports,
        'comparison': 'Every type-2 byte, owner AVFX row, explicit Tex reference and ATEX mip/image/pixel agrees.',
        'limitations': 'A8, BC2 and BC7 real ATEX only; no float/array/volume ATEX or texture-playback claim.'}
    text = json.dumps(result, indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

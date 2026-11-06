"""Bounded readonly BC4/BC5 type-4 header discovery, not decoding evidence.

Args: game/sqpack new-report.json. At most 500000 records, 100000 evenly
selected per index, 128 indexes, 64 MiB per index table; two candidates per ID.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    indexes = sorted(root.glob('*/*.win32.index'), key=lambda p: (p.name != '040000.win32.index', p.parent.name != 'ffxiv', str(p)))
    assert len(indexes) <= 128
    remaining, reports, samples = 500000, [], []
    found = {0x6120: 0, 0x6230: 0}
    for index in indexes:
        if not remaining or all(v == 2 for v in found.values()):
            break
        with index.open('rb') as f:
            f.seek(12)
            hs, = struct.unpack('<I', f.read(4))
            assert hs <= 2 << 20
            f.seek(hs + 8)
            at, size = struct.unpack('<II', f.read(8))
            assert size <= 64 << 20 and size % 16 == 0
            f.seek(at)
            table = f.read(size)
            assert len(table) == size
        total, scanned = size // 16, 0
        selected = min(total, 100000, remaining)
        with contextlib.ExitStack() as stack:
            files = {}
            for i in range(selected):
                ordinal = i * total // selected
                key, packed, _ = struct.unpack_from('<QII', table, ordinal * 16)
                remaining -= 1
                scanned += 1
                if packed & 1:
                    continue
                dat = index.with_suffix(f'.dat{(packed >> 1) & 7}')
                if dat not in files:
                    files[dat] = stack.enter_context(dat.open('rb'))
                f = files[dat]
                entry = (packed >> 4) * 128
                f.seek(entry)
                hs, kind, target = struct.unpack('<3I', f.read(12))
                if kind != 4 or target > 4 << 20:
                    continue
                assert 128 <= hs <= 2 << 20
                f.seek(entry + hs)
                tex = f.read(80)
                assert len(tex) == 80
                flags, fmt, width, height, depth, mips, array = struct.unpack_from('<II3HBB', tex)
                if fmt not in found or found[fmt] == 2 or flags & 0x13c00000 != 0x800000:
                    continue
                samples.append({'index': str(index.relative_to(root)), 'index_ordinal': ordinal,
                    'index_hash': f'{key:016x}', 'dat': str(dat.relative_to(root)), 'offset': entry,
                    'bytes': target, 'format': hex(fmt), 'width': width, 'height': height,
                    'depth': depth, 'mips': mips & 127, 'array_size': array,
                    'header_sha256': hashlib.sha256(tex).hexdigest()})
                found[fmt] += 1
                if all(v == 2 for v in found.values()):
                    break
        reports.append({'index': str(index.relative_to(root)), 'records': total,
            'selected': selected, 'scanned': scanned, 'table_sha256': hashlib.sha256(table).hexdigest()})
    text = json.dumps({'version': (root.parent / 'ffxivgame.ver').read_text().strip(),
        'indexes': reports, 'samples': samples, 'limitations': 'Bounded header candidates, not successful reconstruction or pixel evidence.'}, indent=2) + '\n'
    with out.open('x') as f:
        f.write(text)
    print(text, end='')


if __name__ == '__main__':
    main()

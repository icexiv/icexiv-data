"""Bounded type-4 header inventory across base/expansion SqPack indexes.

Args: game/sqpack new-report.json [--all-records]. At most 2,000,000 records total,
200,000 evenly distributed records per index, 128 indexes, 64 MiB per table.
No decompression or decoding claim. Synonym records are explicitly skipped.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys


def main():
    root, out = map(Path, sys.argv[1:3])
    assert len(sys.argv) in (3, 4) and (len(sys.argv) == 3 or sys.argv[3] == '--all-records')
    all_records = len(sys.argv) == 4
    assert not out.resolve().is_relative_to(root.parent.resolve())
    assert not out.resolve().is_relative_to(Path(__file__).resolve().parent.parent)
    indexes = sorted(root.glob('*/*.win32.index'), key=lambda p: (p.parent.name != 'ffxiv', str(p)))
    assert len(indexes) <= 128
    remaining = 2_000_000
    packs, histogram, matches = [], {}, {}
    floats = {0x2140, 0x2150, 0x2250, 0x2260, 0x2350, 0x2460, 0x2470}
    for index in indexes:
        if not remaining:
            break
        with index.open('rb') as f:
            f.seek(12)
            hs, = struct.unpack('<I', f.read(4))
            assert hs <= 2 << 20
            f.seek(hs+8)
            at, size = struct.unpack('<II', f.read(8))
            assert size <= 64 << 20 and size % 16 == 0 and at+size <= index.stat().st_size
            f.seek(at)
            table = f.read(size)
        count = size // 16
        if all_records:
            assert count <= remaining, 'full selection exceeds total record budget'
            selected = count
        else:
            selected = min(count, 200_000, remaining)
        remaining -= selected
        digest = hashlib.sha256()
        record = {'index': str(index.relative_to(root)), 'index_table_sha256': hashlib.sha256(table).hexdigest(),
            'records': count, 'selected': selected, 'synonyms_skipped': 0, 'type4_headers': 0}
        with contextlib.ExitStack() as stack:
            files = {}
            for i in range(selected):
                ordinal = i*count // selected
                identity, loc, _ = struct.unpack_from('<QII', table, ordinal*16)
                digest.update(struct.pack('<IQI', ordinal, identity, loc))
                if loc & 1:
                    record['synonyms_skipped'] += 1
                    continue
                dat = index.with_suffix(f'.dat{(loc >> 1) & 7}')
                if dat not in files:
                    files[dat] = stack.enter_context(dat.open('rb'))
                f = files[dat]
                entry = (loc >> 4)*128
                f.seek(entry)
                prefix = f.read(24)
                assert len(prefix) == 24
                digest.update(prefix)
                hs, kind, target = struct.unpack_from('<III', prefix)
                if kind != 4:
                    continue
                assert 128 <= hs <= 2 << 20
                f.seek(entry+hs)
                tex = f.read(80)
                assert len(tex) == 80
                digest.update(tex)
                record['type4_headers'] += 1
                flags, fmt, w, h, d, mip_byte, array = struct.unpack_from('<II3HBB', tex)
                typ, mips = flags & 0x13c00000, mip_byte & 127
                bucket = f'{typ:08x}/{fmt:04x}/mips={mips}'
                histogram[bucket] = histogram.get(bucket, 0)+1
                labels = []
                if typ == 0x01000000:
                    labels.append('volume')
                    if mips > 1:
                        labels.append('volume-mips')
                if fmt in floats:
                    labels.append(f'float-{fmt:04x}')
                    if typ == 0x01000000:
                        labels.append('volume-float')
                    if typ == 0x10000000:
                        labels.append('array-float')
                    if typ == 0x02000000:
                        labels.append('cube-float')
                if fmt == 0x6330:
                    labels.append('bc6h')
                for label in labels:
                    saved = matches.setdefault(label, [])
                    if len(saved) >= 6 or target > 16 << 20:
                        continue
                    saved.append({'index': str(index.relative_to(root)), 'ordinal': ordinal,
                        'hash': f'{identity:016x}', 'dat': str(dat.relative_to(root)), 'offset': entry,
                        'bytes': target, 'attributes': hex(flags), 'format': hex(fmt), 'width': w,
                        'height': h, 'depth': d, 'mips': mips, 'array_size': array,
                        'header_sha256': hashlib.sha256(tex).hexdigest()})
        record['selected_headers_sha256'] = digest.hexdigest()
        packs.append(record)
    result = {'version': (root.parent / 'ffxivgame.ver').read_text().strip(), 'platform': 'Win32',
        'limits': {'total_records': 2_000_000, 'records_per_index': 200_000, 'index_count': 128},
        'index_files_found': len(indexes), 'packs': packs, 'histogram': histogram, 'matches': matches,
        'limitations': 'Evenly selected index records and type-4 headers only; no full texture decoding or absence proof.'}
    if all_records:
        assert len(packs) == len(indexes) and all(p['selected'] == p['records'] for p in packs)
        result['limits']['records_per_index'] = None
        result['limitations'] = 'All discovered primary index records, type-4 headers only in the fixed installation; no type-2 payloads, orphan DAT entries, uninstalled packages, complete pixel decoding or cross-version absence proof.'
    text = json.dumps(result, indent=2)+'\n'
    if all_records:
        pin = Path(__file__).resolve().parent.parent/'tests/fixtures/texture-full-inventory-observations.json'
        expected = json.loads(pin.read_text())
        assert hashlib.sha256(text.encode()).hexdigest() == expected['report_sha256']
        assert sum(p['selected'] for p in packs) == expected['selected_records']
        assert sum(p['type4_headers'] for p in packs) == expected['type4_headers']
    with out.open('x') as f:
        f.write(text)
    print(text, end='')


if __name__ == '__main__':
    main()

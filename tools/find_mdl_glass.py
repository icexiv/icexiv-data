"""Readonly type-3 runtime discovery for nonempty MDL Glass mesh ranges.

Args: game/sqpack new-report.json. At most 500000 selected records total,
100000/index, 256 MiB cumulative runtime output and 1 MiB/runtime. Stops at
three candidates. Header/runtime observations only, not complete model proof.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:]]
    assert not out.is_relative_to(root.parent)
    indexes = sorted(list(root.glob('*/01*.win32.index'))+list(root.glob('*/02*.win32.index')),
        key=lambda p: (p.parent.name != 'ffxiv', str(p)))
    assert len(indexes) <= 128
    records, budget = 500000, 256 << 20
    packs, found = [], []
    for index in indexes:
        if not records or not budget or len(found) == 3:
            break
        with index.open('rb') as f:
            f.seek(12)
            hs, = struct.unpack('<I', f.read(4))
            assert hs <= 2 << 20
            f.seek(hs+8)
            offset, size = struct.unpack('<II', f.read(8))
            assert size <= 64 << 20 and size % 16 == 0 and offset+size <= index.stat().st_size
            f.seek(offset)
            table = f.read(size)
        count = size // 16
        selected = min(count, 100000, records)
        records -= selected
        stats = {'index': str(index.relative_to(root)), 'records': count, 'planned': selected,
            'examined': 0, 'synonyms': 0, 'type3_entries': 0, 'runtime_observations': 0,
            'oversized_runtime': 0, 'index_table_sha256': hashlib.sha256(table).hexdigest()}
        digest = hashlib.sha256()
        with contextlib.ExitStack() as stack:
            handles = {}
            for i in range(selected):
                if not budget or len(found) == 3:
                    break
                ordinal = i*count // selected
                key, loc, _ = struct.unpack_from('<QII', table, ordinal*16)
                stats['examined'] += 1
                digest.update(struct.pack('<IQI', ordinal, key, loc))
                if loc & 1:
                    stats['synonyms'] += 1
                    continue
                dat = index.with_suffix(f'.dat{(loc >> 1) & 7}')
                if dat not in handles:
                    handles[dat] = stack.enter_context(dat.open('rb'))
                f, entry = handles[dat], (loc >> 4)*128
                f.seek(entry)
                prefix = f.read(24)
                assert len(prefix) == 24
                digest.update(prefix)
                hs, kind, target = struct.unpack_from('<III', prefix)
                if kind != 3:
                    continue
                stats['type3_entries'] += 1
                assert 208 <= hs <= 1 << 20 and target <= 64 << 20
                f.seek(entry)
                header = f.read(hs)
                capacity, = struct.unpack_from('<I', header, 28)
                if capacity > 1 << 20:
                    stats['oversized_runtime'] += 1
                    continue
                if capacity > budget:
                    budget = 0
                    break
                budget -= capacity
                counts = struct.unpack_from('<11H', header, 178)
                starts = struct.unpack_from('<11H', header, 156)
                assert 208+sum(counts)*2 <= hs
                spans = struct.unpack_from(f'<{sum(counts)}H', header, 208)
                start, = struct.unpack_from('<I', header, 116)
                position, runtime = entry+hs+start, bytearray()
                for j in range(starts[1], starts[1]+counts[1]):
                    f.seek(position)
                    bh, _, stored, expanded = struct.unpack('<4I', f.read(16))
                    n = expanded if stored == 32000 else stored
                    assert 16 <= bh <= spans[j] and n <= spans[j]-bh and expanded <= 65535
                    f.seek(position+bh)
                    data = f.read(n)
                    assert len(data) == n
                    if stored != 32000:
                        dec = zlib.decompressobj(-15)
                        data = dec.decompress(data, 65536)
                        assert dec.eof and not dec.unused_data and not dec.unconsumed_tail
                    assert len(data) == expanded and len(runtime)+expanded <= capacity
                    runtime.extend(data)
                    position += spans[j]
                assert (len(runtime)+127)//128*128 == capacity
                stats['runtime_observations'] += 1
                digest.update(hashlib.sha256(runtime).digest())
                h = 8+struct.unpack_from('<I', runtime, 4)[0]
                assert h+56 <= len(runtime)
                if not runtime[h+27] & 16:
                    continue
                elements, = struct.unpack_from('<H', runtime, h+24)
                extra = h+56+32*elements+180
                assert extra+120 <= len(runtime) and 1 <= header[204] <= 3
                ranges = [struct.unpack_from('<HH', runtime, extra+40*lod+4) for lod in range(header[204])]
                if any(count for _, count in ranges):
                    found.append({'index': str(index.relative_to(root)), 'index_ordinal': ordinal,
                        'index_hash': f'{key:016x}', 'dat': str(dat.relative_to(root)), 'offset': entry,
                        'bytes': target, 'runtime_sha256': hashlib.sha256(runtime).hexdigest(),
                        'glass_ranges': ranges})
        stats['observations_sha256'] = digest.hexdigest()
        packs.append(stats)
    result = {'version': (root.parent/'ffxivgame.ver').read_text().strip(), 'platform': 'Win32',
        'packs': packs, 'matches': found, 'remaining_runtime_budget': budget,
        'limitations': 'Selected base/expansion bgcommon/bg records and bounded runtime segments, not complete geometry, shader behavior, all MDLs or absence proof.'}
    text = json.dumps(result, indent=2)+'\n'
    pin = Path(__file__).resolve().parent.parent/'tests/fixtures/mdl-glass-discovery-observations.json'
    if pin.exists():
        expected = json.loads(pin.read_text())
        actual = {'version': result['version'], 'report_sha256': hashlib.sha256(text.encode()).hexdigest(),
            'indexes': len(packs), 'examined_records': sum(p['examined'] for p in packs),
            'runtime_observations': sum(p['runtime_observations'] for p in packs), 'matches': len(found)}
        assert actual == expected
    with out.open('x') as f:
        f.write(text)
    print(text, end='')


if __name__ == '__main__':
    main()

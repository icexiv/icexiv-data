"""Bounded type-2 TEX-header candidates in VFX indexes, not filename inference.

Args: game/sqpack new-report.json [--all-indexes]. At most 400000 evenly selected records,
100000/index, 128 indexes, 2 GiB of decompressed first-block data. Candidate
headers are not complete texture/ATEX validation or a proof of catalog absence.
--all-indexes selects all records in every discovered primary index instead of
VFX-only sampling, with a 2M total record budget and 32 GiB first-block budget.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:3]]
    assert len(sys.argv) in (3, 4) and (len(sys.argv) == 3 or sys.argv[3] == '--all-indexes')
    all_indexes = len(sys.argv) == 4
    assert not out.is_relative_to(root.parent)
    assert not out.is_relative_to(Path(__file__).resolve().parent.parent)
    indexes = sorted(root.glob('*/*.win32.index' if all_indexes else '*/08*.win32.index'), key=lambda p: (p.parent.name != 'ffxiv', str(p)))
    assert len(indexes) <= 128
    remaining, decode_budget = (2_000_000, 32 << 30) if all_indexes else (400000, 2 << 30)
    packs, histogram, matches = [], {}, {}
    formats = {0x1130, 0x1131, 0x1440, 0x1441, 0x1450, 0x1451, 0x2140, 0x2150,
        0x2250, 0x2260, 0x2350, 0x2460, 0x2470, 0x3420, 0x3430, 0x3431, 0x6120, 0x6230, 0x6330, 0x6432}
    for index in indexes:
        if not remaining or not decode_budget:
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
        if all_indexes:
            assert count <= remaining, 'full selection exceeds total record budget'
            selected = count
        else:
            selected = min(count, 100000, remaining)
        remaining -= selected
        observation = {'index': str(index.relative_to(root)), 'index_table_sha256': hashlib.sha256(table).hexdigest(),
            'records': count, 'planned_records': selected, 'examined_records': 0,
            'synonyms_skipped': 0, 'standard_first_blocks': 0, 'candidate_headers': 0}
        digest = hashlib.sha256()
        with contextlib.ExitStack() as stack:
            files = {}
            for i in range(selected):
                if not decode_budget:
                    break
                ordinal = i*count // selected
                key, packed, _ = struct.unpack_from('<QII', table, ordinal*16)
                observation['examined_records'] += 1
                digest.update(struct.pack('<IQI', ordinal, key, packed))
                if packed & 1:
                    observation['synonyms_skipped'] += 1
                    continue
                dat = index.with_suffix(f'.dat{(packed >> 1) & 7}')
                if dat not in files:
                    files[dat] = stack.enter_context(dat.open('rb'))
                f = files[dat]
                entry = (packed >> 4)*128
                f.seek(entry)
                header = f.read(32)
                assert len(header) == 32
                digest.update(header)
                hs, kind, target, _, _, blocks = struct.unpack_from('<6I', header)
                if kind != 2 or target < 80 or target > 64 << 20:
                    continue
                assert 1 <= blocks <= 65536 and 24+blocks*8 <= hs <= 2 << 20
                relative, span, expected = struct.unpack_from('<IHH', header, 24)
                f.seek(entry+hs+relative)
                block_header = f.read(16)
                bh, _, stored, expanded = struct.unpack('<4I', block_header)
                n = expanded if stored == 32000 else stored
                assert 16 <= bh <= span and n <= span-bh and expanded == expected
                if expanded > decode_budget:
                    decode_budget = 0
                    break
                decode_budget -= expanded
                f.seek(entry+hs+relative+bh)
                prefix = f.read(n)
                assert len(prefix) == n
                if stored != 32000:
                    inflater = zlib.decompressobj(-15)
                    prefix = inflater.decompress(prefix, 65536)
                    assert inflater.eof and not inflater.unused_data and not inflater.unconsumed_tail
                assert len(prefix) == expanded
                observation['standard_first_blocks'] += 1
                digest.update(hashlib.sha256(prefix).digest())
                if len(prefix) < 80:
                    continue
                flags, fmt, w, h, d, mip, array = struct.unpack_from('<II3HBB', prefix)
                typ, mips = flags & 0x13c00000, mip & 127
                if fmt not in formats or typ not in (0x800000, 0x1000000, 0x2000000, 0x10000000):
                    continue
                offsets = struct.unpack_from('<13I', prefix, 28)[:mips]
                if not 1 <= w <= 16384 or not 1 <= h <= 16384 or not 1 <= mips <= 13:
                    continue
                if not offsets or offsets[0] != 80 or not all(a < b for a, b in zip(offsets, offsets[1:]+(target,))):
                    continue
                observation['candidate_headers'] += 1
                bucket = f'{typ:08x}/{fmt:04x}'
                histogram[bucket] = histogram.get(bucket, 0)+1
                candidates = matches.setdefault(bucket, [])
                if len(candidates) < 4:
                    candidates.append({'index': str(index.relative_to(root)), 'ordinal': ordinal,
                        'hash': f'{key:016x}', 'dat': str(dat.relative_to(root)), 'offset': entry,
                        'bytes': target, 'attributes': flags, 'format': fmt, 'width': w, 'height': h,
                        'depth': d, 'mips': mips, 'array_size': array,
                        'header_sha256': hashlib.sha256(prefix[:80]).hexdigest()})
        observation['observed_headers_sha256'] = digest.hexdigest()
        packs.append(observation)
    result = {'version': (root.parent/'ffxivgame.ver').read_text().strip(), 'platform': 'Win32',
        'limits': {'total_records': 400000, 'records_per_index': 100000, 'decoded_prefix_bytes': 2 << 30},
        'remaining_decode_budget': decode_budget, 'index_files_found': len(indexes),
        'packs': packs, 'histogram': histogram, 'matches': matches,
        'limitations': 'Type-2 header candidates in selected VFX index records. No full extraction, pixel decoding, path recovery, ATEX provenance or absence claim.'}
    if all_indexes:
        assert len(packs) == len(indexes)
        assert all(p['examined_records'] == p['records'] for p in packs)
        result['limits'] = {'total_records': 2_000_000, 'records_per_index': None, 'decoded_prefix_bytes': 32 << 30}
        result['limitations'] = 'Complete discovered primary-index record selection and eligible type-2 first blocks in one installation. Candidate TEX headers only, not full extraction, pixel decoding, known ATEX filename ownership, orphan DAT entries or other installations.'
    text = json.dumps(result, indent=2)+'\n'
    pin = Path(__file__).resolve().parent.parent/'tests/fixtures'/('standard-texture-full-inventory-observations.json' if all_indexes else 'standard-texture-inventory-observations.json')
    if pin.exists():
        expected = json.loads(pin.read_text())
        assert result['version'] == expected['version']
        assert hashlib.sha256(text.encode()).hexdigest() == expected['report_sha256'], hashlib.sha256(text.encode()).hexdigest()
        assert sum(p['examined_records'] for p in packs) == expected['examined_records']
        assert sum(p['candidate_headers'] for p in packs) == expected['candidate_headers']
        if all_indexes:
            assert sum(p['standard_first_blocks'] for p in packs) == expected['standard_first_blocks']
            assert (32 << 30) - decode_budget == expected['decoded_prefix_bytes']
    with out.open('x') as f:
        f.write(text)
    print(json.dumps({k: v for k, v in result.items() if k != 'matches'}, indent=2))


if __name__ == '__main__':
    main()

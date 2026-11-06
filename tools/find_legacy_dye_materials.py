"""Bounded read-only Legacy dye MTRL discovery in Win32 character packs.

Args: game/sqpack new-output-directory. At most 500000 selected index records,
64 MiB per index table, 65535 bytes per decoded candidate, four dye files.
Anonymous first-block signatures are candidates; acceptance requires complete
independent extraction and all material rows equal to the Rust validator.
All selected 512-byte Legacy color-only datasets receive the same comparison.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib

from find_vfx_samples import standard
from verify_materials import observe_mtrl


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    indexes = sorted(root.glob('*/04*.win32.index'))
    assert len(indexes) <= 128
    remaining = 500000
    examined = candidates = 0
    samples, reports, legacy_no_dye = [], [], []
    histogram = {}
    with contextlib.ExitStack() as stack:
        files = {}
        for index in indexes:
            if not remaining or len(samples) == 4:
                break
            with index.open('rb') as f:
                f.seek(12)
                header_size, = struct.unpack('<I', f.read(4))
                assert 24 <= header_size <= 2 << 20
                f.seek(header_size + 8)
                offset, size = struct.unpack('<II', f.read(8))
                assert size <= 64 << 20 and size % 16 == 0
                f.seek(offset)
                table = f.read(size)
                assert len(table) == size
            total = len(table) // 16
            selected = min(total, remaining)
            scanned = 0
            for ordinal in (i * total // selected for i in range(selected)):
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
                header = f.read(24)
                assert len(header) == 24
                hs, kind, target, _, _, count = struct.unpack('<6I', header)
                if kind != 2 or not 16 <= target <= 65535:
                    continue
                assert 24 <= hs <= 2 << 20 and 1 <= count <= 4096 and 24 + 8 * count <= hs
                examined += 1
                f.seek(entry + 24)
                at, span, expected = struct.unpack('<IHH', f.read(8))
                f.seek(entry + hs + at)
                bh, _, stored, expanded = struct.unpack('<4I', f.read(16))
                length = expanded if stored == 32000 else stored
                assert 16 <= bh <= span and length <= span - bh and expanded == expected
                f.seek(entry + hs + at + bh)
                prefix = f.read(length)
                assert len(prefix) == length
                if stored != 32000:
                    d = zlib.decompressobj(-15)
                    prefix = d.decompress(prefix, 65536)
                    assert d.eof and not d.unused_data and not d.unconsumed_tail
                assert len(prefix) == expanded
                if len(prefix) < 16 or struct.unpack_from('<I', prefix)[0] != 0x01030000:
                    continue
                declared, dataset = struct.unpack_from('<HH', prefix, 4)
                if declared != target:
                    continue
                histogram[dataset] = histogram.get(dataset, 0) + 1
                if dataset not in (512, 544):
                    continue
                candidates += dataset == 544
                raw = standard(f, entry, max_bytes=65535)
                assert raw is not None
                rows, report, _ = observe_mtrl(raw)
                path = out / (f'{len(samples)}.mtrl' if dataset == 544 else f'legacy-{len(legacy_no_dye)}.mtrl')
                path.write_bytes(raw)
                expected_rows = '\n'.join(rows) + '\n'
                actual = subprocess.check_output(['target/debug/examples/validate_materials', str(path)], text=True, timeout=120)
                assert actual == expected_rows
                path.with_suffix('.python.tsv').write_text(expected_rows)
                path.with_suffix('.rust.tsv').write_text(actual)
                observation = {'dat': str(dat.relative_to(root)), 'offset': entry,
                    'index': str(index.relative_to(root)), 'index_ordinal': ordinal,
                    'index_hash': f'{key:016x}', 'file': path.name,
                    'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                    'rows_sha256': hashlib.sha256(actual.encode()).hexdigest(), **report}
                (samples if dataset == 544 else legacy_no_dye).append(observation)
                if len(samples) == 4:
                    break
            reports.append({'index': str(index.relative_to(root)), 'records': total,
                'selected': selected, 'scanned': scanned, 'table_sha256': hashlib.sha256(table).hexdigest()})
    result = {'version': (root.parent / 'ffxivgame.ver').read_text().strip(), 'platform': 'Win32',
        'index_record_budget': 500000, 'first_blocks_examined': examined,
        'dataset_length_candidates': histogram, 'legacy_dye_candidates': candidates,
        'indexes': reports, 'samples': samples, 'legacy_no_dye': legacy_no_dye,
        'limitations': 'Bounded character-pack selection, not all MTRLs or all game resources; no runtime dye application.'}
    text = json.dumps(result, indent=2) + '\n'
    pin = json.loads((repo / 'tests/fixtures/legacy-mtrl-discovery-observations.json').read_text())
    assert result['version'] == pin['version'] and result['platform'] == pin['platform']
    assert hashlib.sha256(text.encode()).hexdigest() == pin['report_sha256']
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

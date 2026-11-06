"""Readonly type-4 size-gap and BC6H investigation, not successful decoding.

Args: game/sqpack new-output-directory. No zero-fill or reconstructed TEX is
invented when compressed-group output does not fill the declared resource.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib


def main():
    root, out = map(Path, sys.argv[1:])
    assert not out.resolve().is_relative_to(root.parent.resolve())
    out.mkdir(parents=True, exist_ok=False)
    manifest = json.loads((Path(__file__).resolve().parent.parent / 'tests/fixtures/tex-investigation-observations.json').read_text())
    assert (root.parent / 'ffxivgame.ver').read_text().strip() == manifest['version']
    reports = []
    for sample in manifest['samples']:
        dat = (root / sample['dat']).resolve()
        assert dat.is_relative_to(root.resolve())
        entry = sample['offset']
        with dat.open('rb') as f:
            f.seek(entry)
            prefix = f.read(24)
            hs, kind, target, _, _, count = struct.unpack('<6I', prefix)
            assert kind == 4 and 128 <= hs <= 2 << 20 and target <= 16 << 20
            assert 0 < count <= 256 and 24+count*20 <= hs
            f.seek(entry)
            header = f.read(hs)
            records = list(struct.iter_unpack('<5I', header[24:24+count*20]))
            blocks = sum(r[4] for r in records)
            assert 24+count*20+blocks*2 <= hs
            spans = struct.unpack_from(f'<{blocks}H', header, 24+count*20)
            f.seek(entry+hs)
            tex = f.read(records[0][0])
            assert len(tex) == records[0][0] == 80
            assert hashlib.sha256(tex).hexdigest() == sample['header_sha256']
            mip_count = tex[14] & 127
            assert 1 <= mip_count <= 13
            offsets = list(struct.unpack_from('<13I', tex, 28))[:mip_count]
            index = 0
            groups = []
            total = len(tex)
            for offset, stored_span, declared, first, n in records:
                assert first == index and stored_span == sum(spans[index:index+n])
                position = entry+hs+offset
                decoded = bytearray()
                for _ in range(n):
                    f.seek(position)
                    size, _, stored, expanded = struct.unpack('<4I', f.read(16))
                    assert expanded <= 65535 and 16 <= size <= spans[index]
                    length = expanded if stored == 32000 else stored
                    assert length <= spans[index]-size
                    f.seek(position+size)
                    data = f.read(length)
                    assert len(data) == length
                    if stored != 32000:
                        inflater = zlib.decompressobj(-15)
                        data = inflater.decompress(data, 65536)
                        assert inflater.eof and not inflater.unused_data and not inflater.unconsumed_tail
                    assert len(data) == expanded
                    decoded.extend(data)
                    assert len(decoded) <= target
                    position += spans[index]
                    index += 1
                assert len(decoded) == declared
                total += len(decoded)
                assert total <= target
                groups.append({'declared_bytes': declared, 'decoded_bytes': len(decoded),
                    'decoded_sha256': hashlib.sha256(decoded).hexdigest()})
        command = ['target/debug/examples/extract_entry', str(root), sample['dat'], str(entry), str(out / (sample['label']+'.tex'))]
        extraction = subprocess.run(command, capture_output=True, text=True, timeout=120)
        (out / (sample['label']+'.extract.txt')).write_text(extraction.stdout+extraction.stderr)
        if sample['expected_issue'] == 'group-output-size-gap':
            assert total < target and extraction.returncode != 0
            assert 'decoded size disagrees with entry size' in extraction.stderr
            # Measure missing intervals without constructing a filled TEX.
            # These are header-relative slots, not observed runtime memory.
            assert len(groups) == mip_count
            ends = offsets[1:]+[target]
            intervals = []
            for mip, (begin, end, group) in enumerate(zip(offsets, ends, groups)):
                backed_end = begin+group['decoded_bytes']
                assert 80 <= begin <= backed_end <= end <= target
                intervals.append({'mip': mip, 'slot': [begin, end],
                    'group_bytes_if_placed_at_slot_start': [begin, backed_end],
                    'unbacked_tail': [backed_end, end]})
            assert sum(v['unbacked_tail'][1]-v['unbacked_tail'][0] for v in intervals) == target-total
        else:
            assert total == target and extraction.returncode == 0
            decoded = subprocess.run(['target/debug/examples/validate_tex', str(out / (sample['label']+'-pixels')),
                str(out / (sample['label']+'.tex'))], capture_output=True, text=True, timeout=120)
            (out / (sample['label']+'.decode.txt')).write_text(decoded.stdout+decoded.stderr)
            assert decoded.returncode == 0
            for m in range(mip_count):
                w, h = max(1, 256 >> m), max(1, 256 >> m)
                values = (out / (sample['label']+'-pixels') / f'0-{m}.rgba32f').read_bytes()
                assert values == struct.pack('<4f', 0, 0, 0, 1)*(w*h*6)
        result = {**sample, 'declared_resource_bytes': target, 'prefix_plus_decoded_bytes': total,
            'unaccounted_bytes': target-total, 'tex_mip_offsets': offsets,
            'tex_mip_spans': [b-a for a, b in zip(offsets, offsets[1:]+[target])],
            'groups': groups, 'rust_extraction_returncode': extraction.returncode}
        if sample['expected_issue'] == 'group-output-size-gap':
            result['header_slot_intervals'] = intervals
        if 'groups_sha256' in sample:
            assert hashlib.sha256(json.dumps(groups, sort_keys=True).encode()).hexdigest() == sample['groups_sha256']
        reports.append(result)
    text = json.dumps({'version': manifest['version'], 'samples': reports,
        'limitations': 'Header/group facts, unresolved volume gaps and zero-only BC6H cube; no invented gap content or nonzero real HDR claim.'}, indent=2)+'\n'
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

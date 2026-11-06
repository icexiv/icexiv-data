"""Replay pinned volume captures through SqPack -> TEX, with independent zlib pixels.

Args: authorized-executable volume-resolution-research-directory new-output-dir.
Only these captured entries and fixed native evidence are read. No index scan,
game execution, GPU, replacement TEX headers or missing-pixel generation.
"""
import csv
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    exe, research, out = map(lambda p: Path(p).resolve(), sys.argv[1:])
    repo = Path(__file__).resolve().parent.parent
    assert not any(out.is_relative_to(p) for p in [repo, exe.parent, research])
    pin = json.loads((repo / 'tests/fixtures/volume-mip-observations.json').read_bytes())
    manifest_bytes = (research / 'selected-manifest.json').read_bytes()
    assert sha(manifest_bytes) == pin['manifest_sha256']
    manifest = json.loads(manifest_bytes)
    for filename, key in [('sampler-evidence.json', 'sampler_evidence_sha256'),
                          ('path-identities.json', 'path_identities_sha256')]:
        assert sha((research / filename).read_bytes()) == pin[key]
    assert exe.stat().st_size == 51910400
    binary = exe.read_bytes()
    assert sha(binary) == pin['binary_sha256']
    # Recheck the static anchors in their PE-backed locations; no emulation.
    pe, = struct.unpack_from('<I', binary, 60)
    start = pe + 24 + struct.unpack_from('<H', binary, pe + 20)[0]
    sections = [struct.unpack_from('<4I', binary, start + 40*i + 8)
                for i in range(struct.unpack_from('<H', binary, pe + 6)[0])]
    def raw(va, length):
        rva = va - 0x140000000
        for _, section_va, size, offset in sections:
            if section_va <= rva and rva + length <= section_va + size:
                return binary[offset + rva - section_va:offset + rva - section_va + length]
        raise ValueError('unbacked static anchor')
    evidence = json.loads((research / 'sampler-evidence.json').read_bytes())
    for anchor in evidence['anchors']:
        expected = bytes.fromhex(anchor['bytes'])
        assert raw(int(anchor['va'], 0), len(expected)) == expected
    # The two stores share xmm0 when mode & 6 == 0. Initialization has min mip 0.
    mode, minimum = (0x24a >> 6) & 15, (0x24a >> 20) & 15
    assert mode & 6 == 0 and minimum == 0 and 0x24a & 0x3c0 != 0x1c0
    assert evidence['descriptor']['min_lod'] == evidence['descriptor']['max_lod'] == 0
    out.mkdir(parents=True, exist_ok=False)
    captures = research / 'captures'
    for entry in manifest['entries']:
        path = captures / f'{entry["name"]}.entry'
        assert path.resolve().parent == captures and path.stat().st_size == entry['entry_bytes']
        assert sha(path.read_bytes()) == entry['entry_sha256']
    subprocess.run([str(repo / 'target/debug/examples/validate_volume_mips'), str(captures), str(out / 'actual')],
                   cwd=repo, check=True, timeout=120)
    rows = list(csv.DictReader((out / 'actual/results.tsv').open(), delimiter='\t'))
    results = {(row['name'], int(row['mip'])): row for row in rows}
    whole_rows = list(csv.DictReader((out / 'actual/whole.tsv').open(), delimiter='\t'))
    whole = {row['name']: row for row in whole_rows}
    assert len(results) == len(rows) == 274 * 6 and len(whole) == len(whole_rows) == 274
    accepted = rejected = total_bytes = base_pixels = short_base = 0
    digest = hashlib.sha256()
    for entry in manifest['entries']:
        name = entry['name']
        data = (captures / f'{name}.entry').read_bytes()
        hs, kind, logical = struct.unpack_from('<3I', data)
        count, = struct.unpack_from('<I', data, 20)
        assert kind == 4 and count == entry['original_mip_count'] == 6
        header = data[hs:hs + 80]
        assert sha(header) == entry['header_sha256']
        assert (out / 'actual' / f'{name}.header').read_bytes() == header
        assert struct.unpack_from('<2I', header) == (0x01000000, 0x1450)
        dims = struct.unpack_from('<3H', header, 8)
        assert list(dims) == [entry[k] for k in ('width', 'height', 'depth')]
        assert logical == 80 + sum(entry['expected_bytes'])
        expected_offsets, end = [], 80
        for size in entry['expected_bytes']:
            expected_offsets.append(end)
            end += size
        assert list(struct.unpack_from('<6I', header, 28)) in [expected_offsets, [80] * 6]
        good = {row['mip']: row for row in entry['complete_mips']}
        for mip in range(count):
            relative, occupied, length, first, blocks = struct.unpack_from('<5I', data, 24 + 20*mip)
            spans = struct.unpack_from(f'<{blocks}H', data, 24 + count*20 + first*2)
            assert sum(spans) == occupied
            position, decoded = hs + relative, bytearray()
            for span in spans:
                block_header, _, stored, expected = struct.unpack_from('<4I', data, position)
                assert block_header == 16 and expected <= 65535
                size = expected if stored == 32000 else stored
                assert block_header + size <= span
                payload = data[position + block_header:position + block_header + size]
                assert len(payload) == size
                if stored == 32000:
                    output = payload
                else:
                    decoder = zlib.decompressobj(-15)
                    output = decoder.decompress(payload, expected + 1)
                    assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
                assert len(output) == expected
                decoded.extend(output)
                position += span
            assert len(decoded) == length == entry['group_bytes'][mip]
            width, height, depth = [max(1, d >> mip) for d in dims]
            required = width * height * depth * 4
            assert required == entry['expected_bytes'][mip]
            result = results[name, mip]
            dest = out / 'actual' / f'{name}-m{mip}.rgba'
            if required != length:
                assert mip not in good
                expected_error = f'MipSizeMismatch {{ index: {mip}, expected: {required}, actual: {length} }}'
                assert result['status'] == 'error' and result['detail'] == expected_error and not dest.exists()
                rejected += 1
                continue
            rgba = bytearray(decoded)
            rgba[0::4], rgba[2::4] = decoded[2::4], decoded[0::4]
            assert dest.read_bytes() == rgba and sha(rgba) == good[mip]['rgba_sha256']
            assert (dest.with_suffix('.encoded')).read_bytes() == decoded
            assert result['status'] == 'ok'
            assert result['detail'] == f'{width}x{height}x{depth};{len(rgba)};source_mips=6;source_index={mip}'
            accepted += 1
            total_bytes += len(rgba)
            digest.update(name.encode() + b'\0' + bytes([mip]) + rgba)
            if entry['status'] == 'incomplete' and mip == 0:
                short_base += 1
                base_pixels += len(rgba) // 4
        if entry['status'] == 'incomplete':
            assert whole[name]['status'] == 'error'
            assert whole[name]['detail'] == 'invalid SqPack data in capture.dat0: decoded size disagrees with entry size'
        else:
            assert whole[name]['status'] == 'ok' and whole[name]['detail'] == str(logical)
    assert accepted == pin['complete_selected_mips'] and rejected == pin['incomplete_selected_mips']
    assert short_base == pin['short_resources']
    report = {'version': pin['version'], 'manifest_sha256': sha(manifest_bytes),
              'complete_selected_mips': accepted, 'incomplete_selected_mips': rejected,
              'short_resource_base_mips': short_base, 'short_resource_base_pixels': base_pixels,
              'rgba_bytes_compared': total_bytes, 'pixels_mismatched': 0,
              'short_resources_fully_reconstructed': 0, 'whole_read_rejections': 273,
              'output_identity_sha256': digest.hexdigest(),
              'scope': 'Public SqPack selected groups and TEX complete-mip decoding, verified with independent zlib pixels; original headers retained; full-file read rejects all 273 short resources.'}
    text = json.dumps(report, indent=2) + '\n'
    (out / 'report.json').write_text(text)
    (out / 'execution.json').write_text(json.dumps({'python': sys.version,
        'tool_sha256': sha(Path(__file__).read_bytes()),
        'rust_validator_sha256': sha((repo / 'target/debug/examples/validate_volume_mips').read_bytes()),
        'report_sha256': sha(text.encode())}, indent=2) + '\n')
    if 'report_sha256' in pin: assert sha(text.encode()) == pin['report_sha256']
    print(text, end='')
    print('report_sha256:', sha(text.encode()))


if __name__ == '__main__':
    main()

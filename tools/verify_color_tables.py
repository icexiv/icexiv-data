"""Full color-table comparison against an independent mathematical/data oracle.

Args: authorized-exe suite39-directory new-output-directory.
Reuses pinned extracted inputs, never scans or executes the game. Rust performs
fixed-point conversion; Python uses exact rationals, binary32 packing, and an
ordered binary16 value set. Stain destinations come from executable data tables.
Outputs are specification-derived, not observed native uploads.
"""
import bisect
from collections import Counter
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys

from verify_dye_rows import templates


def digest(data):
    return hashlib.sha256(data).hexdigest()


def half(word):
    return struct.unpack('<e', struct.pack('<H', word))[0]


def main():
    exe, suite, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not any(out.is_relative_to(p) for p in [repo, exe.parent, suite / 'inputs', suite / 'mtrl-legacy-inventory'])
    fixtures = repo / 'tests/fixtures'
    pin_path = fixtures / 'color-table-observations.json'
    pin = json.loads(pin_path.read_text())
    conversion = json.loads((fixtures / 'client-legacy-color-observations.json').read_text())
    assert exe.stat().st_size == conversion['binary_bytes']
    binary = exe.read_bytes()
    assert digest(binary) == conversion['binary_sha256']
    pe, = struct.unpack_from('<I', binary, 60)
    sections = struct.unpack_from('<H', binary, pe + 6)[0]
    start = pe + 24 + struct.unpack_from('<H', binary, pe + 20)[0]
    sections = [struct.unpack_from('<4I', binary, start + i * 40 + 8) for i in range(sections)]

    def raw(rva, length):
        for _, va, size, offset in sections:
            if va <= rva and rva + length <= va + size:
                return binary[offset + rva - va:offset + rva - va + length]
        raise ValueError('unbacked RVA range')

    # Identity checks bind the manually interpreted control-flow facts as well
    # as the data tables; hashes alone do not prove those interpretations.
    for span in conversion['ranges'] + pin['additional_native_ranges']:
        assert digest(raw(int(span['rva'], 0), span['bytes'])) == span['sha256']
    for reference in pin['resource_references']:
        at, target = int(reference['lea_rva'], 0), int(reference['string_rva'], 0)
        instruction = raw(at, 7)
        assert instruction[:3] == bytes.fromhex('4c8d05')
        assert at + 7 + struct.unpack_from('<i', instruction, 3)[0] == target
        name = reference['path'].encode() + b'\0'
        assert raw(target, len(name)) == name
    assert raw(0x2141240, 8).hex() == '00000080000000bf'
    mapping = [list(struct.iter_unpack('<II', raw(rva, count * 8)))
               for rva, count in [(0x28aaa40, 5), (0x28aa9e0, 12)]]
    stm_paths = [suite / 'inputs' / f'stm-{i}.stm' for i in range(2)]
    for path, expected in zip(stm_paths, pin['stm_sha256']):
        assert digest(path.read_bytes()) == expected
    tables = [templates(path) for path in stm_paths]
    inventory = suite / 'mtrl-legacy-inventory'
    manifest_bytes = (inventory / 'report.json').read_bytes()
    inventory_pin = json.loads((fixtures / 'legacy-mtrl-discovery-observations.json').read_text())
    assert digest(manifest_bytes) == inventory_pin['report_sha256']
    manifest = json.loads(manifest_bytes)
    material_pin = json.loads((fixtures / 'material-observations.json').read_text())
    out.mkdir(parents=True, exist_ok=False)

    positive = [half(i) for i in range(0x7c00)]
    complements, arithmetic = [], Counter()
    for word in range(65536):
        source = half(word)
        if math.isnan(source):
            result = (word & 0x8000) | 0x7e00
            arithmetic['nan'] += 1
        elif math.isinf(source):
            result = word ^ 0x8000
            arithmetic['infinity'] += 1
        else:
            exact = Fraction(1) - Fraction(source)
            rounded, = struct.unpack('<f', struct.pack('<f', float(exact)))
            result = bisect.bisect_right(positive, abs(rounded)) - 1
            result |= 0x8000 if rounded < 0 else 0
            arithmetic['finite'] += 1
            arithmetic['binary32_rounding_changes_exact_value'] += Fraction(rounded) != exact
            if rounded:
                assert abs(rounded) >= 2**-11 and abs(rounded) <= 65505
                assert result & 0x7c00 not in (0, 0x7c00)
            else:
                arithmetic['positive_zero'] += 1
        complements.append(result)
    complement_bytes = struct.pack('<65536H', *complements)
    (out / 'complements.bin').write_bytes(complement_bytes)
    jobs, expected = bytearray(), bytearray()
    counts, writes, misses, output_digests = Counter(), Counter(), Counter(), {}
    records = []

    def check(data, stains, origin, identity):
        version, size, dataset_size, string_size, shader_offset, tx, uv, cs, ad = struct.unpack_from('<I4H4B', data)
        assert version == 0x01030000 and size == len(data) and ad >= 4
        string_start = 16 + 4 * (tx + uv + cs)
        extra = string_start + string_size
        shader_start = string_start + shader_offset
        shader = data[shader_start:data.index(0, shader_start, extra)]
        profile = int(shader != b'characterlegacy.shpk')
        flags, = struct.unpack_from('<I', data, extra)
        assert flags & 4
        modern = dataset_size in (2048, 2176)
        assert dataset_size in (512, 544, 2048, 2176)
        row_count, width = (32, 32) if modern else (16, 16)
        width_exp = (flags >> 4) & 15
        declared = (1 << ((flags >> 8) & 15), 4 << width_exp) if width_exp else (16, 16)
        assert declared == (row_count, width)
        color_bytes = row_count * width * 2
        offset = extra + ad
        source = list(struct.iter_unpack(f'<{width}H', data[offset:offset + color_bytes]))
        base = []
        for row in source:
            if modern:
                base.append(list(row))
            else:
                converted = [0] * 32
                for source_lane, destination in conversion['copied_lane_pairs']:
                    converted[destination] = row[source_lane]
                for lane in [3, 4, 5, 6, 11, 26]:
                    converted[lane] = 0x3c00
                converted[16] = 0x3800
                converted[7] = complements[row[3]]
                base.append(converted)
        base.extend([base[0][:] for _ in range(32 - row_count)])
        has_dye = dataset_size > color_bytes
        assert bool(flags & 8) == has_dye
        words = struct.unpack_from(f'<{row_count}{"I" if modern else "H"}', data, offset + color_bytes) if has_dye else []
        result, masks, missing = [r[:] for r in base], [0] * 32, 0
        for row, word in enumerate(words):
            channel = (word >> 27) & 3 if modern else 0
            eligible = word & (4095 if modern else 31)
            if channel > 1 or stains[channel] == 0 or not eligible:
                continue
            key = (word >> (16 if modern else 5)) & 2047
            if key not in tables[profile]:
                missing |= 1 << row
                continue
            capacity, columns = tables[profile][key]
            assert 1 <= stains[channel] <= capacity
            for flag, lane in mapping[profile]:
                values = columns[flag][stains[channel] - 1]
                if eligible & (1 << flag) and (flag != 1 or any(values)):
                    result[row][lane:lane + len(values)] = values
                    masks[row] |= sum(1 << i for i in range(lane, lane + len(values)))
        base_bytes = struct.pack('<1024H', *(v for row in base for v in row))
        output = struct.pack('<1024H', *(v for row in result for v in row))
        jobs.extend(struct.pack('<H2B', len(data), *stains) + data)
        expected.extend(bytes([profile]) + base_bytes + output + struct.pack('<33I', *masks, missing))
        counts[origin] += 1
        writes[origin] += sum(bool(mask) for mask in masks)
        misses[origin] += missing.bit_count()
        output_digests.setdefault(origin, hashlib.sha256()).update(output)
        records.append({'identity': identity, 'source_sha256': digest(data), 'stains': stains,
                        'origin': origin, 'profile': profile, 'base_sha256': digest(base_bytes),
                        'result_sha256': digest(output), 'missing_template_rows': missing})

    def controlled(rows, words, modern, profile):
        shader = b'character.shpk\0' if profile else b'characterlegacy.shpk\0'
        colors = struct.pack(f'<{len(rows) * len(rows[0])}H', *(v for row in rows for v in row))
        dye = struct.pack(f'<{len(words)}{"I" if modern else "H"}', *words)
        flags = (0x534 if modern else 4) | (8 if words else 0)
        body = shader + struct.pack('<I', flags) + colors + dye + bytes(12)
        return struct.pack('<I4H4B', 0x01030000, 16 + len(body), len(colors) + len(dye),
                           len(shader), 0, 0, 0, 0, 4) + body

    for sample in manifest['legacy_no_dye']:
        path = inventory / sample['file']
        assert path.resolve().parent == inventory and path.stat().st_size == sample['bytes']
        data = path.read_bytes()
        assert digest(data) == sample['sha256']
        check(data, [0, 0], 'actual_legacy_material', f'{sample["dat"]}@{sample["offset"]}')
    for index in [0, 4]:
        path = suite / 'inputs' / f'material-{index}.mtrl'
        data = path.read_bytes()
        assert digest(data) == material_pin['samples'][index]['sha256']
        for stain in range(254):
            check(data, [stain, 253 - stain], 'actual_modern_material', f'M{index}')
        check(data, [0, 0], 'actual_modern_material', f'M{index}')
    # All 65536 source half words pass through every copied source lane, too.
    for first in range(0, 65536, 16):
        rows = [[(first + row + lane * 4096) & 65535 for lane in range(16)] for row in range(16)]
        for row in range(16):
            rows[row][3] = first + row
        check(controlled(rows, [], False, 0), [0, 0], 'controlled_all_half', str(first))
    # Both source dimensions, both shader profiles, all real template IDs and
    # representative boundary stains. Declarations are synthetic, STM data real.
    for profile, table in enumerate(tables):
        for modern in [False, True]:
            rows = [[(0x7d01 + row * 73 + lane * 503) & 65535 for lane in range(32 if modern else 16)]
                    for row in range(32 if modern else 16)]
            for key in sorted(table):
                words = [((key << 16) | 4095 | ((row % 4) << 27) | 0xe000f000) if modern
                         else (key << 5) | 31 for row in range(len(rows))]
                data = controlled(rows, words, modern, profile)
                for stains in [[0, 0], [1, 253], [128, 1], [253, 0]]:
                    check(data, stains, 'controlled_dye_tables', f'{modern}:{profile}:{key}')
            # Distinct unknown ID plus no-flags records; confirmed absence.
            absent = next(i for i in range(2047, -1, -1) if i not in table)
            words = [((absent << (16 if modern else 5)) | (1 if row % 2 else 0)) for row in range(len(rows))]
            check(controlled(rows, words, modern, profile), [1, 2], 'controlled_missing_templates', f'{modern}:{profile}')

    assert len(jobs) <= 16 << 20
    (out / 'jobs.bin').write_bytes(jobs)
    (out / 'python.bin').write_bytes(expected)
    subprocess.run([str(repo / 'target/debug/examples/validate_color_tables'), *map(str, stm_paths),
                    str(out / 'jobs.bin'), str(out / 'rust.bin')], cwd=repo, check=True, timeout=120)
    actual = (out / 'rust.bin').read_bytes()
    assert len(actual) == len(expected) == 4229 * len(records)
    for offset in range(0, len(expected), 4229):
        assert actual[offset:offset + 4229] == expected[offset:offset + 4229], records[offset // 4229]
    (out / 'tables.json').write_text(json.dumps(records, indent=2) + '\n')
    report = {
        'version': conversion['version'], 'binary_sha256': conversion['binary_sha256'],
        'inventory_report_sha256': digest(manifest_bytes), 'arithmetic': dict(arithmetic),
        'complements_sha256': digest(complement_bytes), 'tables': dict(counts),
        'written_rows': dict(writes), 'missing_template_rows': dict(misses),
        'output_sha256_by_origin': {k: v.hexdigest() for k, v in output_digests.items()},
        'total_tables': sum(counts.values()), 'complete_results_bytes': len(expected),
        'jobs_sha256': digest(jobs), 'results_sha256': digest(expected),
        'limitations': 'Independent specification-derived full outputs under binary32 nearest/even then half truncation; no native execution/upload capture, no actual 544-byte Legacy dye resource, no shader pixel evaluation.',
    }
    text = json.dumps(report, indent=2) + '\n'
    (out / 'report.json').write_text(text)
    (out / 'execution.json').write_text(json.dumps({
        'python': sys.version, 'tool_sha256': digest(Path(__file__).read_bytes()),
        'stm_oracle_sha256': digest((repo / 'tools/verify_dye_rows.py').read_bytes()),
        'rust_validator_sha256': digest((repo / 'target/debug/examples/validate_color_tables').read_bytes()),
        'report_sha256': digest(text.encode()),
    }, indent=2) + '\n')
    if 'report_sha256' in pin:
        assert digest(text.encode()) == pin['report_sha256']
    print(text, end='')
    print('report_sha256:', digest(text.encode()))


if __name__ == '__main__':
    main()

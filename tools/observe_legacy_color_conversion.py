"""Observe Legacy color-conversion evidence without executing client code.

Args: authorized-ffxiv_dx11.exe pinned-legacy-inventory-directory new-output-dir.
Reuses the 322 already extracted, hash-pinned MTRLs; performs no resource scan.
The numerical experiment compares mathematical hypotheses, not native outputs.
No reconstructed color table, client emulator, shader or parsing API is produced.
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


def digest(data):
    return hashlib.sha256(data).hexdigest()


def half(word):
    return struct.unpack('<e', struct.pack('<H', word))[0]


def main():
    exe, inventory, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(exe.parent) and not out.is_relative_to(repo)
    assert not out.is_relative_to(inventory)
    fixtures = repo / 'tests/fixtures'
    pin = json.loads((fixtures / 'client-legacy-color-observations.json').read_text())
    inventory_pin = json.loads((fixtures / 'legacy-mtrl-discovery-observations.json').read_text())
    assert exe.stat().st_size == pin['binary_bytes']
    binary = exe.read_bytes()
    assert digest(binary) == pin['binary_sha256']
    manifest_path = inventory / 'report.json'
    assert manifest_path.stat().st_size <= 1 << 20
    manifest_bytes = manifest_path.read_bytes()
    assert digest(manifest_bytes) == inventory_pin['report_sha256']
    manifest = json.loads(manifest_bytes)
    assert manifest['version'] == pin['version'] and manifest['platform'] == 'Win32'
    samples = manifest['legacy_no_dye']
    assert len(samples) == inventory_pin['legacy_color_only_materials_verified']
    out.mkdir(parents=True, exist_ok=False)

    pe, = struct.unpack_from('<I', binary, 60)
    assert binary[:2] == b'MZ' and binary[pe:pe + 4] == b'PE\0\0'
    machine, count, _, _, _, size, _ = struct.unpack_from('<HHIIIHH', binary, pe + 4)
    opt = pe + 24
    assert machine == 0x8664 and count <= 96
    assert struct.unpack_from('<Q', binary, opt + 24)[0] == 0x140000000
    sections = [struct.unpack_from('<4I', binary, opt + size + i * 40 + 8) for i in range(count)]

    def raw(rva, length):
        for _, start, length_on_disk, offset in sections:
            if start <= rva and rva + length <= start + length_on_disk:
                assert offset + length_on_disk <= len(binary)
                return binary[offset + rva - start:offset + rva - start + length]
        raise ValueError('RVA range has no complete file backing')

    for span in pin['ranges']:
        start = int(span['rva'], 0)
        assert 0 < span['bytes'] <= 65536
        assert digest(raw(start, span['bytes'])) == span['sha256']
        asm = subprocess.check_output([
            'objdump', '-d', '-Mintel', f'--start-address={0x140000000 + start:#x}',
            f'--stop-address={0x140000000 + start + span["bytes"]:#x}', str(exe),
        ], text=True, timeout=30)
        (out / (span['label'] + '.asm')).write_text(asm)
    for call in pin['calls']:
        start = int(call['rva'], 0)
        ins = raw(start, 5)
        assert ins[0] == 0xe8
        assert start + 5 + struct.unpack_from('<i', ins, 1)[0] == int(call['target'], 0)
    for item in pin['constants']:
        start = int(item['load_rva'], 0)
        ins = raw(start, 8)
        assert ins[:4] == bytes.fromhex(item['load_prefix'])
        target = start + 8 + struct.unpack_from('<i', ins, 4)[0]
        assert target == int(item['value_rva'], 0)
        assert raw(target, 4).hex() == item['f32_le_hex']

    # Enumerate representable values and choose by order/distance. This does not
    # translate the client's mantissa/exponent instructions into a converter.
    positive_half = [half(word) for word in range(0x7c00)]
    assert all(a < b for a, b in zip(positive_half, positive_half[1:]))

    def toward_zero(value):
        assert math.isfinite(value) and abs(value) <= positive_half[-1]
        word = bisect.bisect_right(positive_half, abs(value)) - 1
        return word | (0x8000 if math.copysign(1, value) < 0 else 0)

    def nearest_even(value):
        magnitude = abs(value)
        low = bisect.bisect_right(positive_half, magnitude) - 1
        high = min(low + 1, len(positive_half) - 1)
        word = min((low, high), key=lambda w: (abs(positive_half[w] - magnitude), w % 2))
        word |= 0x8000 if math.copysign(1, value) < 0 else 0
        assert struct.unpack('<H', struct.pack('<e', value))[0] == word
        return word

    controlled = []
    controlled_witnesses = []
    controlled_differences = 0
    for source in range(0x3c01):
        # Every binary16 x in [0,1] has exact binary32 1-x. Fraction zero
        # is recorded as +0; this does not predict native signed-zero behavior.
        exact = Fraction(1) - Fraction(half(source))
        value, = struct.unpack('<f', struct.pack('<f', float(exact)))
        assert Fraction(value) == exact
        truncation, rounding = toward_zero(value), nearest_even(value)
        controlled.append(f'{source:04x}\t{struct.pack("<f", value).hex()}\t{truncation:04x}\t{rounding:04x}\n')
        if truncation != rounding:
            controlled_differences += 1
            if len(controlled_witnesses) < 4:
                controlled_witnesses.append({'source_half': f'{source:04x}',
                    'exact_one_minus_source': str(exact),
                    'toward_zero_half': f'{truncation:04x}', 'nearest_even_half': f'{rounding:04x}'})
    controlled_text = ''.join(controlled)
    (out / 'quantization-hypotheses.tsv').write_text(controlled_text)

    source_lanes = [source for source, _ in pin['copied_lane_pairs']]
    assert len(source_lanes) == len(set(source_lanes)) == 14
    hist = Counter()
    row_lines, witness_rows = [], []
    projection_witnesses = []
    negative = different_rounding = changed_lane3 = unequal_specular = 0
    pair_rows = pair_materials = padding_rows = padding_materials = 0
    identity_digest = hashlib.sha256()
    for sample in samples:
        path = (inventory / sample['file']).resolve()
        assert path.parent == inventory and path.stat().st_size == sample['bytes'] <= 65535
        data = path.read_bytes()
        assert digest(data) == sample['sha256']
        assert struct.unpack_from('<I', data)[0] == 0x01030000
        declared, dataset_bytes, string_bytes, shader_offset = struct.unpack_from('<4H', data, 4)
        assert declared == len(data) and dataset_bytes == 512
        strings = 16 + 4 * sum(data[12:15])
        additional = strings + string_bytes
        dataset = additional + data[15]
        assert data[15] >= 4 and dataset + 512 <= len(data)
        flags, = struct.unpack_from('<I', data, additional)
        assert flags & 4 and not flags & 8
        width = (flags >> 4) & 15
        assert width == 0 or (4 * (1 << width), 1 << ((flags >> 8) & 15)) == (16, 16)
        shader_start = strings + shader_offset
        assert strings <= shader_start < additional
        shader_end = data.index(0, shader_start, additional)
        assert data[shader_start:shader_end].decode('ascii') == sample['shader']
        rows = list(struct.iter_unpack('<16H', data[dataset:dataset + 512]))
        identity = f'{sample["dat"]}@{sample["offset"]}'
        identity_digest.update(identity.encode() + b'\0' + bytes.fromhex(sample['sha256']))
        # Compare copied-lane projections only: these are raw source words, not
        # synthesized native output rows or a full Legacy conversion oracle.
        projections = [tuple(row[lane] for lane in source_lanes) for row in rows]
        pair_count = sum(projections[i if i < 16 else 0] != projections[i // 2] for i in range(32))
        padding_count = 16 * (projections[0] != tuple(0 for _ in source_lanes))
        pair_rows += pair_count
        pair_materials += pair_count != 0
        padding_rows += padding_count
        padding_materials += padding_count != 0
        if pair_count and len(projection_witnesses) < 4:
            destination_row = next(i for i in range(32)
                if projections[i if i < 16 else 0] != projections[i // 2])
            observed_source_row = destination_row if destination_row < 16 else 0
            candidate_source_row = destination_row // 2
            lane = next(lane for lane in source_lanes
                if rows[observed_source_row][lane] != rows[candidate_source_row][lane])
            projection_witnesses.append({'dat': sample['dat'], 'offset': sample['offset'],
                'sha256': sample['sha256'], 'destination_row': destination_row,
                'statically_selected_source_row': observed_source_row,
                'pair_duplication_source_row': candidate_source_row, 'source_lane': lane,
                'statically_selected_source_half': f'{rows[observed_source_row][lane]:04x}',
                'pair_duplication_source_half': f'{rows[candidate_source_row][lane]:04x}'})
        for row_index, row in enumerate(rows):
            value = half(row[3])
            assert math.isfinite(value)
            hist[row[3]] += 1
            exact = Fraction(1) - Fraction(value)
            number, = struct.unpack('<f', struct.pack('<f', float(exact)))
            assert Fraction(number) == exact
            truncated, rounded = toward_zero(number), nearest_even(number)
            negative += number < 0
            different_rounding += truncated != rounded
            changed_lane3 += truncated != row[3]
            unequal_specular += row[4:7] != (0x3c00, 0x3c00, 0x3c00)
            row_lines.append(f'{identity}\t{row_index}\t{row[3]:04x}\t{truncated:04x}\t{rounded:04x}\n')
            if value > 1 or 0 < value < 1:
                witness_rows.append({'dat': sample['dat'], 'offset': sample['offset'],
                    'sha256': sample['sha256'], 'shader': sample['shader'], 'row': row_index,
                    'source_lane3_half': f'{row[3]:04x}', 'source_lane3_value': value,
                    'one_minus_value': number, 'toward_zero_half': f'{truncated:04x}'})
    row_text = ''.join(row_lines)
    (out / 'actual-source-lane3-hypotheses.tsv').write_text(row_text)
    result = {
        'version': pin['version'], 'platform': 'Win32 resources / Win64 executable',
        'binary_sha256': pin['binary_sha256'], 'inventory_report_sha256': digest(manifest_bytes),
        'sample_identity_sha256': identity_digest.hexdigest(),
        'static_ranges': len(pin['ranges']), 'direct_calls': len(pin['calls']),
        'actual_materials': len(samples), 'actual_rows': len(row_lines),
        'source_lane3_histogram': [{'half': f'{word:04x}', 'value': half(word), 'rows': count}
            for word, count in sorted(hist.items())],
        'actual_input_hypothesis_comparison': {
            'one_minus_negative_rows': negative,
            'one_minus_vs_direct_copy_different_rows': changed_lane3,
            'toward_zero_vs_nearest_even_different_rows': different_rounding,
            'source_specular_vs_constant_one_different_rows': unequal_specular,
            'pair_duplication_copied_projection_different_rows': pair_rows,
            'pair_duplication_copied_projection_different_materials': pair_materials,
            'zero_padding_copied_projection_different_rows': padding_rows,
            'zero_padding_copied_projection_different_materials': padding_materials,
        },
        'actual_source_hypotheses_sha256': digest(row_text.encode()),
        'witnesses': witness_rows,
        'pair_duplication_projection_witnesses': projection_witnesses,
        'controlled_quantization': {'inputs': len(controlled),
            'different_results': controlled_differences,
            'sha256': digest(controlled_text.encode()), 'witnesses': controlled_witnesses,
            'domain': 'Binary16 words 0x0000..0x3c00; exact binary32 1-x. Rational zero is recorded as +0; native zero sign can depend on floating-point rounding state. Mathematical quantization hypotheses, no target execution.'},
        'limitations': 'Static selected-path semantics and consequences on pinned source rows; no observed native converted rows, no complete shader/dye routing or reset policy, no real 544-byte dye material, no native execution or emulation.',
    }
    text = json.dumps(result, indent=2) + '\n'
    (out / 'report.json').write_text(text)
    execution = {'python': sys.version, 'objdump': subprocess.check_output(
        ['objdump', '--version'], text=True, timeout=30).splitlines()[0],
        'tool_sha256': digest(Path(__file__).read_bytes()),
        'fixture_sha256': digest((fixtures / 'client-legacy-color-observations.json').read_bytes()),
        'report_sha256': digest(text.encode())}
    (out / 'execution.json').write_text(json.dumps(execution, indent=2) + '\n')
    if 'report_sha256' in pin:
        assert digest(text.encode()) == pin['report_sha256']
    print(json.dumps({'report_sha256': digest(text.encode()),
        'actual_materials': len(samples), 'actual_rows': len(row_lines),
        'actual_input_hypothesis_comparison': result['actual_input_hypothesis_comparison'],
        'controlled_inputs': len(controlled), 'controlled_differences': controlled_differences}, indent=2))


if __name__ == '__main__':
    main()

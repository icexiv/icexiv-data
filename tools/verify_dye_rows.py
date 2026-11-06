"""Independent raw-half STM row application checks. No native execution.

Args: authorized-exe legacy.stm dawntrail.stm weapon.mtrl equipment.mtrl new-out.
STM columns are independently decoded; destination ranges are read from the
fixed executable's format tables, not from Rust constants. Output comparison
tests explicit row operations, not client fallback or Legacy color conversion.
"""
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def templates(path):
    assert path.stat().st_size <= 16 << 20
    b = path.read_bytes()
    assert len(b) <= 16 << 20 and b[:2] == b'MS'
    version, count = struct.unpack_from('<HH', b, 2)
    assert count <= 4096
    columns = {0x101: 5, 0x201: 12}[version]
    result = {}
    for i in range(count):
        key, = struct.unpack_from('<I', b, 8 + i * 4)
        offset, = struct.unpack_from('<I', b, 8 + count * 4 + i * 4)
        start = 8 + count * 8 + offset * 2
        ends = struct.unpack_from(f'<{columns}H', b, start)
        previous, decoded, capacity = 0, [], 254
        for c, end in enumerate(ends):
            raw = b[start + columns * 2 + previous * 2:start + columns * 2 + end * 2]
            previous = end
            width = 3 if c < 3 else 1
            stride = width * 2
            if not raw: values = [(0,) * width] * 254
            elif len(raw) == stride: values = [struct.unpack(f'<{width}H', raw)] * 254
            elif len(raw) == stride * 254: values = list(struct.iter_unpack(f'<{width}H', raw))
            else:
                dictionary = [(0,) * width] + list(struct.iter_unpack(f'<{width}H', raw[:-254]))
                assert raw[-254] == 255
                values = [dictionary[j] for j in raw[-253:]]
                capacity = 253
            decoded.append(values)
        assert key not in result
        result[key] = (capacity, decoded)
    return result


def material(path):
    assert path.stat().st_size <= 65535
    b = path.read_bytes()
    _, _, size, ss, _, tx, uv, cs, ad = struct.unpack_from('<I4H4B', b)
    assert size == 2176
    start = 16 + (tx + uv + cs) * 4 + ss + ad
    colors = list(struct.iter_unpack('<32H', b[start:start + 2048]))
    words = list(struct.unpack_from('<32I', b, start + 2048))
    return list(zip(colors, words))


def main():
    exe, legacy, modern, weapon, equipment, out = map(Path, sys.argv[1:])
    repo = Path(__file__).resolve().parent.parent
    assert not out.resolve().is_relative_to(repo) and not out.resolve().is_relative_to(exe.resolve().parent)
    assert exe.stat().st_size <= 128 << 20
    binary = exe.read_bytes()
    # Use the independently fixed executable identity, not a path-only match.
    known = json.loads((repo/'tests/fixtures/client-mtrl-observations.json').read_text())
    assert hashlib.sha256(binary).hexdigest() == known['binary_sha256']
    # These tables are in .data: RVA 0x289f000 -> file 0x289d000.
    mapping = []
    for rva, n in [(0x28aaa40, 5), (0x28aa9e0, 12)]:
        at = rva - 0x2000
        fields = list(struct.iter_unpack('<II', binary[at:at + n * 8]))
        assert sorted(flag for flag, _ in fields) == list(range(n))
        mapping.append(fields)
    tables = [templates(legacy), templates(modern)]
    out.mkdir(parents=True, exist_ok=False)
    jobs, expected = bytearray(), bytearray()
    counts = Counter()
    origins = {'material': Counter(), 'controlled': Counter()}
    origin = 'material'
    specular_zero = 0

    def check(base, raw, profile, stain, channel=255, source=1):
        nonlocal specular_zero
        jobs.extend(struct.pack('<4BIHH32H', source, profile, channel, 0, raw, stain, 0, *base))
        result = list(base)
        status, mask = 0, 0
        identity = (raw >> 16) & 2047 if source else (raw >> 5) & 2047
        flags = raw & (4095 if source else 31)
        if channel != 255 and channel > 3: status = 5
        elif channel != 255 and not source: status = 4
        elif channel != 255 and (raw >> 27) & 3 != channel: status = 1
        elif identity not in tables[profile]: status = 2
        else:
            capacity, columns = tables[profile][identity]
            if not 1 <= stain <= capacity: status = 3
            else:
                for flag, lane in mapping[profile]:
                    if not flags & (1 << flag): continue
                    values = columns[flag][stain - 1]
                    if flag == 1 and not any(values):
                        specular_zero += 1
                        continue
                    # Simultaneous replacement of independently observed ranges.
                    result[lane:lane + len(values)] = values
                    mask |= sum(1 << j for j in range(lane, lane + len(values)))
        counts[str(status)] += 1
        origins[origin][str(status)] += 1
        expected.extend(struct.pack('<BI32H', status, mask, *result))

    materials = [material(weapon), material(equipment)]
    real_requests = 0
    for rows in materials:
        for base, raw in rows:
            for profile in range(2):
                for stain in range(1, 254):
                    check(base, raw, profile, stain)
                    real_requests += 1
                channel = (raw >> 27) & 3
                check(base, raw, profile, 1, channel)
                check(base, raw, profile, 0, (channel + 1) % 4)
    # Every actual template/stain, with controlled all-flags declarations and
    # bit-rich intermediate rows; these are operations on real columns, not
    # claims that these declarations occur in production MTRLs.
    controlled = 0
    origin = 'controlled'
    base = tuple((0x7d00 + i * 71) & 65535 for i in range(32))
    for profile, table in enumerate(tables):
        for key, (capacity, _) in table.items():
            assert key <= 2047
            for stain in range(1, capacity + 1):
                check(base, (key << 16) | 4095, profile, stain)
                controlled += 1
        key = min(table)
        for flags in range(4096):
            check(base, (key << 16) | flags | 0xe000f000, profile, 1)
            controlled += 1
        for source in range(2):
            raw = (key << (16 if source else 5)) | (4095 if source else 31)
            for stain in [0, 254, 255, 65535]:
                check(base, raw, profile, stain, source=source)
            check(base, raw, profile, 1, 0, source=source)
            check(base, raw, profile, 1, 4, source=source)
            for stain in [1, 253]:
                check(base, raw, profile, stain, source=source)
        check(base, (2047 << 16) | 4095, profile, 1)
    (out/'jobs.bin').write_bytes(jobs)
    subprocess.run(['target/debug/examples/validate_dye_rows', str(legacy), str(modern),
        str(out/'jobs.bin'), str(out/'rust.bin')], check=True, timeout=120)
    (out/'python.bin').write_bytes(expected)
    assert (out/'rust.bin').read_bytes() == expected
    report = {'platform': 'Win32', 'version': '2026.10.06.0000.0000',
        'binary_sha256': known['binary_sha256'],
        'input_sha256': [hashlib.sha256(p.read_bytes()).hexdigest() for p in [legacy, modern, weapon, equipment]],
        'real_material_rows': 64, 'real_row_stain_profile_requests': real_requests,
        'controlled_template_and_flag_requests': controlled, 'total_requests': len(jobs)//76,
        'statuses': dict(sorted(counts.items())), 'all_zero_specular_preserved': specular_zero,
        'origin_statuses': {key: dict(sorted(value.items())) for key, value in origins.items()},
        'jobs_sha256': hashlib.sha256(jobs).hexdigest(),
        'results_sha256': hashlib.sha256(expected).hexdigest(),
        'scope': 'Explicit immutable 32-half intermediate row operations, both profiles, all observed STM templates/stains and controlled declarations; not native execution, shader routing, Legacy conversion, stain-zero/default policy or full material playback.'}
    text = json.dumps(report, indent=2)+'\n'
    fixture = json.loads((repo/'tests/fixtures/dye-row-observations.json').read_text())
    digest = hashlib.sha256(text.encode()).hexdigest()
    assert digest == fixture['report_sha256'], digest
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__': main()

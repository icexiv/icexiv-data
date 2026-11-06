"""Compare declaration masks with pinned native field metadata, not dye execution.

Args: authorized-ffxiv_dx11.exe weapon.mtrl equipment.mtrl new-output-directory.
All flag subsets are controlled words; actual MTRL rows retain their provenance.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    exe, first, second, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(exe.parent) and not out.is_relative_to(repo)
    pin = json.loads((repo/'tests/fixtures/dye-lane-observations.json').read_text())
    native = json.loads((repo/'tests/fixtures/client-mtrl-observations.json').read_text())
    assert exe.stat().st_size <= 128 << 20
    binary = exe.read_bytes()
    assert hashlib.sha256(binary).hexdigest() == native['binary_sha256']
    # Fixed .data/.rdata file mappings of the SHA-bound PE; the static verifier
    # separately checks the consuming code and its references to these locations.
    metadata = {}
    for name, rva, count in [('legacy',0x28aaa40,5),('dawntrail',0x28aa9e0,12)]:
        metadata[name] = list(struct.iter_unpack('<II', binary[rva-0x2000:rva-0x2000+count*8]))
    widths = struct.unpack_from('<2Q', binary, 0x22dcfd0-0x1800)
    assert widths == (3, 1)
    for name, pairs in metadata.items():
        assert [flag for flag, _ in pairs] == list(range(5 if name=='legacy'else 12))
        assert all(lane+(widths[0] if flag < 3 else widths[1]) <= 32 for flag, lane in pairs)

    def mask(word, profile):
        lanes = set()
        for flag, lane in metadata[profile]:
            if word // (2**flag) % 2:
                lanes.update(range(lane, lane+widths[0 if flag < 3 else 1]))
        return sum(2**lane for lane in lanes)

    rows = []
    for flags in range(4096):
        for extra in (0, 0xe000f000, 0x1fff0000):
            raw = flags | extra
            for profile in metadata:
                rows.append(f'control\t{profile}\t{raw:08x}\t{mask(raw,profile):08x}')
    for raw in range(65536):
        for profile in metadata:
            rows.append(f'legacy_control\t{profile}\t{raw:08x}\t{mask(raw%32,profile):08x}')
    sources = []
    for file, path in enumerate((first, second)):
        assert path.stat().st_size <= 65535
        b = path.read_bytes()
        digest = hashlib.sha256(b).hexdigest()
        assert digest == pin['sources'][file]
        assert struct.unpack_from('<H', b, 6)[0] == 2176
        start = 16+4*sum(b[12:15])+struct.unpack_from('<H', b, 8)[0]+b[15]
        assert start+2176 <= len(b)
        words = struct.unpack_from('<32I', b, start+2048)
        for row, word in enumerate(words):
            for profile in metadata:
                rows.append(f'file\t{file}\t{row}\t{profile}\t{word:08x}\t{mask(word,profile):08x}')
        sources.append({'sha256': digest, 'dye_rows': len(words),
            'masks_sha256': {profile:hashlib.sha256(struct.pack('<32I',*[mask(w,profile)for w in words])).hexdigest()for profile in metadata}})
    expected = '\n'.join(rows)+'\n'
    actual = subprocess.check_output(['target/debug/examples/validate_dye_lanes', str(first), str(second)], text=True, timeout=120)
    assert actual == expected
    result = {'binary_sha256': native['binary_sha256'], 'sources': sources,
        'native_flag_lane_pairs': metadata, 'native_rgb_scalar_widths': widths,
        'controlled_modern_words': 4096*3, 'controlled_legacy_words':65536, 'profiles_per_word':2, 'actual_dye_rows': 64,
        'rows_sha256': hashlib.sha256(expected.encode()).hexdigest(),
        'limitations': 'Intermediate-row declaration eligibility with explicitly selected destination profiles, not source file-row masks. Full flag subsets are controlled inputs; real rows are two pinned Dawntrail-table materials, including a characterlegacy shader material. No native execution, Legacy color conversion, STM write/application or shader/channel-state selection.'}
    text = json.dumps(result, indent=2)+'\n'
    if 'report_sha256' in pin:
        assert hashlib.sha256(text.encode()).hexdigest() == pin['report_sha256']
    out.mkdir(parents=True, exist_ok=False)
    (out/'rows.rust.tsv').write_text(actual)
    (out/'rows.python.tsv').write_text(expected)
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

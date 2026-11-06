"""Signed BC6H mode corpus against pinned independent Microsoft CPU results.

Args: new-output-directory [explicit-DirectX-oracle-executable]. Without the
oracle, compare Rust with its pinned per-mode output digests. With it, regenerate
and compare every float bit. No GPU, game execution, or FFXIV parser oracle.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


MODES = [0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 3, 7, 11, 15, 19, 23, 27, 31]


def corpus(code):
    blocks = []
    for sample in range(256):
        seed = b'icexiv-bc6h-modes-v1'+struct.pack('<II', code, sample)
        value = int.from_bytes(hashlib.sha256(seed).digest()[:16], 'little')
        if sample in (0, 1):
            value = 0 if sample == 0 else (1 << 128)-1
        bits = 2 if code in (0, 1) else 5
        value = value & ~((1 << bits)-1) | code
        if code in MODES[:10]:
            # Official two-region shape field: bits 77..81, LSB numbering.
            value = value & ~(31 << 77) | (sample % 32) << 77
        blocks.append(value.to_bytes(16, 'little'))
    return b''.join(blocks)


def main():
    out = Path(sys.argv[1]).resolve()
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    pin_path = repo/'tests/fixtures/bc6h-mode-observations.json'
    pin = json.loads(pin_path.read_text()) if pin_path.exists() else None
    oracle = Path(sys.argv[2]).resolve() if len(sys.argv) == 3 else None
    assert oracle is not None or pin is not None
    results = []
    for code in MODES:
        blocks = corpus(code)
        header = bytearray(80)
        struct.pack_into('<II3HBB', header, 0, 0x800000, 0x6330, 4, 1024, 1, 1, 0)
        struct.pack_into('<I', header, 28, 80)
        tex = out/f'{code:02}.tex'
        tex.write_bytes(header+blocks)
        pixels = out/f'{code:02}-pixels'
        log = subprocess.check_output(['target/debug/examples/validate_tex', str(pixels), str(tex)], text=True, timeout=120)
        (out/f'{code:02}.rust.tsv').write_text(log)
        actual = (pixels/'0-0.rgba32f').read_bytes()
        assert len(actual) == 256*16*16
        if oracle is not None:
            expected = subprocess.check_output([str(oracle)], input=blocks, timeout=120)
            (out/f'{code:02}.oracle.rgba32f').write_bytes(expected)
            assert actual == expected, f'mode prefix {code}: float bits disagree'
        values = struct.unpack(f'<{len(actual)//4}f', actual)
        result = {'prefix': code, 'blocks': 256,
            'block_sha256': hashlib.sha256(blocks).hexdigest(),
            'rgba32f_sha256': hashlib.sha256(actual).hexdigest(),
            'negative_values': sum(v < 0 for v in values), 'above_one_values': sum(v > 1 for v in values)}
        if pin:
            assert result == pin['modes'][len(results)]
        results.append(result)
    evidence = {'modes': results, 'total_blocks': 4608, 'float_components': 294912}
    if pin:
        assert all(evidence[k] == pin[k] for k in evidence)
    evidence['comparison'] = 'DirectX CPU oracle exact bytes' if oracle else 'Pinned DirectX CPU oracle output digests'
    evidence['limitations'] = 'Controlled signed-block corpus, not real nonzero game textures or exhaustive 128-bit coverage. Reserved prefixes are black. No unsigned TEX claim.'
    text = json.dumps(evidence, indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

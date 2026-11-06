"""R16F/unsigned R11G11B10F native ID facts and independent CPU controls.

Args: authorized-ffxiv_dx11.exe new-output-directory [explicit-float-oracle].
The optional Microsoft DirectXMath executable regenerates all expected values;
without it, verify pinned independent result digests. No target/GPU execution.
Finite values, signed zeros and infinities compare exact bits; NaNs compare
classification, with payload/sign explicitly outside the half-format API.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_tex import quantize, pixels as wire_pixels


def canonical(data):
    output, nan = bytearray(), 0
    for word, in struct.iter_unpack('<I', data):
        if word & 0x7f800000 == 0x7f800000 and word & 0x7fffff:
            word = 0x7fc00000
            nan += 1
        output.extend(struct.pack('<I', word))
    return bytes(output), nan


def pools():
    half = b''.join(struct.pack('<H', v) for v in range(65536))
    words = [i | ((i*73 & 2047) << 11) | ((i & 1023) << 22) for i in range(2048)]
    for i in range(4096):
        words.append(int.from_bytes(hashlib.sha256(b'icexiv-packed-float-v1'+struct.pack('<I', i)).digest()[:4], 'little'))
    return {0x2140: half, 0x2350: b''.join(struct.pack('<I', w) for w in words)}


def controls(fmt, source, kind, dims, images, levels, sequence):
    stride = 2 if fmt == 0x2140 else 4
    header, payloads = bytearray(80), []
    struct.pack_into('<II3HBB', header, 0, kind, fmt, *dims, levels, images)
    offset = 80
    for mip in range(levels):
        w, h = max(1, dims[0] >> mip), max(1, dims[1] >> mip)
        count = images if kind == 0x10000000 else (6 if kind == 0x2000000 else (max(1, dims[2] >> mip) if kind == 0x1000000 else 1))
        if sequence == 0:
            payload = source
            assert len(payload) == w*h*count*stride
        else:
            indices = [(sequence*131+j*7919+mip*32749) % (len(source)//stride) for j in range(w*h*count)]
            payload = b''.join(source[i*stride:(i+1)*stride] for i in indices)
        struct.pack_into('<I', header, 28+4*mip, offset)
        offset += len(payload)
        payloads.append(payload)
    return bytes(header)+b''.join(payloads), payloads


def main():
    exe, out = [Path(p).resolve() for p in sys.argv[1:3]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(exe.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    fixtures = repo/'tests/fixtures'
    native = json.loads((fixtures/'bc6h-observations.json').read_text())
    assert exe.stat().st_size <= 128 << 20
    binary = exe.read_bytes()
    assert hashlib.sha256(binary).hexdigest() == native['binary_sha256']
    rva, size = native['format_function_rva'], native['format_function_bytes']
    assert hashlib.sha256(binary[rva-0xc00:rva-0xc00+size]).hexdigest() == native['format_function_sha256']
    for at, value in [(0x2181b4, '81e950140000'), (0x2181bc, '83e901'),
        (0x2181c1, '81e9ef0c0000'), (0x2181dc, '41b836000000'),
        (0x218202, '81f9502300007416'), (0x218220, '41b81a000000')]:
        value = bytes.fromhex(value)
        assert binary[at-0xc00:at-0xc00+len(value)] == value
    assert 0x1450+1+0xcef == 0x2140
    pin_file = fixtures/'float-texture-control-observations.json'
    pin = json.loads(pin_file.read_text()) if pin_file.exists() else None
    oracle = Path(sys.argv[3]).resolve() if len(sys.argv) == 4 else None
    assert oracle is not None or pin is not None
    reports = []
    for fmt, source in pools().items():
        base = (256,256,1) if fmt == 0x2140 else (128,48,1)
        profiles = [(0x800000, base, 0, 1), (0x10000000,(7,5,1),3,3),
            (0x2000000,(7,7,1),6,3), (0x1000000,(7,5,3),0,3)]
        for sequence, profile in enumerate(profiles):
            number = len(reports)
            raw, payloads = controls(fmt, source, *profile, sequence)
            path, pixels = out/f'{number}.tex', out/f'{number}-pixels'
            path.write_bytes(raw)
            log = subprocess.check_output(['target/debug/examples/validate_tex', str(pixels), str(path)], text=True, timeout=120)
            (out/f'{number}.rust.tsv').write_text(log)
            floats, rgba, nan_count, scalar_count = hashlib.sha256(), hashlib.sha256(), 0, 0
            for mip, payload in enumerate(payloads):
                actual = (pixels/f'0-{mip}.rgba32f').read_bytes()
                normalized, nan = canonical(actual)
                python_rgba, python_float = wire_pixels(payload, len(payload)//(2 if fmt == 0x2140 else 4), 1, fmt)
                assert canonical(python_float) == (normalized, nan)
                assert (pixels/f'0-{mip}.rgba8').read_bytes() == python_rgba
                if oracle:
                    expected = subprocess.check_output([str(oracle), 'h' if fmt == 0x2140 else 'p'], input=payload, timeout=120)
                    (out/f'{number}-{mip}.oracle.rgba32f').write_bytes(expected)
                    expected, expected_nan = canonical(expected)
                    assert normalized == expected and nan == expected_nan
                    values = [v for v, in struct.iter_unpack('<f', expected)]
                    assert (pixels/f'0-{mip}.rgba8').read_bytes() == bytes(map(quantize, values))
                floats.update(normalized)
                rgba.update((pixels/f'0-{mip}.rgba8').read_bytes())
                nan_count += nan
                scalar_count += len(actual)//4
            report = {'format': hex(fmt), 'kind': hex(profile[0]), 'dimensions': list(profile[1]),
                'images_byte': profile[2], 'mips': profile[3], 'tex_sha256': hashlib.sha256(raw).hexdigest(),
                'canonical_float_sha256': floats.hexdigest(), 'rgba8_sha256': rgba.hexdigest(),
                'scalar_results': scalar_count, 'nan_results': nan_count}
            if pin:
                assert report == pin['samples'][number]
            reports.append(report)
    evidence = {'binary_sha256': native['binary_sha256'], 'native_dxgi': {'0x2140':54,'0x2350':26},
        'samples': reports, 'scalar_results': sum(s['scalar_results'] for s in reports),
        'nan_results': sum(s['nan_results'] for s in reports)}
    if pin:
        assert evidence == pin
    result = {**evidence, 'comparison': 'DirectXMath and Python numeric oracles' if oracle else 'Python numeric oracle and pinned DirectXMath result digests',
        'limitations': 'Controlled TEX layouts, not real R16F/R11G11B10F/ATEX samples. Exact finite/zero/infinity bits and NaN classification; no NaN payload/sign guarantee.'}
    text = json.dumps(result, indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

"""BC6H_SF16 native mapping, real zero cube, independent mode-11 experiments.

Args: game/sqpack authorized-ffxiv_dx11.exe new-output-directory. CPU only.
Spec-derived signed-10-bit endpoints + indices are constructed independently;
the oracle evaluates palette equations on original integer endpoint values,
not bcdec_rs or its C upstream. See docs/provenance/bc6h.md for scope/licenses.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_tex import reconstruct, quantize


WEIGHTS = (0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64)


def block(seed):
    choices = (-512, -511, -257, -33, -1, 0, 1, 33, 257, 510, 511)
    ends = [[choices[(seed*7+c*3+e*5) % len(choices)] for c in range(3)] for e in range(2)]
    indices = [(seed*3+i*5) % (8 if i == 0 else 16) for i in range(16)]
    wire, shift = 3, 5
    for row in ends:
        for v in row:
            wire |= (v & 1023) << shift
            shift += 10
    for i, v in enumerate(indices):
        wire |= v << shift
        shift += 3 if i == 0 else 4
    assert shift == 128

    def unquant(v):
        a = abs(v)
        if a == 0:
            return 0
        value = min(32767, ((a*32768+16384)//512)) if a < 511 else 32767
        return -value if v < 0 else value

    colors = []
    for index in indices:
        rgba = []
        for c in range(3):
            value = (unquant(ends[0][c])*(64-WEIGHTS[index])+unquant(ends[1][c])*WEIGHTS[index]+32)//64
            half = abs(value)*31//32 | (32768 if value < 0 else 0)
            rgba.append(struct.unpack('<e', struct.pack('<H', half))[0])
        colors.append((*rgba, 1.))
    return wire.to_bytes(16, 'little'), colors


def controlled(kind, dimensions, images, levels, seed):
    header, payloads, expected = bytearray(80), [], []
    struct.pack_into('<II3HBB', header, 0, kind, 0x6330, *dimensions, levels, images)
    width, height, depth = dimensions
    for m in range(levels):
        w, h = max(1, width >> m), max(1, height >> m)
        d = max(1, depth >> m) if kind == 0x1000000 else 1
        count = images if kind == 0x10000000 else (6 if kind == 0x2000000 else d)
        encoded, pixels = bytearray(), bytearray()
        for _ in range(count):
            surface = [None]*(w*h)
            for by in range(0, h, 4):
                for bx in range(0, w, 4):
                    data, colors = block(seed)
                    seed += 1
                    encoded.extend(data)
                    for y in range(min(4, h-by)):
                        for x in range(min(4, w-bx)):
                            surface[(by+y)*w+bx+x] = colors[y*4+x]
            pixels.extend(b''.join(struct.pack('<4f', *v) for v in surface))
        payloads.append(bytes(encoded))
        expected.append((w, h, count, bytes(pixels)))
    offset = 80
    for m, encoded in enumerate(payloads):
        struct.pack_into('<I', header, 28+m*4, offset)
        offset += len(encoded)
    return bytes(header)+b''.join(payloads), expected


def main():
    root, exe, out = map(Path, sys.argv[1:])
    assert not out.resolve().is_relative_to(root.parent.resolve())
    out.mkdir(parents=True, exist_ok=False)
    fixture = Path(__file__).resolve().parent.parent / 'tests/fixtures/bc6h-observations.json'
    pin = json.loads(fixture.read_text())
    assert (root.parent / 'ffxivgame.ver').read_text().strip() == pin['version']
    assert exe.stat().st_size <= 128 << 20
    binary = exe.read_bytes()
    assert hashlib.sha256(binary).hexdigest() == pin['binary_sha256']
    start, size = pin['format_function_rva'], pin['format_function_bytes']
    # Section mapping for this exact pinned PE, also checked by signature/hash.
    assert binary[:2] == b'MZ'
    code = binary[start-0xc00:start-0xc00+size]
    assert hashlib.sha256(code).hexdigest() == pin['format_function_sha256']
    assert binary[0x21831a-0xc00:0x218322-0xc00] == bytes.fromhex('81f9306300007424')
    assert binary[0x218346-0xc00:0x218350-0xc00] == bytes.fromhex('41b860000000418bc0c3')
    # Neighboring entries in the same pinned map independently identify BC4/5
    # as DXGI BC4_UNORM (80) and BC5_UNORM (83), used by the DDS pixel oracles.
    assert binary[0x21823e-0xc00:0x218250-0xc00] == bytes.fromhex('81f9206100000f8fbe0000000f84ae000000')
    assert binary[0x2182fe-0xc00:0x218308-0xc00] == bytes.fromhex('41b850000000418bc0c3')
    assert binary[0x218312-0xc00:0x21831a-0xc00] == bytes.fromhex('81f9306200007436')
    assert binary[0x218350-0xc00:0x21835a-0xc00] == bytes.fromhex('41b853000000418bc0c3')
    asm = subprocess.check_output(['objdump', '-d', '-Mintel', '--start-address=0x140218100', '--stop-address=0x140218389', str(exe)], text=True)
    (out / 'format-mapping.asm').write_text(asm)
    raw, _ = reconstruct(root, pin)
    assert hashlib.sha256(raw).hexdigest() == pin['resource_sha256'] and raw[80:] == bytes(len(raw)-80)
    paths, expected = [], []
    actual = out / 'actual-zero-cube.tex'
    subprocess.check_output(['target/debug/examples/extract_entry', str(root), pin['dat'], str(pin['offset']), str(actual)])
    assert actual.read_bytes() == raw
    paths.append(actual)
    expected.append([(max(1,256 >> m), max(1,256 >> m), 6,
        struct.pack('<4f',0,0,0,1)*(max(1,256 >> m)**2*6)) for m in range(9)])
    profiles = [(0x800000,(29,31,1),0,1), (0x10000000,(9,5,1),3,4),
        (0x2000000,(8,8,1),6,4), (0x1000000,(5,3,3),0,3)]
    controls = []
    for i, profile in enumerate(profiles):
        data, pixels = controlled(*profile, i*101)
        path = out / f'controlled-{i}.tex'
        path.write_bytes(data)
        paths.append(path)
        expected.append(pixels)
        controls.append({'kind':hex(profile[0]),'dimensions':profile[1], 'images':profile[2],
            'mips':profile[3], 'sha256':hashlib.sha256(data).hexdigest()})
    pixels = out / 'pixels'
    log = subprocess.check_output(['target/debug/examples/validate_tex', str(pixels), *map(str, paths)], text=True)
    (out / 'rust.tsv').write_text(log)
    totals = {'images':0, 'pixels':0, 'float_components':0, 'negative_values':0, 'above_one_values':0}
    digest = hashlib.sha256()
    for i, mips in enumerate(expected):
        for m, (w,h,count,values) in enumerate(mips):
            assert (pixels / f'{i}-{m}.rgba32f').read_bytes() == values
            floats = struct.unpack(f'<{len(values)//4}f', values)
            rgba = bytes(map(quantize, floats))
            assert (pixels / f'{i}-{m}.rgba8').read_bytes() == rgba
            digest.update(values)
            totals['images'] += count
            totals['pixels'] += w*h*count
            totals['float_components'] += len(floats)
            totals['negative_values'] += sum(v < 0 for v in floats)
            totals['above_one_values'] += sum(v > 1 for v in floats)
    result = {'resource_sha256': pin['resource_sha256'], 'native_dxgi_value':96,
        'real_blocks':len(raw[80:])//16, 'real_unique_blocks':1, 'real_mips':9, 'real_images':54,
        'controlled_profiles':controls, 'totals':totals, 'float_oracle_sha256':digest.hexdigest(),
        'limitations':'Actual cube is all-zero mode 1. Nonzero signed/HDR exact-bit proof uses controlled official mode 11, not real nonzero blocks or an all-mode independent oracle.'}
    if 'float_oracle_sha256' in pin:
        assert result['float_oracle_sha256'] == pin['float_oracle_sha256']
    text = json.dumps(result, indent=2)+'\n'
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

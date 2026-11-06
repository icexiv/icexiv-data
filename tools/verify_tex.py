"""Exact all-image TEX comparison, independent SqPack type-4 reconstruction.

Args: game/sqpack new-output-directory extracted.tex ...
Requires Pillow (MIT-CMU), used only as a generic DDS image oracle. It is not
a FFXIV parser and is not a library dependency. Each BC surface is wrapped in a
standard DDS header; no GPU, server or target executable is used.
"""
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import zlib
import PIL
from PIL import Image


def fnv(b):
    value = 0xcbf29ce484222325
    for x in b: value = (value ^ x) * 0x100000001b3 & 0xffffffffffffffff
    return f"{value:016x}"


def locate(root, source):
    if "dat" in source:
        dat = (root / source["dat"]).resolve()
        assert dat.is_relative_to(root.resolve())
        return dat, source["offset"]
    path = source["path"].lower()
    directory, filename = path.rsplit("/", 1)
    jam = lambda s: ~zlib.crc32(s.encode("ascii")) & 0xffffffff
    key = jam(directory) << 32 | jam(filename)
    category = {"chara":4,"ui":6,"vfx":8}[path.split("/", 1)[0]]
    for index in sorted((root / "ffxiv").glob(f"{category:02x}00*.win32.index")):
        with index.open("rb") as f:
            f.seek(12); hs, = struct.unpack("<I", f.read(4))
            f.seek(hs + 8); offset, size = struct.unpack("<II", f.read(8))
            assert size <= 64 * 1024 * 1024
            f.seek(offset)
            for identity, loc, _ in struct.iter_unpack("<QII", f.read(size)):
                if identity == key:
                    assert loc & 1 == 0
                    return index.with_suffix(f".dat{(loc >> 1) & 7}"), (loc >> 4) * 128
    raise ValueError("resource not found")


def reconstruct(root, source):
    dat, entry = locate(root, source)
    with dat.open("rb") as f:
        f.seek(entry); prefix = f.read(24)
        size, kind, target, _, _, levels = struct.unpack("<6I", prefix)
        assert kind == 4 and target <= 64 * 1024 * 1024 and 128 <= size <= 2 * 1024 * 1024
        assert 24 + levels * 20 <= size and levels > 0
        f.seek(entry); header = f.read(size)
        records = list(struct.iter_unpack("<5I", header[24:24 + levels * 20]))
        count = sum(r[4] for r in records)
        assert count * 2 <= size - 24 - levels * 20
        spans = struct.unpack_from(f"<{count}H", header, 24 + levels * 20)
        body = entry + size
        assert records[0][0] <= target
        f.seek(body); output = bytearray(f.read(records[0][0]))
        block_index = 0
        for offset, stored_span, decoded_size, block_offset, n in records:
            assert block_offset == block_index
            assert stored_span == sum(spans[block_index:block_index + n])
            position, decoded = body + offset, 0
            for _ in range(n):
                f.seek(position); hs, _, stored, expanded = struct.unpack("<4I", f.read(16))
                length = expanded if stored == 32000 else stored
                assert expanded <= 65535 and 16 <= hs <= spans[block_index] and length <= spans[block_index] - hs
                f.seek(position + hs); data = f.read(length)
                if stored != 32000:
                    decoder = zlib.decompressobj(-15)
                    data = decoder.decompress(data, 65536)
                    assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
                assert len(data) == expanded and len(output) + expanded <= target
                output.extend(data); decoded += expanded
                position += spans[block_index]; block_index += 1
            assert decoded_size in [0, decoded]
        assert len(output) == target and block_index == len(spans)
    return bytes(output), {"dat":str(dat),"offset":entry,"blocks":count,"levels":levels,"comparison":"all extracted bytes equal"}


def dds(raw, width, height, fmt):
    fourcc = {0x6120:b"DX10",0x6230:b"DX10",0x6432:b"DX10",0x3420:b"DXT1",0x3430:b"DXT3",0x3431:b"DXT5"}[fmt]
    header = struct.pack("<7I",124,0x81007,height,width,len(raw),0,0) + bytes(44)
    header += struct.pack("<II4s5I",32,4,fourcc,0,0,0,0,0) + struct.pack("<5I",0x1000,0,0,0,0)
    if fmt in (0x6120,0x6230,0x6432): header += struct.pack("<5I",{0x6120:80,0x6230:83,0x6432:98}[fmt],3,0,1,0)
    return Image.open(io.BytesIO(b"DDS " + header + raw)).convert("RGBA").tobytes()


def quantize(v):
    if math.isnan(v): return 0
    v = max(0., min(1., v))
    rounded, = struct.unpack("<f", struct.pack("<f", v * 255))
    return math.floor(rounded + 0.5)


def pixels(raw, width, height, fmt):
    if fmt in [0x3420,0x3430,0x3431,0x6120,0x6230,0x6432]: return dds(raw,width,height,fmt), None
    if fmt == 0x1131:
        image = Image.new('RGBA', (width, height), (255,255,255,0))
        image.putalpha(Image.frombytes('L', (width,height), raw))
        return image.tobytes(), None
    if fmt == 0x1130: return Image.frombytes("L",(width,height),raw).convert("RGBA").tobytes(), None
    if fmt == 0x1450: return Image.frombytes("RGBA",(width,height),raw,"raw","BGRA").tobytes(), None
    if fmt == 0x1451: return Image.frombytes("RGB",(width,height),raw,"raw","BGRX").convert("RGBA").tobytes(), None
    if fmt == 0x1440:
        result = bytearray()
        for v, in struct.iter_unpack("<H", raw): result.extend([((v >> 8) & 15) * 17,((v >> 4) & 15) * 17,(v & 15) * 17,(v >> 12) * 17])
        return bytes(result), None
    if fmt == 0x2350:
        result, floats = bytearray(), bytearray()
        for word, in struct.iter_unpack('<I', raw):
            values = []
            for shift, bits in [(0,6),(11,6),(22,5)]:
                lane = word >> shift & ((1 << (bits+5))-1)
                exponent, fraction = lane >> bits, lane & ((1 << bits)-1)
                value = (math.inf if not fraction else math.nan) if exponent == 31 else (
                    math.ldexp(fraction, -14-bits) if not exponent else math.ldexp(1+fraction/(1 << bits), exponent-15))
                values.append(value)
            values.append(1.)
            result.extend(map(quantize, values))
            floats.extend(struct.pack('<4f', *values))
        return bytes(result), bytes(floats)
    floating = {0x2140:(1,"e"),0x2150:(1,"f"),0x2250:(2,"e"),0x2260:(2,"f"),0x2460:(4,"e"),0x2470:(4,"f")}
    if fmt in floating:
        channels, code = floating[fmt]; result, floats = bytearray(), bytearray()
        for v in struct.iter_unpack(f"<{channels}{code}", raw):
            rgba = (v[0],v[0],v[0],1.) if channels == 1 else ((v[0],v[1],0.,1.) if channels == 2 else v)
            result.extend(map(quantize,rgba)); floats.extend(struct.pack("<4f",*rgba))
        return bytes(result), bytes(floats)
    raise ValueError(f"oracle not selected for format {fmt:#x}")


def observe(b, sample, rust_output, packed_volume=False):
    attributes, fmt, width, height, depth, flags, array = struct.unpack_from("<II3HBB", b)
    kind = attributes & 0x13c00000
    layout = {0x800000:"D2",0x10000000:"D2Array",0x2000000:"Cube",0x1000000:"Volume"}[kind]
    mips = flags & 127
    offsets = struct.unpack_from("<13I", b, 28)
    layers, faces = (array if layout == "D2Array" else 1), (6 if layout == "Cube" else 1)
    if layout == "Cube": assert array in [0,6]
    if packed_volume:
        assert layout == 'Volume' and list(offsets[:mips]) == [80]*mips
    cursor = 80
    out = [f"tex\t{len(b)}\t{layout}\t{fmt}\t{width}\t{height}\t{depth}\t{array}\t{mips}"]
    image_count = pixel_count = float_samples = negatives = hdr = 0
    bc45 = [{'endpoint_gt':0,'endpoint_le':0,'endpoint_equal':0,'selectors':[0]*8}
        for _ in range({0x6120:1,0x6230:2}.get(fmt,0))]
    for m in range(mips):
        w,h = max(1,width >> m),max(1,height >> m)
        d = max(1,depth >> m) if layout == "Volume" else 1
        n = layers * faces * d
        if fmt in [0x3420,0x3430,0x3431,0x6120,0x6230,0x6432]: plane = ((w + 3) // 4) * ((h + 3) // 4) * (8 if fmt in (0x3420,0x6120) else 16)
        else: plane = w * h * {0x1130:1,0x1131:1,0x1440:2,0x1450:4,0x1451:4,0x2140:2,0x2150:4,0x2250:4,0x2260:8,0x2350:4,0x2460:8,0x2470:16}[fmt]
        start = cursor if packed_volume else offsets[m]
        data = b[start:start + n * plane]
        assert len(data) == n * plane
        cursor = start+n*plane
        if bc45:
            stride = len(bc45)*8
            for block in range(0,len(data),stride):
                for channel,hist in enumerate(bc45):
                    at = block+channel*8
                    a,c = data[at:at+2]
                    hist['endpoint_gt' if a>c else 'endpoint_le'] += 1
                    hist['endpoint_equal'] += a==c
                    selectors = int.from_bytes(data[at+2:at+8],'little')
                    for texel in range(16):
                        hist['selectors'][selectors>>(texel*3)&7] += 1
        rgba, floats, images = bytearray(), bytearray(), []
        for i in range(n):
            raw = data[i * plane:(i + 1) * plane]
            decoded, floating = pixels(raw,w,h,fmt)
            rgba.extend(decoded)
            if floating is not None: floats.extend(floating)
            images.append(f"image\t{i}\t{plane}\t{fnv(raw)}")
        actual = (rust_output / f"{sample}-{m}.rgba8").read_bytes()
        assert actual == rgba, f"exact RGBA8 mismatch at sample {sample}, mip {m}"
        if floats:
            assert (rust_output / f"{sample}-{m}.rgba32f").read_bytes() == floats, "exact float mismatch"
            for v, in struct.iter_unpack("<f", floats):
                float_samples += 1; negatives += v < 0; hdr += v > 1
        out.append(f"mip\t{m}\t{w}\t{h}\t{d}\t{layers}\t{faces}\t{n}\t{len(data)}\t{len(rgba)}\t{fnv(rgba)}\t{fnv(floats) if floats else 'absent'}")
        out.extend(images); image_count += n; pixel_count += w * h * n
    report = {"layout":layout,"format":hex(fmt),"mips":mips,"images":image_count,"pixels":pixel_count,
                 "float_samples":float_samples,"negative_float_values":negatives,"above_one_float_values":hdr}
    if bc45:
        report['bc45_encoded_block_channels'] = bc45
    return out, report


def main():
    root, output, paths = Path(sys.argv[1]), Path(sys.argv[2]), list(map(Path,sys.argv[3:]))
    assert paths and not output.resolve().is_relative_to(root.parent.resolve())
    output.mkdir(parents=True, exist_ok=False); rust_output = output / "rust-images"
    rust = subprocess.check_output(["target/debug/examples/validate_tex",str(rust_output),*map(str,paths)], text=True)
    (output / "rust.tsv").write_text(rust)
    manifest = json.loads((Path(__file__).resolve().parent.parent / "tests/fixtures/tex-observations.json").read_text())
    sources = {s["file"]:s for s in manifest["samples"]}
    expected, reports = [], []
    for i,path in enumerate(paths):
        source = sources[path.name]
        b = path.read_bytes(); direct, extraction = reconstruct(root,source)
        assert direct == b, "independent type-4 extraction differs"
        rows, report = observe(b,i,rust_output)
        report.update({"file":str(path),"bytes":len(b),"sha256":hashlib.sha256(b).hexdigest(),
                       "rows_sha256":hashlib.sha256(("\n".join(rows) + "\n").encode()).hexdigest(),"extraction":extraction})
        if "sha256" in source:
            assert report["sha256"] == source["sha256"] and report["rows_sha256"] == source["rows_sha256"]
            report["pinned_regression"] = "equal"
        else: report["pinned_regression"] = "unregistered sample"
        expected.extend(rows); reports.append(report)
    assert rust == "\n".join(expected) + "\n", "TEX metadata/image rows differ"
    result = {"version":(root.parent / "ffxivgame.ver").read_text().strip(),"platform":"Win32",
              "python":sys.version,"pillow":PIL.__version__,"samples":reports,
              "comparison":"all type-4 bytes, all RGBA8 pixels and float scalar bits equal"}
    text = json.dumps(result,indent=2) + "\n"; (output / "report.json").write_text(text); print(text,end="")


if __name__ == "__main__": main()

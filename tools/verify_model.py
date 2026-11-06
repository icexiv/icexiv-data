"""Read-only type-3 evidence. Args: sqpack-root new-output-dir model-path ...

Build the local_sqpack example first. Only Win32 index (not synonyms) is probed.
Python zlib independently checks every block and the reconstructed file bytes.
"""

import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib


def locate(root, path):
    if path.startswith("@"):
        name, offset = path[1:].rsplit(":", 1)
        dat = (root / name).resolve()
        assert dat.is_relative_to(root.resolve()) and dat.name.endswith(tuple(f".dat{i}" for i in range(8)))
        return dat, int(offset)
    directory, name = path.lower().rsplit("/", 1)
    jam = lambda s: ~zlib.crc32(s.encode("ascii")) & 0xffffffff
    key = jam(directory) << 32 | jam(name)
    for index in sorted((root / "ffxiv").glob("0400*.win32.index")):
        with index.open("rb") as f:
            f.seek(12)
            header_size, = struct.unpack("<I", f.read(4))
            f.seek(header_size + 8)
            offset, length = struct.unpack("<II", f.read(8))
            if length > 64 * 1024 * 1024:
                raise ValueError("index observation budget exceeded")
            f.seek(offset)
            for candidate, packed, _ in struct.iter_unpack("<QII", f.read(length)):
                if candidate == key:
                    assert packed & 1 == 0, "synonym outside experiment scope"
                    return index.with_suffix(f".dat{(packed >> 1) & 7}"), (packed >> 4) * 128
    raise ValueError(f"resource not found: {path}")


def observe(root, path, output, ordinal):
    dat, entry = locate(root, path)
    with dat.open("rb") as f:
        f.seek(entry)
        prefix = f.read(208)
        header_size, kind, target = struct.unpack_from("<III", prefix)
        assert kind == 3 and target <= 64 * 1024 * 1024
        assert 208 <= header_size <= 1024 * 1024
        f.seek(entry)
        header = f.read(header_size)
        capacities = struct.unpack_from("<11I", header, 24)
        packed_sizes = struct.unpack_from("<11I", header, 68)
        offsets = struct.unpack_from("<11I", header, 112)
        starts = struct.unpack_from("<11H", header, 156)
        counts = struct.unpack_from("<11H", header, 178)
        spans = struct.unpack_from(f"<{sum(counts)}H", header, 208)
        payloads = []
        blocks = []
        for i in range(11):
            position = entry + header_size + offsets[i]
            decoded = b""
            for j in range(starts[i], starts[i] + counts[i]):
                f.seek(position)
                hsize, _, stored, expanded = struct.unpack("<4I", f.read(16))
                assert expanded <= 65535 and 16 <= hsize <= spans[j]
                assert (expanded if stored == 32000 else stored) <= spans[j] - hsize
                f.seek(position + hsize)
                data = f.read(expanded if stored == 32000 else stored)
                if stored == 32000:
                    block = data
                else:
                    decoder = zlib.decompressobj(-15)
                    block = decoder.decompress(data, 65536)
                    assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
                assert len(block) == expanded
                assert len(decoded) + len(block) <= target
                decoded += block
                blocks.append({"segment": i, "span_index": j, "offset": position,
                               "stored": stored, "expanded": expanded})
                position += spans[j]
            assert len(decoded) <= target
            assert (len(decoded) + 127) // 128 * 128 == capacities[i]
            assert sum(spans[starts[i]:starts[i] + counts[i]]) == packed_sizes[i]
            payloads.append(decoded)
    # Reconstruct using sequential cursor observations and Python struct packing.
    logical = bytearray(68)
    vertex_offsets = [0] * 3
    index_offsets = [0] * 3
    for segment in [0, 1, 2, 5, 8, 3, 6, 9, 4, 7, 10]:
        if payloads[segment]:
            if 2 <= segment <= 4:
                vertex_offsets[segment - 2] = len(logical)
            elif 8 <= segment <= 10:
                index_offsets[segment - 8] = len(logical)
        logical.extend(payloads[segment])
    assert len(logical) == target
    struct.pack_into("<IIIHH", logical, 0, struct.unpack_from("<I", header, 20)[0],
                     len(payloads[0]), len(payloads[1]), *struct.unpack_from("<HH", header, 200))
    struct.pack_into("<12I", logical, 16, *vertex_offsets, *index_offsets,
                     *map(len, payloads[2:5]), *map(len, payloads[8:11]))
    logical[64:67] = header[204:207]
    extracted = output / f"{ordinal}.mdl"
    if path.startswith("@"):
        command = ["target/debug/examples/extract_entry", str(root), str(dat.relative_to(root.resolve())), str(entry), str(extracted)]
    else:
        command = ["target/debug/examples/local_sqpack", str(root), path, str(extracted)]
    log = subprocess.check_output(command, text=True)
    assert extracted.read_bytes() == logical
    (output / f"{ordinal}-extract.txt").write_text(log)
    (output / f"{ordinal}-header.bin").write_bytes(header)
    (output / f"{ordinal}-blocks.json").write_text(json.dumps(blocks, indent=2))
    return {"path": path, "dat": str(dat), "entry_offset": entry,
            "entry_header_bytes": header_size, "bytes": target,
            "sha256": hashlib.sha256(logical).hexdigest(),
            "header_sha256": hashlib.sha256(header).hexdigest(),
            "capacities": capacities, "decoded_sizes": list(map(len, payloads)),
            "block_counts": counts, "lods": header[204], "edge": header[206],
            "comparison": "all blocks and logical bytes equal"}


def main():
    root, output = map(Path, sys.argv[1:3])
    if output.resolve().is_relative_to(root.resolve().parent):
        raise ValueError("output must be outside installation")
    paths = sys.argv[3:]
    assert paths
    output.mkdir(parents=True, exist_ok=False)
    report = {"version": (root.parent / "ffxivgame.ver").read_text().strip(),
              "platform": "Win32", "python": sys.version, "zlib": zlib.ZLIB_RUNTIME_VERSION,
              "samples": [observe(root, p, output, i) for i, p in enumerate(paths)]}
    text = json.dumps(report, indent=2) + "\n"
    (output / "report.json").write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()

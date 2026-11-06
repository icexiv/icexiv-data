"""Bounded read-only discovery of non-normal/extra-LOD Win32 model entries.

Args: game/sqpack new-report.json. Reads at most the first 10000 records of
bgcommon then bg base-pack indexes; stops after five candidates. Hashes are
identities, not recovered path names. No game binaries or parsers are executed.
"""

import contextlib
import json
from pathlib import Path
import struct
import sys
import zlib


def main():
    root = Path(sys.argv[1]) / "ffxiv"
    destination = Path(sys.argv[2])
    assert not destination.resolve().is_relative_to(root.parent.parent.resolve())
    findings, checked = [], 0
    with contextlib.ExitStack() as stack:
        for stem in ["010000", "020000"]:
            with (root / f"{stem}.win32.index").open("rb") as f:
                f.seek(12)
                header_size, = struct.unpack("<I", f.read(4))
                f.seek(header_size + 8)
                offset, size = struct.unpack("<II", f.read(8))
                assert size <= 64 * 1024 * 1024
                f.seek(offset)
                records = list(struct.iter_unpack("<QII", f.read(min(size, 10000 * 16))))
            files = {}
            for ordinal, (key, packed, _) in enumerate(records):
                if packed & 1:
                    continue
                dat = root / f"{stem}.win32.dat{(packed >> 1) & 7}"
                if dat not in files:
                    files[dat] = stack.enter_context(dat.open("rb"))
                f = files[dat]
                entry = (packed >> 4) * 128
                f.seek(entry)
                prefix = f.read(208)
                if len(prefix) != 208 or struct.unpack_from("<I", prefix, 4)[0] != 3:
                    continue
                checked += 1
                header_size, _, target = struct.unpack_from("<III", prefix)
                if target > 16 * 1024 * 1024 or not 208 <= header_size <= 1024 * 1024:
                    continue
                f.seek(entry)
                header = f.read(header_size)
                first, = struct.unpack_from("<H", header, 158)
                count, = struct.unpack_from("<H", header, 180)
                offset, = struct.unpack_from("<I", header, 116)
                position = entry + header_size + offset
                runtime = bytearray()
                for j in range(first, first + count):
                    f.seek(position)
                    hsize, _, stored, expanded = struct.unpack("<4I", f.read(16))
                    block_span, = struct.unpack_from("<H", header, 208 + j * 2)
                    stored_size = expanded if stored == 32000 else stored
                    assert expanded <= 65535 and 16 <= hsize <= block_span and stored_size <= block_span - hsize
                    f.seek(position + hsize)
                    data = f.read(stored_size)
                    if stored != 32000:
                        decoder = zlib.decompressobj(-15)
                        data = decoder.decompress(data, 65536)
                        assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
                    assert len(data) == expanded and len(runtime) + expanded <= target
                    runtime.extend(data)
                    position += block_span
                string_size, = struct.unpack_from("<I", runtime, 4)
                model_header = runtime[8 + string_size:8 + string_size + 56]
                assert len(model_header) == 56
                elements, = struct.unpack_from("<H", model_header, 24)
                lod_start = 8 + string_size + 56 + elements * 32
                ranges = [list(struct.unpack_from("<8H", runtime, lod_start + lod * 60 + 12)) for lod in range(3)]
                if model_header[26] or model_header[27] & 16 or any(row[i] for row in ranges for i in [1, 3, 5, 7]):
                    findings.append({"stem": stem, "index_record": ordinal, "hash": f"{key:016x}",
                                     "dat": dat.name, "offset": entry, "target": target,
                                     "version": hex(struct.unpack_from("<I", header, 20)[0]),
                                     "flags2": model_header[27], "terrain_meshes": model_header[26], "lod_ranges": ranges})
                    if len(findings) == 5:
                        break
            if len(findings) == 5:
                break
    report = {"version": (root.parent.parent / "ffxivgame.ver").read_text().strip(),
              "platform": "Win32", "model_entries_examined": checked,
              "index_record_limit_per_pack": 10000, "candidates": findings}
    text = json.dumps(report, indent=2) + "\n"
    with destination.open("x") as f:
        f.write(text)
    print(text, end="")


if __name__ == "__main__":
    main()

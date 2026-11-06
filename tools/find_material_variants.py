"""Bounded read-only MTRL dataset discovery.

Args: game/sqpack new-report.json [dataset-byte-size, default 544].
Examines at most 100000 index records per base chara/bgcommon pack; stops at the
first match. Inspect only small standard entries and their first data block.
"""
import contextlib
import json
from pathlib import Path
import struct
import sys
import zlib


def main():
    root, destination = Path(sys.argv[1]), Path(sys.argv[2])
    wanted = int(sys.argv[3]) if len(sys.argv) > 3 else 544
    assert not destination.resolve().is_relative_to(root.parent.resolve())
    findings, examined, material_count = [], 0, 0
    with contextlib.ExitStack() as stack:
        for stem in ["040000", "010000"]:
            index = root / "ffxiv" / f"{stem}.win32.index"
            with index.open("rb") as f:
                f.seek(12)
                header_size, = struct.unpack("<I", f.read(4))
                f.seek(header_size + 8)
                offset, size = struct.unpack("<II", f.read(8))
                assert size <= 64 * 1024 * 1024
                f.seek(offset)
                table = f.read(min(size, 100000 * 16))
            files = {}
            for ordinal, (key, packed, _) in enumerate(struct.iter_unpack("<QII", table)):
                if packed & 1:
                    continue
                name = f"{stem}.win32.dat{(packed >> 1) & 7}"
                if name not in files:
                    files[name] = stack.enter_context((root / "ffxiv" / name).open("rb"))
                f = files[name]
                entry = (packed >> 4) * 128
                f.seek(entry)
                h = f.read(32)
                if len(h) != 32:
                    continue
                header_size, kind, target = struct.unpack_from("<III", h)
                if kind != 2 or not 16 <= target <= 8192:
                    continue
                examined += 1
                offset, stored_span, expected = struct.unpack_from("<IHH", h, 24)
                start = entry + header_size + offset
                f.seek(start)
                hsize, _, stored, expanded = struct.unpack("<4I", f.read(16))
                stored_size = expanded if stored == 32000 else stored
                if expanded != expected or not 16 <= hsize <= stored_span or stored_size > stored_span - hsize:
                    continue
                f.seek(start + hsize)
                data = f.read(stored_size)
                if stored != 32000:
                    decoder = zlib.decompressobj(-15)
                    data = decoder.decompress(data, 65536)
                    assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
                if len(data) != expanded or len(data) < 16 or struct.unpack_from("<I", data)[0] != 0x01030000:
                    continue
                material_count += 1
                dataset, = struct.unpack_from("<H", data, 6)
                if dataset == wanted:
                    findings.append({"index": str(index.relative_to(root)), "ordinal": ordinal,
                                     "hash": f"{key:016x}", "dat": f"ffxiv/{name}", "offset": entry,
                                     "bytes": target, "dataset_bytes": dataset})
                    break
            if findings:
                break
    report = {"version": (root.parent / "ffxivgame.ver").read_text().strip(), "platform": "Win32",
              "index_record_limit_per_pack": 100000, "small_entries_examined": examined,
              "material_entries_observed": material_count, "requested_dataset_bytes": wanted,
              "matches": findings}
    text = json.dumps(report, indent=2) + "\n"
    with destination.open("x") as f:
        f.write(text)
    print(text, end="")


if __name__ == "__main__":
    main()

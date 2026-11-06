"""Bounded read-only TEX header discovery in indexed type-4 entries.

Args: game/sqpack new-report.json. No extraction/decoding claim is made by this
inventory. At most 100000 records per base common/vfx/bg/chara/bgcommon pack.
"""
import contextlib
import json
from pathlib import Path
import struct
import sys


def main():
    root, output = Path(sys.argv[1]), Path(sys.argv[2])
    assert not output.resolve().is_relative_to(root.parent.resolve())
    found, examined, hist = {}, 0, {}
    float_formats = {0x2150,0x2250,0x2260,0x2460,0x2470}
    with contextlib.ExitStack() as stack:
        for stem in ["000000","080000","020000","040000","010000"]:
            index = root / "ffxiv" / f"{stem}.win32.index"
            with index.open("rb") as f:
                f.seek(12); hs, = struct.unpack("<I", f.read(4))
                f.seek(hs + 8); off, size = struct.unpack("<II", f.read(8))
                assert size <= 64 * 1024 * 1024
                f.seek(off); table = f.read(min(size, 100000 * 16))
            files = {}
            for ordinal, (key, loc, _) in enumerate(struct.iter_unpack("<QII", table)):
                if loc & 1:
                    continue
                name = f"{stem}.win32.dat{(loc >> 1) & 7}"
                if name not in files:
                    files[name] = stack.enter_context((root / "ffxiv" / name).open("rb"))
                f = files[name]; entry = (loc >> 4) * 128
                f.seek(entry); h = f.read(24)
                if len(h) != 24:
                    continue
                hs, kind, target = struct.unpack_from("<III", h)
                if kind != 4 or not 128 <= hs <= 2 * 1024 * 1024:
                    continue
                f.seek(entry + hs); tex = f.read(80)
                if len(tex) != 80:
                    continue
                flags, fmt, w, h, d, mips, array = struct.unpack_from("<II3HBB", tex)
                typ = flags & 0x13c00000
                examined += 1
                bucket = f"{typ:08x}/{fmt:04x}"
                hist[bucket] = hist.get(bucket, 0) + 1
                labels = []
                if typ == 0x02000000: labels.append("cube")
                if typ == 0x02000000 and mips & 127 > 1: labels.append("cube-mips")
                if typ == 0x01000000: labels.append("volume")
                if typ == 0x01000000 and mips & 127 > 1: labels.append("volume-mips")
                if typ == 0x10000000: labels.append("array")
                if fmt in float_formats: labels.append("float")
                if fmt == 0x6330: labels.append("bc6h")
                for label in labels:
                    if label not in found and target <= 16 * 1024 * 1024:
                        found[label] = {"index": str(index.relative_to(root)), "ordinal": ordinal,
                                        "hash": f"{key:016x}", "dat": f"ffxiv/{name}", "offset": entry,
                                        "bytes": target, "attributes": hex(flags), "format": hex(fmt),
                                        "width": w, "height": h, "depth": d, "mips": mips,
                                        "array_size": array, "surface_offsets": list(struct.unpack_from("<13I", tex, 28))}
                if all(label in found for label in ["cube","volume","array","float","cube-mips"]):
                    break
            if all(label in found for label in ["cube","volume","array","float","cube-mips"]):
                break
    report = {"version": (root.parent / "ffxivgame.ver").read_text().strip(), "platform": "Win32",
              "record_limit_per_pack": 100000, "type4_headers_examined": examined, "histogram": hist, "matches": found}
    text = json.dumps(report, indent=2) + "\n"
    with output.open("x") as f:
        f.write(text)
    print(text, end="")


if __name__ == "__main__":
    main()

"""Independent complete STM column decode and Stain/MTRL reference checks.

Args: new-output legacy.stm dawntrail.stm Stain.exh Stain-page.exd [MTRL ...]
Raw game files remain outside the repository. No third-party FFXIV parser used.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


class Hash:
    def __init__(self): self.v = 0xcbf29ce484222325
    def data(self, b):
        for x in b: self.v = (self.v ^ x) * 0x100000001b3 & 0xffffffffffffffff
    def word(self, v): self.data(struct.pack("<I", v))
    def __str__(self): return f"{self.v:016x}"


def observe_stm(path):
    b = path.read_bytes()
    magic, version, count, colors, scalars = struct.unpack_from("<HHHBB", b)
    assert magic == 0x534d and version in [0x0101,0x0201]
    legacy = version == 0x0101
    layout, n = ("Legacy", 5) if legacy else ("Dawntrail", 12)
    keys = struct.unpack_from(f"<{count}I", b, 8)
    offsets = struct.unpack_from(f"<{count}I", b, 8 + count * 4)
    base, used = 8 + count * 8, 0
    out, hist = [], {}
    for identity, offset in zip(keys, offsets):
        p = base + offset * 2
        ends = struct.unpack_from(f"<{n}H", b, p)
        data = p + n * 2
        used = max(used, data + ends[-1] * 2)
        columns, capacities = [], []
        previous = 0
        for c, end in enumerate(ends):
            assert end >= previous
            raw = b[data + previous * 2:data + end * 2]
            previous = end
            components, width = (3,6) if c < 3 else (1,2)
            if not raw:
                encoding, palette, capacity = "Zero", 0, 254
                values = [(0,) * components] * 254
            elif len(raw) == width:
                encoding, palette, capacity = "Uniform", 1, 254
                values = [struct.unpack(f"<{components}H", raw)] * 254
            elif len(raw) == width * 254:
                encoding, palette, capacity = "Dense", 254, 254
                values = list(struct.iter_unpack(f"<{components}H", raw))
            else:
                encoding, capacity = "Palette", 253
                dictionary, indices = raw[:-254], raw[-254:]
                assert len(dictionary) % width == 0 and indices[0] == 255
                palette = len(dictionary) // width
                decoded = [(0,) * components] + list(struct.iter_unpack(f"<{components}H", dictionary))
                assert all(i <= palette for i in indices[1:])
                values = [decoded[i] for i in indices[1:]]
            hist[encoding] = hist.get(encoding, 0) + 1
            raw_hash, decoded_hash = Hash(), Hash()
            raw_hash.data(raw)
            for value in values:
                bits = list(value) + [0] * (3 - components)
                for half in bits:
                    fp, = struct.unpack("<e", struct.pack("<H", half))
                    fbits, = struct.unpack("<I", struct.pack("<f", fp))
                    decoded_hash.word(half)
                    decoded_hash.word(fbits)
            columns.append(f"column\t{c}\t{encoding}\t{len(raw)}\t{palette}\t{components}\t{raw_hash}\t{decoded_hash}")
            capacities.append(capacity)
        out.append(f"template\t{identity}\t{min(capacities)}")
        out.extend(columns)
    tail = len(b) - used
    out.insert(0, f"stm\t{len(b)}\t{layout}\t{count}\t{colors},{scalars}\t{tail}")
    report = {"file": str(path), "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest(),
              "layout": layout, "templates": count, "template_ids": keys, "column_encodings": hist,
              "trailing_bytes": tail, "tail_all_zero": not any(b[used:]),
              "rows_sha256": hashlib.sha256(("\n".join(out) + "\n").encode()).hexdigest()}
    return out, report


def observe_stains(exh, exd):
    h, b = exh.read_bytes(), exd.read_bytes()
    assert h[:4] == b"EXHF" and b[:4] == b"EXDF"
    fixed, cols = struct.unpack_from(">HH", h, 6)
    columns = list(struct.iter_unpack(">HH", h[32:32 + cols * 4]))
    index_size, = struct.unpack_from(">I", b, 8)
    index = sorted(struct.iter_unpack(">II", b[32:32 + index_size]))
    out = [f"stains\t{len(index)}\t{cols}"]
    strings = 0
    for identity, offset in index:
        size, subrows = struct.unpack_from(">IH", b, offset)
        assert subrows == 1
        data = b[offset + 6:offset + 6 + size]
        values = []
        for kind, at in columns:
            if kind == 0:
                relative, = struct.unpack_from(">I", data, at)
                start = fixed + relative
                value = data[start:data.index(0, start)]
                values.append("s:" + value.hex())
                value.decode("utf-8")
                strings += 1
            elif kind == 3: values.append(f"u8:{data[at]}")
            elif kind == 7: values.append(f"u32:{struct.unpack_from('>I', data, at)[0]}")
            elif 0x19 <= kind <= 0x20: values.append(f"b:{(data[at] >> (kind - 0x19)) & 1}")
            else: raise ValueError(f"unobserved Stain column type {kind}")
        out.append(f"stain\t{identity}\t" + "\t".join(values))
    return out, {"exh_sha256": hashlib.sha256(h).hexdigest(), "exd_sha256": hashlib.sha256(b).hexdigest(),
                 "rows": len(index), "columns": cols, "minimum_id": index[0][0], "maximum_id": index[-1][0],
                 "utf8_strings_checked": strings}


def main():
    output = Path(sys.argv[1]); stm_paths = list(map(Path, sys.argv[2:4])); sheet_paths = list(map(Path, sys.argv[4:6]))
    materials = list(map(Path, sys.argv[6:]))
    output.mkdir(parents=True, exist_ok=False)
    expected, reports = [], []
    manifest_path = Path(__file__).resolve().parent.parent / "tests/fixtures/stm-observations.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
    pinned = {s["sha256"]:s for s in manifest.get("samples", []) + manifest.get("controlled_samples", [])}
    for path in stm_paths:
        rows, report = observe_stm(path)
        if report["sha256"] in pinned:
            assert report["rows_sha256"] == pinned[report["sha256"]]["rows_sha256"]
            report["pinned_regression"] = "equal"
        else: report["pinned_regression"] = "unregistered sample"
        expected.extend(rows); reports.append(report)
    rows, sheet = observe_stains(*sheet_paths); expected.extend(rows)
    for key, value in manifest.get("stain_sheet", {}).items():
        assert sheet[key] == value, "Stain sheet differs from pinned sample"
    assert sheet["maximum_id"] <= 253
    links = []
    for path in materials:
        b = path.read_bytes(); _, _, size, ss, _, tx, uv, cs, ad = struct.unpack_from("<I4H4B", b)
        start = 16 + (tx + uv + cs) * 4 + ss + ad
        if size not in [544,2176]: continue
        legacy = size == 544
        dye_start = start + (512 if legacy else 2048)
        ids = set()
        for raw, in struct.iter_unpack("<H" if legacy else "<I", b[dye_start:start + size]):
            identity = raw >> 5 if legacy else (raw >> 16) & 2047
            if identity: ids.add(identity)
        lookup = {str(identity): [r["layout"] for r in reports if identity in r["template_ids"]] for identity in sorted(ids)}
        assert all(lookup.values())
        links.append({"material": str(path), "templates": lookup})
    rust = subprocess.check_output(["target/debug/examples/validate_stm", *map(str, stm_paths + sheet_paths)], text=True)
    target = "\n".join(expected) + "\n"
    (output / "rust.tsv").write_text(rust); (output / "python.tsv").write_text(target)
    assert rust == target, "complete STM/Stain semantic rows differ"
    result = {"python": sys.version, "samples": reports, "stain_sheet": sheet, "material_links": links,
              "comparison": "all columns/all encoded stains and Stain fields equal"}
    text = json.dumps(result, indent=2) + "\n"; (output / "report.json").write_text(text); print(text, end="")


if __name__ == "__main__": main()

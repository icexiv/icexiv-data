"""Independent observations of extracted MTRL and SHPK, plus material links.

Args: new-output-directory extracted-files ...
Uses Python struct/zlib, not a third-party FFXIV parser. Compares full semantic
rows and shader/node fingerprints with examples/validate_materials. Sample
identities and semantic row digests are reported, without redistributing data.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib


class Hash:
    def __init__(self):
        self.value = 0xcbf29ce484222325

    def data(self, b):
        for x in b:
            self.value = (self.value ^ x) * 0x100000001b3 & 0xffffffffffffffff

    def word(self, v):
        self.data(struct.pack("<I", v & 0xffffffff))

    def resource(self, r):
        identity, name, flags, slot, size = r
        self.word(identity)
        self.word(len(name))
        self.data(name)
        for v in [flags, slot, size]:
            self.word(v)

    def __str__(self):
        return f"{self.value:016x}"


def observe_mtrl(b):
    version, size, ds, ss, shader, tx, uv, cs, ad = struct.unpack_from("<I4H4B", b)
    assert version == 0x01030000 and size == len(b)
    p = 16
    tables = []
    for n in [tx, uv, cs]:
        tables.append(b[p:p + n * 4])
        p += n * 4
    pool = b[p:p + ss]
    string = lambda offset: pool[offset:pool.index(0, offset)]
    shader_name = string(shader)
    p += ss
    additional = b[p:p + ad]
    p += ad
    dataset = b[p:p + ds]
    p += ds
    value_size, kc, cc, sc, flags = struct.unpack_from("<4HI", b, p)
    p += 12
    keys = list(struct.iter_unpack("<II", b[p:p + kc * 8]))
    p += kc * 8
    constants = list(struct.iter_unpack("<IHH", b[p:p + cc * 8]))
    p += cc * 8
    samplers = list(struct.iter_unpack("<IIB3s", b[p:p + sc * 12]))
    p += sc * 12
    values = b[p:p + value_size]
    assert p + value_size == len(b)
    rows = [f"mtrl\t{len(b)}\t{shader_name.hex()}\t{tx}\t{uv}\t{cs}\t{kc}\t{cc}\t{sc}\t{flags}", f"additional\t{additional.hex()}"]
    for i, (offset, flags) in enumerate(struct.iter_unpack("<HH", tables[0])):
        rows.append(f"texture\t{i}\t{string(offset).hex()}\t{flags}")
    for label, table in zip(["uv", "color-set"], tables[1:]):
        for i, (offset, index, unknown) in enumerate(struct.iter_unpack("<HBB", table)):
            rows.append(f"{label}\t{i}\t{string(offset).hex()}\t{index}\t{unknown}")
    for i, (identity, value) in enumerate(keys):
        rows.append(f"key\t{i}\t{identity}\t{value}")
    for i, (identity, offset, length) in enumerate(constants):
        assert offset + length <= len(values)
        rows.append(f"constant\t{i}\t{identity}\t{offset}\t{length}\t{values[offset:offset + length].hex()}")
    for i, (identity, flags, index, reserved) in enumerate(samplers):
        assert index < tx or index == 255
        rows.append(f"sampler\t{i}\t{identity}\t{flags}\t{index}\t{reserved.hex()}")
    templates, negatives, hdr = set(), 0, 0
    if ds:
        assert ds in [512, 544, 2048, 2176]
        legacy = ds < 2048
        count, lanes = (16, 16) if legacy else (32, 32)
        color_size = count * lanes * 2
        dye = dataset[color_size:]
        if len(additional) >= 4:
            declaration, = struct.unpack_from('<I', additional)
            if declaration & 4:
                exponent = declaration >> 4 & 15
                declared_lanes = 4*(1 << exponent) if exponent else 16
                declared_rows = 1 << (declaration >> 8 & 15) if exponent else 16
                assert (declared_lanes, declared_rows) == (lanes, count)
                assert bool(declaration & 8) == bool(dye)
        layout = "Legacy" if legacy else "Dawntrail"
        rows.append(f"table\t{layout}\t{count}\t{str(bool(dye)).lower()}")
        for i in range(count):
            raw = dataset[i * lanes * 2:(i + 1) * lanes * 2]
            floats = list(struct.unpack(f"<{lanes}e", raw))
            negatives += sum(v < 0 for v in floats)
            hdr += sum(v > 1 for v in floats)
            bits = [struct.unpack("<I", struct.pack("<f", v))[0] for v in floats + [0] * (32 - lanes)]
            rows.append(f"row\t{i}\t{raw.hex()}\t" + ",".join(f"{v:08x}" for v in bits))
            if dye:
                value = struct.unpack_from("<H" if legacy else "<I", dye, i * (2 if legacy else 4))[0]
                mask = value & (31 if legacy else 4095)
                template = value >> 5 if legacy else (value >> 16) & 2047
                channel = -1 if legacy else (value >> 27) & 3
                unknown = 0 if legacy else value & 0xe000f000
                templates.add(template)
                rows.append(f"dye\t{i}\t{value}\t{mask}\t{template}\t{channel}\t{unknown}")
    else:
        layout, count, dye = None, 0, b""
        rows.append("table\tabsent")
    report = {"format": "MTRL", "shader": shader_name.decode("ascii"), "dataset_bytes": ds,
              "layout": layout, "rows": count, "dye_rows": count if dye else 0,
              "negative_half_values": negatives, "above_one_half_values": hdr,
              "templates": sorted(templates), "textures": tx, "keys": kc, "constants": cc, "samplers": sc}
    links = {"shader": shader_name.decode("ascii"), "keys": keys, "constants": constants, "samplers": samplers}
    return rows, report, links


def observe_shpk(b):
    assert b[:4] == b"ShPk" and b[8:12] == b"DX11"
    version=struct.unpack_from('<I',b,4)[0]
    assert version in[0x0d01,0x0e01]and struct.unpack_from("<I", b, 12)[0] == len(b)
    code_start, names_start = struct.unpack_from("<II", b, 16)
    vertex, pixel, material_size = struct.unpack_from("<III", b, 24)
    param_count, default_flag, c, _, s, t, r, _ = struct.unpack_from("<8H", b, 36)
    key_counts = list(struct.unpack_from("<3I", b, 52))
    node_count, alias_count, hull, domain, geometry = struct.unpack_from("<5I", b, 64)
    cluster_count=struct.unpack_from('<I',b,84)[0]if version==0x0e01 else 0
    stages = ["Vertex", "Pixel", "Hull", "Domain", "Geometry"]
    counts = [vertex, pixel, hull, domain, geometry]
    kinds = ["Constant", "Sampler", "Texture", "UnorderedAccess"]
    groups = ["System", "Scene", "Material", "SubView"]
    names = b[names_start:]
    crc_checked = 0
    def resource(at):
        nonlocal crc_checked
        identity, offset, length, flags, slot, size = struct.unpack_from("<II4H", b, at)
        name = names[offset:offset + length]
        assert names[offset + length] == 0 and 0 not in name
        assert identity == (~zlib.crc32(name, 0xffffffff) & 0xffffffff)
        crc_checked += 1
        return identity, name, flags, slot, size
    p = 88 if version==0x0e01 else 84
    shader_rows = []
    dxbc_checked = 0
    for stage, n in zip(stages, counts):
        for index in range(n):
            offset, length, nc, ns, nr, nt, unknown = struct.unpack_from("<II4HI", b, p)
            p += 20
            tables = {}
            for kind, count in [("Constant", nc), ("Sampler", ns), ("UnorderedAccess", nr), ("Texture", nt)]:
                tables[kind] = [resource(p + i * 16) for i in range(count)]
                p += count * 16
            prefix_size = 8 if stage == "Vertex" else 0
            assert offset + length <= names_start - code_start
            raw = b[code_start + offset:code_start + offset + length]
            prefix, code = raw[:prefix_size], raw[prefix_size:]
            assert code[:4] == b"DXBC"
            # Independent container-length observation, without shader execution.
            assert struct.unpack_from("<I", code, 24)[0] == len(code)
            dxbc_checked += 1
            code_hash, resource_hash = Hash(), Hash()
            code_hash.data(code)
            for kind in kinds:
                resource_hash.word(len(tables[kind]))
                for res in tables[kind]:
                    resource_hash.resource(res)
            shader_rows.append(f"shader\t{stage}\t{index}\t{unknown}\t{prefix.hex()}\t{len(code)}\t{code_hash}\t{resource_hash}")
    params = list(struct.iter_unpack("<IHH", b[p:p + param_count * 8]))
    p += param_count * 8
    defaults = b[p:p + material_size] if default_flag else None
    if default_flag:
        p += material_size
    rows = [f"shpk\t{len(b)}\t" + ",".join(map(str, counts)) + f"\t{param_count}\t{material_size}\t{node_count}\t{alias_count}\t{cluster_count}"]
    for i, (identity, offset, size) in enumerate(params):
        assert offset + size <= material_size
        data = "absent" if defaults is None else defaults[offset:offset + size].hex()
        rows.append(f"parameter\t{i}\t{identity}\t{offset}\t{size}\t{data}")
    globals_ = {}
    for kind, count in zip(kinds, [c, s, t, r]):
        globals_[kind] = [resource(p + i * 16) for i in range(count)]
        p += count * 16
        for i, res in enumerate(globals_[kind]):
            identity, name, flags, slot, size = res
            rows.append(f"resource\t{kind}\t{i}\t{identity}\t{name.hex()}\t{flags}\t{slot}\t{size}")
    keys = []
    for count in key_counts:
        keys.append(list(struct.iter_unpack("<II", b[p:p + count * 8])))
        p += count * 8
    subview = struct.unpack_from("<II", b, p)
    p += 8
    keys.append(list(enumerate(subview, 1)))
    for group, table in zip(groups, keys):
        for i, (identity, value) in enumerate(table):
            rows.append(f"default-key\t{group}\t{i}\t{identity}\t{value}")
    rows.extend(shader_rows)
    nodes_hash, alias_hash, cluster_hash = Hash(), Hash(), Hash()
    total_passes = 0
    for _ in range(node_count):
        selector, passes = struct.unpack_from("<II", b, p)
        indices = struct.unpack_from("<16b", b, p + 8)
        unknown = struct.unpack_from("<II", b, p + 24)
        p += 32
        assert all(i == -1 or 0 <= i < passes for i in indices)
        nodes_hash.word(selector)
        nodes_hash.word(passes)
        for i in indices:
            nodes_hash.word(i)
        for count in key_counts + [2]:
            for value in struct.unpack_from(f"<{count}I", b, p):
                nodes_hash.word(value)
            p += count * 4
        for _ in range(passes):
            values = struct.unpack_from("<6I", b, p)
            p += 24
            assert values[1] < vertex or values[1] == 0xffffffff
            assert values[2] < pixel or values[2] == 0xffffffff
            for value in values:
                nodes_hash.word(value)
        total_passes += passes
        for value in unknown:
            nodes_hash.word(value)
    for selector, node in struct.iter_unpack("<II", b[p:p + alias_count * 8]):
        assert node < node_count
        alias_hash.word(selector)
        alias_hash.word(node)
    p += alias_count * 8
    assert p <= code_start
    clusters = b[p:code_start]
    cluster_hash.data(clusters)
    rows.append(f"node-fingerprints\t{nodes_hash}\t{alias_hash}\t{len(clusters)}\t{cluster_hash}")
    report = {"format": "SHPK", "version": f"0x{version:04x}", "dx": "DX11", "stages": dict(zip(stages, counts)),
              "parameters": param_count, "material_buffer_bytes": material_size, "nodes": node_count,
              "passes": total_passes, "aliases": alias_count, "clusters": cluster_count,
              "raw_cluster_bytes": len(clusters), "name_crc_checks": crc_checked, "dxbc_length_checks": dxbc_checked}
    links = {"keys": keys[2], "parameters": params,
             "resource_ids": {r[0] for kind in ["Sampler", "Texture"] for r in globals_[kind]}}
    return rows, report, links


def main():
    output, paths = Path(sys.argv[1]), list(map(Path, sys.argv[2:]))
    assert paths
    output.mkdir(parents=True, exist_ok=False)
    all_rows, reports, materials, packages = [], [], [], {}
    manifest_path = Path(__file__).resolve().parent.parent / "tests/fixtures/material-observations.json"
    pinned = {s["sha256"]: s for s in json.loads(manifest_path.read_text())["samples"]} if manifest_path.exists() else {}
    for path in paths:
        assert path.stat().st_size <= 128 * 1024 * 1024
        data = path.read_bytes()
        if data[:4] == b"ShPk":
            rows, report, link = observe_shpk(data)
            packages[path.name] = link
        else:
            rows, report, link = observe_mtrl(data)
            materials.append((path.name, link))
        report.update({"file": str(path), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                       "rows_sha256": hashlib.sha256(("\n".join(rows) + "\n").encode()).hexdigest()})
        if report["sha256"] in pinned:
            assert report["rows_sha256"] == pinned[report["sha256"]]["rows_sha256"], "pinned regression differs"
            report["pinned_regression"] = "equal"
        else:
            report["pinned_regression"] = "unregistered sample"
        all_rows.extend(rows)
        reports.append(report)
    links = []
    for name, material in materials:
        shader = material["shader"]
        if shader not in packages:
            links.append({"material": name, "shader": shader, "status": "shader not supplied"})
            continue
        package = packages[shader]
        keys = {k[0] for k in package["keys"]}
        parameters = {p[0] for p in package["parameters"]}
        missing_keys = [k[0] for k in material["keys"] if k[0] not in keys]
        missing_constants = [c[0] for c in material["constants"] if c[0] not in parameters]
        missing_samplers = [s[0] for s in material["samplers"] if s[0] not in package["resource_ids"]]
        assert not missing_keys and not missing_constants and not missing_samplers
        links.append({"material": name, "shader": shader, "status": "all key/constant/sampler IDs resolve"})
    expected = "\n".join(all_rows) + "\n"
    rust = subprocess.check_output(["target/debug/examples/validate_materials", *map(str, paths)], text=True)
    (output / "rust.tsv").write_text(rust)
    (output / "python.tsv").write_text(expected)
    for i, (actual, target) in enumerate(zip(rust.splitlines(), all_rows)):
        assert actual == target, f"row {i}: Rust {actual!r}, Python {target!r}"
    assert len(rust.splitlines()) == len(all_rows)
    report = {"python": sys.version, "samples": reports, "material_links": links,
              "comparison": "all semantic rows and complete shader/node fingerprints equal"}
    text = json.dumps(report, indent=2) + "\n"
    (output / "report.json").write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()

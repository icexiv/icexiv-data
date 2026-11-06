"""Compare full MDL geometry observations with the Rust API.

Args: new-output-directory extracted-model.mdl ...
Samples must be extracted with the recorded type-3 experiment, outside the repo.
No third-party FFXIV library is imported. Python struct supplies scalar decoding.
"""

import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys


class Fingerprint:
    def __init__(self):
        self.value = 0xcbf29ce484222325

    def mix(self, value):
        for b in struct.pack("<I", value):
            self.value = (self.value ^ b) * 0x100000001b3 & 0xffffffffffffffff

    def __str__(self):
        return f"{self.value:016x}"


def f32_bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def observe(path,require_positions_inside_bounds=True):
    b = path.read_bytes()
    u16 = lambda p: struct.unpack_from("<H", b, p)[0]
    u32 = lambda p: struct.unpack_from("<I", b, p)[0]
    stack, runtime = u32(4), u32(8)
    assert u32(0) == 0x01000006
    p = 68 + stack
    strings = b[p + 8:p + 8 + u32(p + 4)]
    p += 8 + len(strings)
    header = b[p:p + 56]
    mesh_count, attr_count, sub_count, mat_count, bone_count, palette_count, shape_count, sm_count, sv_count = struct.unpack_from("<9H", header, 4)
    element_count, = struct.unpack_from("<H", header, 24)
    terrain_sub_count, = struct.unpack_from("<H", header, 38)
    pool_count, = struct.unpack_from("<H", header, 44)
    p += 56
    elements = p
    p += element_count * 32
    lods = p
    p += 180
    extra = p
    if header[27] & 16:
        p += 120
    meshes = p
    p += mesh_count * 36
    attributes = p
    p += attr_count * 4
    p += header[26] * 20
    submeshes = p
    p += sub_count * 16
    p += terrain_sub_count * 12
    materials = p
    p += mat_count * 4
    bones = p
    p += bone_count * 4
    palettes = p
    p += palette_count * 4 + pool_count * 2
    shapes = p
    p += shape_count * 16
    shape_meshes = p
    p += sm_count * 12
    shape_values = p
    p += sv_count * 4
    submap_length = u32(p)
    submap = p + 4
    p += 4 + submap_length
    additional_boxes, = struct.unpack_from("<H", header, 36)
    bounds = 68 + stack + runtime - (4 + bone_count + additional_boxes) * 32
    assert bounds > p
    string = lambda offset: strings[offset:strings.index(0, offset)]
    out = [f"file\t{len(b)}\t{mesh_count}\t{mat_count}\t{bone_count}\t{shape_count}\t{sm_count}\t{sv_count}"]
    for label, start, count in [("attribute", attributes, attr_count), ("material", materials, mat_count), ("bone", bones, bone_count)]:
        for i in range(count):
            out.append(f"{label}\t{i}\t{string(u32(start + i * 4)).hex()}")
    palette_sizes = []
    for i in range(palette_count):
        offset, count = struct.unpack_from("<HH", b, palettes + i * 4)
        indices = struct.unpack_from(f"<{count}H", b, palettes + i * 4 + offset * 4)
        assert all(index < bone_count for index in indices)
        palette_sizes.append(count)
        out.append(f"palette\t{i}\t" + ",".join(map(str, indices)))
    for i in range(sub_count):
        values = struct.unpack_from("<IIIHH", b, submeshes + i * 16)
        out.append(f"submesh\t{i}\t" + "\t".join(map(str, values)))
    for i in range(4 + bone_count + additional_boxes):
        values = struct.unpack_from("<8I", b, bounds + i * 32)
        out.append(f"bounds\t{i}\t" + ",".join(f"{v:08x}" for v in values))
    for i in range(element_count):
        at = elements + i * 32
        values = ",".join(f"{v:08x}" for v in struct.unpack_from("<6I", b, at + 8))
        out.append(f"element\t{u32(at)}\t{string(u32(at + 4)).hex()}\t{values}")
    shape_ranges = []
    for i in range(shape_count):
        at = shapes + i * 16
        starts = struct.unpack_from("<3H", b, at + 4)
        counts = struct.unpack_from("<3H", b, at + 10)
        shape_ranges.append((starts, counts))
        out.append(f"shape\t{i}\t{string(u32(at)).hex()}\t" + ",".join(map(str, starts)) + "\t" + ",".join(map(str, counts)))
    for i in range(sm_count):
        target, count, offset = struct.unpack_from("<III", b, shape_meshes + i * 12)
        assert offset + count <= sv_count
        fingerprint = Fingerprint()
        for n in range(offset, offset + count):
            base, replacement = struct.unpack_from("<HH", b, shape_values + n * 4)
            fingerprint.mix(base)
            fingerprint.mix(replacement)
        out.append(f"replacement\t{i}\t{target}\t{count}\t{offset}\t{fingerprint}")
    type_counts = {}
    role_counts = [0] * 9
    mesh_total = vertex_total = index_total = eight_influence_vertices = 0
    shape_occurrence_witness = None
    weight_sums = set()
    position_outside_bounds = 0
    min_bounds = struct.unpack_from("<4f", b, bounds)
    max_bounds = struct.unpack_from("<4f", b, bounds + 16)
    for lod in range(b[64]):
        l = lods + lod * 60
        ranges = [struct.unpack_from("<HH", b, l + (0 if r == 0 else 12 + (r - 1) * 4)) for r in range(5)]
        ranges += [struct.unpack_from("<HH", b, extra + lod * 40 + (r - 5) * 4) if header[27] & 16 else (0, 0) for r in range(5, 9)]
        for r, (_, count) in enumerate(ranges):
            role_counts[r] += count
        vo, io = u32(16 + lod * 4), u32(28 + lod * 4)
        assert u32(l + 52) == vo and u32(l + 56) == io
        mesh_ids = sorted({m for r, (start, count) in enumerate(ranges) if r != 3 for m in range(start, start + count)})
        for m in mesh_ids:
            vcount, _, icount, material, substart, subcount, palette, istart, *rest = struct.unpack_from("<HHIHHHHI3I4B", b, meshes + m * 36)
            offsets, strides, flags = rest[:3], rest[3:6], rest[6]
            streams = flags & 3
            assert material < mat_count and streams <= 3
            fingerprint = Fingerprint()
            declarations = []
            for j in range(17):
                e = tuple(b[68 + m * 136 + j * 8:68 + m * 136 + j * 8 + 5])
                if e[0] == 255:
                    break
                declarations.append(e)
                stream, offset, kind, usage, usage_index = e
                type_counts[kind] = type_counts.get(kind, 0) + 1
                for n in e:
                    fingerprint.mix(n)
                for v in range(vcount):
                    at = vo + offsets[stream] + v * strides[stream] + offset
                    if kind in (0, 1, 2, 3):
                        value = list(struct.unpack_from(f"<{kind + 1}f", b, at))
                    elif kind in (13, 14):
                        value = list(struct.unpack_from(f"<{2 if kind == 13 else 4}e", b, at))
                    elif kind in (5, 8, 17):
                        value = list(b[at:at + (8 if kind == 17 else 4)])
                        if kind == 8:
                            value = [x / 255 for x in value]
                    else:
                        raise ValueError(f"unobserved scalar type {kind}")
                    assert all(math.isfinite(x) for x in value)
                    if usage == 0 and usage_index == 0:
                        position_outside_bounds += int(any(value[a] < min_bounds[a] - 0.002 or value[a] > max_bounds[a] + 0.002 for a in range(min(3, len(value)))))
                    if usage == 1 and kind in (5, 17):
                        weight_sums.add(sum(value))
                        if kind == 17:
                            eight_influence_vertices += 1
                    for x in value + [0] * (8 - len(value)):
                        fingerprint.mix(f32_bits(x))
            for v in range(vcount):
                weights = next((e for e in declarations if e[3:] == (1, 0)), None)
                indices = next((e for e in declarations if e[3:] == (2, 0)), None)
                if weights and indices:
                    n = 8 if weights[2] == 17 else 4
                    w_at = vo + offsets[weights[0]] + v * strides[weights[0]] + weights[1]
                    i_at = vo + offsets[indices[0]] + v * strides[indices[0]] + indices[1]
                    assert all(b[i_at + a] < palette_sizes[palette] for a in range(n) if b[w_at + a] != 0)
            index_values = struct.unpack_from(f"<{icount}H", b, io + istart * 2)
            assert all(index < vcount for index in index_values)
            for value in index_values:
                fingerprint.mix(value)
            for s in range(substart, substart + subcount):
                start, count, _, bs, bc = struct.unpack_from("<IIIHH", b, submeshes + s * 16)
                assert istart <= start and start + count <= istart + icount
                assert bc == 0 or (bs + bc) * 2 <= submap_length
            for starts, counts in shape_ranges:
                for sm in range(starts[lod], starts[lod] + counts[lod]):
                    target, count, offset = struct.unpack_from("<III", b, shape_meshes + sm * 12)
                    if target != istart:
                        continue
                    for n in range(offset, offset + count):
                        base, replacement = struct.unpack_from("<HH", b, shape_values + n * 4)
                        assert base < icount and replacement < vcount
                        if base >= vcount and shape_occurrence_witness is None:
                            shape_occurrence_witness = {"lod": lod, "mesh": m, "base_index_occurrence": base,
                                                        "mesh_vertex_count": vcount, "source_vertex": index_values[base],
                                                        "replacement_vertex": replacement}
            out.append(f"mesh\t{lod}\t{m}\t{vcount}\t{icount}\t{material}\t{substart}\t{subcount}\t{palette}\t{fingerprint}")
            mesh_total += 1
            vertex_total += vcount
            index_total += icount
    if require_positions_inside_bounds:assert position_outside_bounds == 0
    assert weight_sums <= {0, 255}
    report = {"file": str(path), "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest(),
              "meshes": mesh_total, "vertices": vertex_total, "indices": index_total,
              "element_ids": element_count, "shape_values": sv_count, "types": type_counts,
              "roles": role_counts, "eight_influence_vertices": eight_influence_vertices,
              "byte_weight_sums": sorted(weight_sums), "position_outside_bounds": position_outside_bounds,
              "runtime_tail_bytes": bounds - p, "additional_boxes": additional_boxes,
              "shape_occurrence_witness": shape_occurrence_witness}
    return out, report


def main():
    output = Path(sys.argv[1])
    paths = list(map(Path, sys.argv[2:]))
    assert paths
    output.mkdir(parents=True, exist_ok=False)
    expected, reports = [], []
    manifest = json.loads((Path(__file__).resolve().parent.parent / "tests/fixtures/mdl-observations.json").read_text())
    pinned = {sample["sha256"]: sample for sample in manifest["samples"]}
    for path in paths:
        rows, report = observe(path)
        rows_hash = hashlib.sha256(("\n".join(rows) + "\n").encode()).hexdigest()
        report["rows_sha256"] = rows_hash
        if report["sha256"] in pinned:
            previous = pinned[report["sha256"]]
            assert rows_hash == previous["rows_sha256"], "pinned semantic regression differs"
            for field in ["meshes", "vertices", "indices", "shape_values"]:
                assert report[field] == previous[field]
            report["pinned_regression"] = "equal"
        else:
            report["pinned_regression"] = "unregistered sample"
        expected.extend(rows)
        reports.append(report)
    rust = subprocess.check_output(["target/debug/examples/validate_mdl", *map(str, paths)], text=True)
    (output / "rust.tsv").write_text(rust)
    (output / "python.tsv").write_text("\n".join(expected) + "\n")
    actual = rust.splitlines()
    assert len(actual) == len(expected)
    for i, (a, e) in enumerate(zip(actual, expected)):
        assert a == e, f"row {i}: Rust {a!r}, Python {e!r}"
    report = {"python": sys.version, "samples": reports, "comparison": "all semantic rows and full scalar/index fingerprints equal"}
    text = json.dumps(report, indent=2) + "\n"
    (output / "report.json").write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()

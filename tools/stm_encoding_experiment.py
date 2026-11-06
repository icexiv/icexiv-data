"""Re-encode one real template with value-preserving uniform/dense columns.

Args: new-output-directory legacy.stm dawntrail.stm. Does not modify sources.
For IDs 1..253 every half bit is preserved. Dense slot 254 is an explicitly
chosen zero boundary control, not claimed to be an observed game stain value.
"""
import hashlib
import json
from pathlib import Path
import struct
import sys


def experiment(path, output):
    b = path.read_bytes()
    _, version, count, colors, scalars = struct.unpack_from("<HHHBB", b)
    columns = 5 if version == 0x0101 else 12
    identity, = struct.unpack_from("<I", b, 8)
    offset, = struct.unpack_from("<I", b, 8 + count * 4)
    start = 8 + count * 8 + offset * 2
    ends = struct.unpack_from(f"<{columns}H", b, start)
    base, previous, payloads = start + columns * 2, 0, []
    dense, uniform = None, None
    for column, end in enumerate(ends):
        raw = b[base + previous * 2:base + end * 2]
        previous = end
        stride = 6 if column < 3 else 2
        if raw:
            values, selectors = raw[:-254], raw[-254:]
            assert selectors[0] == 255 and len(values) % stride == 0
            palette = [bytes(stride)] + [values[i:i + stride] for i in range(0, len(values), stride)]
            if column == 0:
                replacement = b"".join(palette[s] for s in selectors[1:]) + bytes(stride)
                assert len(replacement) == stride * 254
                assert all(replacement[(i - 1) * stride:i * stride] == palette[selectors[i]] for i in range(1,254))
                raw, dense = replacement, column
            elif column >= 3 and uniform is None and len(set(selectors[1:])) == 1:
                replacement = palette[selectors[1]]
                assert all(palette[s] == replacement for s in selectors[1:])
                raw, uniform = replacement, column
        payloads.append(raw)
    assert dense is not None and uniform is not None
    body, length, new_ends = bytearray(), 0, []
    for raw in payloads:
        body.extend(raw); length += len(raw); assert length % 2 == 0
        new_ends.append(length // 2)
    controlled = struct.pack("<HHHBBII",0x534d,version,1,colors,scalars,identity,0)
    controlled += struct.pack(f"<{columns}H",*new_ends) + body
    target = output / path.name
    with target.open("xb") as f: f.write(controlled)
    return {"source":str(path),"source_sha256":hashlib.sha256(b).hexdigest(),
            "controlled":str(target),"controlled_sha256":hashlib.sha256(controlled).hexdigest(),
            "template_id":identity,"dense_column":dense,"uniform_column":uniform,
            "equivalence":"all original half bits for stain IDs 1..253", "dense_slot_254":"zero boundary control"}


def main():
    output = Path(sys.argv[1]); output.mkdir(parents=True,exist_ok=False)
    report = {"experiments":[experiment(Path(p),output) for p in sys.argv[2:]]}
    text = json.dumps(report,indent=2) + "\n"; (output / "report.json").write_text(text); print(text,end="")


if __name__ == "__main__": main()

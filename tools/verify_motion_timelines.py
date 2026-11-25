"""Compare inspect_motion rows to an independent read of four extracted PAPs.

Usage: python tools/verify_motion_timelines.py <extracted-directory> <inspect_motion>
"""
import hashlib
from pathlib import Path
import struct
import subprocess
import sys


SAMPLES = {
    "bt_common_resident-idle": "96c29c780bad9d7cbccc2fe012ca2b1b34a950ca91df3e8b7180bc7dc1695d37",
    "event_base_chair_sit_relax": "c9805e3a243a6d121105c8de71368dc5089e772ae1dd326cdd8f9ba5102a6d96",
    "move_a": "89e733a21e3c47b5664435bc71e39b71eb8146c51c7fa0eca6feced73beb19ba",
    "sit": "5c6b8c2673d33803822704f5655e7f1614da681a68731536426c09635689388a",
}


def observe(b):
    count, = struct.unpack_from("<H", b, 8)
    records, _, timeline = struct.unpack_from("<III", b, 14)
    names = [b[i:i+32].split(b"\0")[0] for i in range(records, records + count * 40, 40)]
    start = timeline
    rows, observed_names = [], []
    while start < len(b):
        magic, size, n = struct.unpack_from("<4sII", b, start)
        assert magic == b"TMLB"
        at = start + 12
        for _ in range(n):
            tag, length = struct.unpack_from("<4sI", b, at)
            base = at + 8
            row = [start - timeline, at - start, tag.decode()]
            if tag in (b"TMAL", b"TMAC", b"TMTR"):
                field = {b"TMAL": 0, b"TMAC": 12, b"TMTR": 4}[tag]
                off, number = struct.unpack_from("<II", b, base + field)
                row.extend(struct.unpack_from(f"<{number}H", b, base + off))
            if tag == b"C009":
                off, = struct.unpack_from("<I", b, base + 12)
                name = b[base+off:b.index(0, base+off, start+size)]
                observed_names.append(name)
                row.append(name.decode())
            rows.append("\t".join(map(str, row)))
            at += length
        start = (start + size + 3) & ~3
    assert observed_names == names
    return rows


if __name__ == "__main__":
    folder, exe = map(Path, sys.argv[1:])
    for name, sha in SAMPLES.items():
        path = folder / (name + ".pap")
        b = path.read_bytes()
        if hashlib.sha256(b).hexdigest() != sha:
            raise ValueError(f"{path}: unexpected sample")
        expected = observe(b)
        actual = subprocess.check_output([exe.resolve(), path], text=True).splitlines()
        if actual != expected:
            raise ValueError(f"{path}: timeline rows differ")
        print(name, len(actual), "entries matched")

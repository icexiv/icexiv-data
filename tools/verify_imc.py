"""Compare Rust IMC output with byte-level observations from installed samples.

Build local_sqpack and validate_imc first. Arguments: sqpack-root output-directory.
The directory must not exist; outputs never target the installation.
"""

import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    root, output = map(Path, sys.argv[1:])
    if output.resolve().is_relative_to(root.resolve().parent):
        raise ValueError("output must be outside the game installation")
    output.mkdir(parents=True, exist_ok=False)
    paths = [
        "chara/weapon/w2001/obj/body/b0102/b0102.imc",
        "chara/equipment/e0001/e0001.imc",
        "chara/equipment/e0600/e0600.imc",
    ]
    observed = []
    expected = []
    for index, path in enumerate(paths):
        target = output / f"{index}.imc"
        extraction = subprocess.check_output(
            ["target/debug/examples/local_sqpack", str(root), path, str(target)],
            text=True,
        )
        (output / f"{index}-extraction.txt").write_text(extraction)
        data = target.read_bytes()
        # Independent sequential unpack, not an invocation of the Rust parser.
        count, mask = struct.unpack_from("<HH", data)
        records = list(struct.iter_unpack("<BBHBB", data[4:]))
        parts = [bit for bit in range(16) if mask & (1 << bit)]
        assert len(records) == (count + 1) * len(parts)
        expected.append(f"file\t{path}\t{len(data)}\t{count}\t{mask:#06x}")
        for ordinal, (material, decal, flags, vfx, animation) in enumerate(records):
            subset, part_index = divmod(ordinal, len(parts))
            values = [subset, parts[part_index], material, decal, flags,
                      flags % 1024, flags // 1024, vfx, animation]
            expected.append("entry\t" + "\t".join(map(str, values)))
        observed.append({
            "path": path, "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "variant_count": count, "part_mask": mask,
            "records": len(records),
            "nonzero_decal": sum(r[1] != 0 for r in records),
            "nonzero_vfx": sum(r[3] != 0 for r in records),
            "nonzero_animation": sum(r[4] != 0 for r in records),
        })
    rust = subprocess.check_output(
        ["target/debug/examples/validate_imc", str(root), *paths], text=True,
    )
    (output / "rust.tsv").write_text(rust)
    assert rust.splitlines() == expected, "Rust / independent byte observations differ"
    version = (root.parent / "ffxivgame.ver").read_text().strip()
    report = {"version": version, "platform": "Win32", "samples": observed,
              "comparison": "all records equal", "python": sys.version}
    text = json.dumps(report, indent=2) + "\n"
    (output / "report.json").write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()

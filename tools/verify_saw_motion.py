"""Verify the offline saw export: RGB samples and two explicit owner frames.

Args: export manifest, content-addressed object directory, examples binary dir.
"""
import ast
import bisect
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def blocks(data):
    i = 0
    while i < len(data):
        k, n = struct.unpack_from('<4sI', data, i)
        yield k[::-1].strip(b'\0'), data[i+8:i+8+n]
        i += 8 + ((n+3) & ~3)
    assert i == len(data)


def main(manifest, objects, exe):
    files = json.loads(Path(manifest).read_text())['files']
    def asset(suffix):
        sha, = [h for path, h in files.items() if path.endswith(suffix)]
        path = Path(objects) / sha[:2] / sha[2:]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == sha
        return path
    fx, mdl, sklb = map(asset, ('.avfx', '.mdl', '.sklb'))
    curves = []
    for name, particle in blocks(fx.read_bytes()[8:]):
        if name != b'Ptcl':
            continue
        color = dict(blocks(dict(blocks(particle))[b'Col']))
        fields = dict(blocks(color[b'RGB']))
        pre, post = (int.from_bytes(fields[k], 'little') for k in (b'BvPr', b'BvPo'))
        keys = list(struct.iter_unpack('<hHfff', fields[b'Keys']))
        assert all(k[1] == 1 for k in keys)
        curves.append((keys, pre, post))
    assert len(curves) == 12
    times = {-1., 0., 35., 180.5, 500.}
    for keys, _, _ in curves:
        times.update(k[0] for k in keys)
        times.update((a[0]+b[0])/2 for a, b in zip(keys, keys[1:]))
    error = 0.
    for t in sorted(times):
        out = subprocess.check_output([exe/'inspect_motion', fx, str(t)], text=True)
        colors = {int(row[1]): list(map(float, row[3:])) for line in out.splitlines()
                  if (row := line.split('\t'))[0] == 'rgb'}
        assert len(colors) == len(curves)
        for i, (keys, pre, post) in enumerate(curves):
            f = t
            lo, hi = keys[0][0], keys[-1][0]
            if lo == hi:
                want = keys[0][2:]
            else:
                if f <= lo:
                    f = hi - ((lo-f) % (hi-lo)) if pre else lo
                elif f >= hi:
                    f = lo + ((f-hi) % (hi-lo)) if post else hi
                right = min(max(1, bisect.bisect_left([k[0] for k in keys], f)), len(keys)-1)
                a, b = keys[right-1:right+1]
                weight = (f-a[0])/(b[0]-a[0])
                want = [(1-weight)*x + weight*y for x,y in zip(a[2:], b[2:])]
            error = max(error, *(abs(x-y) for x,y in zip(want,colors[i])))
    assert error < 2e-6, error
    print(len(times)*len(curves), "RGB samples, maximum absolute error", error)
    for frame, axes, position, scale in [
        ('1,0,0,0,1,0,0,0,1,0,0,0', [1,0,0,0,1,0,0,0,1], [0,.18621668,0], [1,1,1]),
        ('0,2,0,-3,0,0,0,0,4,7,8,9', [0,1,0,-1,0,0,0,0,1], [7-3*.18621668,8,9], [2,3,4]),
    ]:
        fields = subprocess.check_output([exe/'weapon_anchor',fx,mdl,sklb,frame],text=True).strip().split('\t')
        assert fields[:3] == ['3','1','n_hara']
        bx, p, s = map(ast.literal_eval, fields[3:])
        got = [c for v in bx for c in v]+p+s
        assert max(abs(x-y) for x,y in zip(got,axes+position+scale)) < 2e-6
    print("2 owner frames, unique ElementId and bone match")


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2], Path(sys.argv[3]))

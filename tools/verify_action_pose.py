"""Check the idle.tmb -> c0101 idle PAP/SKLB example against independent matrices.

Args: idle.tmb idle.pap base.sklb action_pose_executable
"""
import ast
import subprocess
import sys
from observe_spline import decode, sample_track
from observe_tagfile import read


def local_matrix(trs):
    tx, ty, tz, x, y, z, w, sx, sy, sz = trs
    k = 2 / sum(v*v for v in (x, y, z, w))
    return [
        [(1-k*(y*y+z*z))*sx, k*(x*y-z*w)*sy, k*(x*z+y*w)*sz, tx],
        [k*(x*y+z*w)*sx, (1-k*(x*x+z*z))*sy, k*(y*z-x*w)*sz, ty],
        [k*(x*z-y*w)*sx, k*(y*z+x*w)*sy, (1-k*(x*x+y*y))*sz, tz],
        [0., 0., 0., 1.],
    ]


def container(o):
    c, = [v['fields'] for v in o.objects if v and
          o.classes[v['class']]['name'] == b'hkaAnimationContainer']
    return c


def main(tmb, pap, sklb, exe):
    p, s = read(pap), read(sklb)
    # The retained cbnm_id0 record has Havok index 1. Take the container
    # references, never the order of objects as serialized.
    binding = p.objects[container(p)[b'bindings'][1]['ref']]['fields']
    fields = p.objects[binding[b'animation']['ref']]['fields']
    blocks = decode(fields)
    sk = s.objects[container(s)[b'skeletons'][0]['ref']]['fields']
    bone, = [i for i, b in enumerate(sk[b'bones']) if b['fields'][b'name'] == b'n_buki_r']
    largest = 0
    for time in (0, 125, 500, 1000, 2500):
        pose = [tuple(v[:3]+v[4:8]+v[8:11]) for v in sk[b'referencePose']]
        frame = min(time/1000, fields[b'duration']) / fields[b'frameDuration']
        stride = fields[b'maxFramesPerBlock']-1
        block = min(int(frame//stride), len(blocks)-1)
        for track, dest in enumerate(binding[b'transformTrackToBoneIndices']):
            if dest >= 0:
                pose[dest] = sample_track(blocks[block]['tracks'][track], frame-block*stride)
        i = bone
        m = local_matrix(pose[i])
        while sk[b'parentIndices'][i] >= 0:
            i = sk[b'parentIndices'][i]
            a = local_matrix(pose[i])
            m = [[sum(a[r][j]*m[j][c] for j in range(4)) for c in range(4)] for r in range(4)]
        key, idx, target, _, vectors = subprocess.check_output(
            [exe, tmb, pap, sklb, 'n_buki_r', str(time)], text=True).strip().split('\t')
        assert (key, idx, int(target)) == ('cbnm_id0', '1', bone)
        got = ast.literal_eval(vectors)
        largest = max(largest, *(abs(m[r][c]-got[c][r]) for r in range(3) for c in range(4)))
    assert largest < 2e-6, largest
    print(f"5 poses, 60 matrix lanes; maximum absolute error {largest:.3g}")


if __name__ == '__main__':
    main(*sys.argv[1:])

"""Named cube-face byte views, not a cubemap renderer or coordinate transform.

Args: extracted-cube-rgba4.tex extracted-cube-bc7.tex new-output-directory.
The face identities use the separately reviewed D3D enum; native initialization
dataflow is checked by verify_client_texture_groups.py, not emulated here.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    first, second, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo)
    assert not out.is_relative_to(Path.home()/'Games/ffxiv')
    pin = json.loads((repo/'tests/fixtures/tex-observations.json').read_text())
    sources = [next(s for s in pin['samples'] if s['file']==name)
        for name in ['cube-rgba4.tex','cube-bc7-mips.tex']]
    names = ['PositiveX','NegativeX','PositiveY','NegativeY','PositiveZ','NegativeZ']
    out.mkdir(parents=True, exist_ok=False)
    actual = subprocess.check_output(['target/debug/examples/validate_cube_faces',str(out/'faces'),str(first),str(second)],text=True,timeout=120)
    rows, reports = [], []
    for file, path in enumerate((first,second)):
        assert path.stat().st_size <= 64 << 20
        b = path.read_bytes()
        digest = hashlib.sha256(b).hexdigest()
        assert digest == sources[file]['sha256']
        flags, fmt, w, h, depth, mip_flags, count = struct.unpack_from('<II3HBB',b)
        assert flags&0x13c00000==0x02000000 and depth==1 and count in (0,6)
        mips = mip_flags&127
        assert 0 < mips <= 13 and fmt in (0x1440,0x6432)
        offsets = struct.unpack_from('<13I',b,28)
        faces = []
        for mip in range(mips):
            width,height=max(1,w>>mip),max(1,h>>mip)
            size = width*height*2 if fmt==0x1440 else ((width+3)//4)*((height+3)//4)*16
            assert offsets[mip]+6*size<=len(b)
            for face, name in enumerate(names):
                start=offsets[mip]+face*size
                data=b[start:start+size]
                assert (out/f'faces/{file}-{mip}-{face}.bin').read_bytes()==data
                rows.append(f'face\t{file}\t{mip}\t{name}\t{face}\t{size}')
                faces.append({'mip':mip,'face':name,'file_offset':start,'bytes':size,
                    'sha256':hashlib.sha256(data).hexdigest()})
        reports.append({'sha256':digest,'header_array_byte':count,'mips':mips,'faces':faces,
            'distinct_base_faces':len({f['sha256']for f in faces if f['mip']==0})})
    expected='\n'.join(rows)+'\n'
    assert actual==expected
    result={'version':pin['version'],'platform':pin['platform'],'sources':reports,
        'faces_verified':len(rows),'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),
        'limitations':'Named D3D face identities and unchanged encoded bytes. Static initialization linkage is separate; no native execution, world-axis interpretation, UV rotation/flip, cube-array profile or GPU sampling claim.'}
    text=json.dumps(result,indent=2)+'\n'
    fixture=repo/'tests/fixtures/cube-face-observations.json'
    assert hashlib.sha256(text.encode()).hexdigest()==json.loads(fixture.read_text())['report_sha256']
    (out/'report.json').write_text(text)
    (out/'rows.rust.tsv').write_text(actual)
    (out/'rows.python.tsv').write_text(expected)
    print(text,end='')


if __name__=='__main__':
    main()

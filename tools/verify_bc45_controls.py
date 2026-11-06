"""All UNORM BC4/BC5 endpoint pairs and selectors against generic Pillow DDS.

Args: new-output-directory. Generated TEX payloads are controlled data, not
game resources. Every ordered endpoint pair appears once in each block channel.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import PIL
from verify_tex import observe


def main():
    out = Path(sys.argv[1]).resolve()
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    files = []
    forward = sum((i % 8) << (i*3) for i in range(16)).to_bytes(6,'little')
    reverse = sum((7-i % 8) << (i*3) for i in range(16)).to_bytes(6,'little')
    for fmt in (0x6120,0x6230):
        data = bytearray(80)
        struct.pack_into('<II3HBB',data,0,0x800000,fmt,1024,1024,1,1,0)
        struct.pack_into('<I',data,28,80)
        for a in range(256):
            for b in range(256):
                data.extend(bytes([a,b])+forward)
                if fmt == 0x6230:
                    data.extend(bytes([b,a])+reverse)
        path = out/f'{fmt:04x}.tex'
        path.write_bytes(data)
        files.append(path)
    rust = subprocess.check_output(['target/debug/examples/validate_tex',str(out/'pixels'),*map(str,files)],text=True,timeout=120)
    expected, reports = [], []
    for i,path in enumerate(files):
        data = path.read_bytes()
        rows, report = observe(data,i,out/'pixels')
        for channel in report['bc45_encoded_block_channels']:
            assert channel['endpoint_gt']==32640 and channel['endpoint_le']==32896 and channel['endpoint_equal']==256
            assert channel['selectors']==[131072]*8
        report.update({'sha256':hashlib.sha256(data).hexdigest(),
            'rows_sha256':hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest()})
        expected.extend(rows)
        reports.append(report)
    oracle = '\n'.join(expected)+'\n'
    assert rust==oracle
    (out/'rust.tsv').write_text(rust)
    (out/'python.tsv').write_text(oracle)
    result = {'pillow':PIL.__version__,'endpoint_pairs_per_channel':65536,'samples':reports,
        'limitations':'Controlled 2D BC4/BC5 UNORM with exhaustive endpoint pairs and all selectors, not arbitrary selector arrangements or actual game-generated files.'}
    text = json.dumps(result,indent=2)+'\n'
    pin = json.loads((repo/'tests/fixtures/bc45-control-observations.json').read_text())
    assert hashlib.sha256(text.encode()).hexdigest()==pin['report_sha256']
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__=='__main__':
    main()

"""All named equipment/accessory IMC records, including nonzero decal/animation.

Args: game/sqpack new-output-dir. Repeats bounded discovery, independently
reconstructs and expands all selected files, compares every Rust query output.
No native execution, animation evaluation or path fallback is performed.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import contextlib
from find_vfx_samples import standard


def main():
    root,out=[Path(p).resolve() for p in sys.argv[1:]]
    repo=Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True,exist_ok=False)
    subprocess.check_call([sys.executable,str(repo/'tools/find_imc_variants.py'),str(root),str(out/'discovery')],stdout=subprocess.DEVNULL,timeout=120)
    discovered=(out/'discovery/report.json').read_bytes()
    discovery=json.loads(discovered)
    expected=[]
    witnesses=[]
    with contextlib.ExitStack() as stack:
        files={}
        for sample in discovery['files']:
            dat=sample['dat']
            if dat not in files:
                files[dat]=stack.enter_context((root/dat).open('rb'))
            raw=standard(files[dat],sample['offset'],max_bytes=1<<20)
            assert hashlib.sha256(raw).hexdigest()==sample['sha256']
            count,mask=struct.unpack_from('<HH',raw)
            parts=[bit for bit in range(16) if mask>>bit&1]
            expected.append(f'file\t{sample["path"]}\t{len(raw)}\t{count}\t{mask:#06x}')
            for ordinal,(material,decal,flags,vfx,animation) in enumerate(struct.iter_unpack('<BBHBB',raw[4:])):
                subset,part_index=divmod(ordinal,len(parts))
                part=parts[part_index]
                values=[subset,part,material,decal,flags,flags%1024,flags//1024,vfx,animation]
                expected.append('entry\t'+'\t'.join(map(str,values)))
                expected.append(f'animation\t{subset}\t{part}\t{animation%16}')
                if decal or animation:
                    witnesses.append({'path':sample['path'],'subset':subset,'part':part,
                        'material_id':material,'decal_id':decal,'vfx_id':vfx,'animation_byte':animation,
                        'animation_id':animation%16})
    oracle='\n'.join(expected)+'\n'
    actual=subprocess.check_output(['target/debug/examples/validate_imc',str(root),'--animation-fields',
        *[s['path'] for s in discovery['files']]],text=True,timeout=120)
    assert actual==oracle
    (out/'python.tsv').write_text(oracle)
    (out/'rust.tsv').write_text(actual)
    result={'version':discovery['version'],'platform':'Win32','discovery_sha256':hashlib.sha256(discovered).hexdigest(),
        'files_verified':discovery['files_examined'],'records_verified':discovery['records_examined'],
        'nonzero_records':discovery['nonzero_records'],'rows_sha256':hashlib.sha256(oracle.encode()).hexdigest(),
        'nonzero_decal_animation_witnesses':witnesses,
        'limitations':'Named ID range and one index only; all file/query fields, not animation/decal playback or full path fallback.'}
    text=json.dumps(result,indent=2)+'\n'
    pin=json.loads((repo/'tests/fixtures/imc-variant-observations.json').read_text())
    assert hashlib.sha256(text.encode()).hexdigest()==pin['report_sha256']
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__=='__main__':
    main()

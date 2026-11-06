"""Read-only named equipment/accessory IMC field discovery.

Args: game/sqpack new-output-dir. Checks 19998 candidate paths, at most 5000
actual files, 1 MiB per decoded file and 64 MiB cumulative decoded input.
No path fallback or animation/decal application is inferred.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from find_vfx_samples import standard


def main():
    root, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    index = root/'ffxiv/040000.win32.index'
    with index.open('rb') as f:
        f.seek(12)
        hs, = struct.unpack('<I',f.read(4))
        assert hs <= 2 << 20
        f.seek(hs+8)
        offset,size = struct.unpack('<II',f.read(8))
        assert size <= 64 << 20 and size % 16 == 0
        f.seek(offset)
        table = f.read(size)
        assert len(table)==size
    locations = {key: (n,p) for n,(key,p,_) in enumerate(struct.iter_unpack('<QII',table)) if not p&1}
    jam = lambda text: ~zlib.crc32(text.encode('ascii')) & 0xffffffff
    examined=paths=total=records=0
    found=[]
    catalog=[]
    nonzero={'decal':0,'vfx':0,'animation':0,'animation_high':0}
    representatives={}
    observations=hashlib.sha256()
    with contextlib.ExitStack() as stack:
        files={}
        for domain,prefix in [('equipment','e'),('accessory','a')]:
            for identity in range(1,10000):
                paths+=1
                path=f'chara/{domain}/{prefix}{identity:04d}/{prefix}{identity:04d}.imc'
                directory,name=path.rsplit('/',1)
                key=jam(directory)<<32|jam(name)
                if key not in locations:
                    continue
                if examined==5000:
                    break
                ordinal,packed=locations[key]
                dat=f'ffxiv/040000.win32.dat{(packed>>1)&7}'
                if dat not in files:
                    files[dat]=stack.enter_context((root/dat).open('rb'))
                entry=(packed>>4)*128
                raw=standard(files[dat],entry,max_bytes=1<<20)
                assert raw is not None
                examined+=1
                total+=len(raw)
                assert total <= 64<<20
                count,mask=struct.unpack_from('<HH',raw)
                assert mask and len(raw)==4+(count+1)*mask.bit_count()*6
                rows=list(struct.iter_unpack('<BBHBB',raw[4:]))
                records+=len(rows)
                occurrences={'decal':sum(r[1]!=0 for r in rows),'vfx':sum(r[3]!=0 for r in rows),
                    'animation':sum(r[4]!=0 for r in rows),'animation_high':sum(r[4]>>4!=0 for r in rows)}
                digest=hashlib.sha256(raw).hexdigest()
                catalog.append({'path':path,'dat':dat,'offset':entry,'index_ordinal':ordinal,
                    'index_hash':f'{key:016x}','sha256':digest})
                observations.update(path.encode()+b'\0'+bytes.fromhex(digest))
                for field,value in occurrences.items():
                    nonzero[field]+=value
                interesting=[field for field,value in occurrences.items() if value and field not in representatives]
                if interesting:
                    file=f'{len(found)}.imc'
                    (out/file).write_bytes(raw)
                    report={'path':path,'file':file,'index_ordinal':ordinal,'index_hash':f'{key:016x}',
                        'dat':dat,'offset':entry,'sha256':digest,'bytes':len(raw),'variants':count,
                        'part_mask':mask,'records':len(rows),'nonzero':occurrences}
                    found.append(report)
                    for field in interesting:
                        representatives[field]=file
            if examined==5000:
                break
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32',
        'index_table_sha256':hashlib.sha256(table).hexdigest(),'candidate_paths':paths,
        'files_examined':examined,'decoded_bytes':total,'records_examined':records,
        'file_observations_sha256':observations.hexdigest(),'nonzero_records':nonzero,'samples':found,'files':catalog,
        'limitations':'Named equipment/accessory paths with numeric IDs 1..9999 in one character index; bounded discovery, not complete installation or reference/application semantics.'}
    text=json.dumps(result,indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__=='__main__':
    main()

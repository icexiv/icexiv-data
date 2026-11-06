"""Bounded readonly furniture SGB path discovery. Args: sqpack new-output-dir.

Probe indoor/outdoor general IDs 1..2000 from one base bgcommon index. Read
only indexed type-2 resources; stop after three structurally referenced SGBs.
"""
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from find_vfx_samples import standard
from observe_sgb import observe


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    index=root/'ffxiv/010000.win32.index'
    with index.open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);entries={k:(p,i)for i,(k,p,_)in enumerate(struct.iter_unpack('<QII',f.read(size)))if not p&1}
    findings=[];inspected=0
    for domain,prefix in [('indoor','fun'),('outdoor','gar')]:
        for n in range(1,2001):
            path=f'bgcommon/hou/{domain}/general/{n:04}/asset/{prefix}_b0_m{n:04}.sgb';directory,filename=path.rsplit('/',1)
            jam=lambda s:~zlib.crc32(s.encode())&0xffffffff
            key=jam(directory)<<32|jam(filename)
            if key not in entries:continue
            packed,ordinal=entries[key];dat=f'ffxiv/010000.win32.dat{(packed>>1)&7}';entry=(packed>>4)*128
            with (root/dat).open('rb')as f:raw=standard(f,entry)
            if raw is None:continue
            inspected+=1;o=observe(raw)
            refs=[p.decode('ascii')for l in o['layers']for i in l['instances']for kind,p in i['paths']if kind=='shared_group'and p]
            if not refs:continue
            name=f'{prefix}-{n:04}.sgb';(out/name).write_bytes(raw)
            findings.append({'path':path,'dat':dat,'offset':entry,'index_ordinal':ordinal,'index_hash':f'{key:016x}','file':name,'sha256':hashlib.sha256(raw).hexdigest(),'shared_group_paths':refs})
            if len(findings)>=3:break
        if len(findings)>=3:break
    report={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','id_limit_per_domain':2000,'resources_inspected':inspected,'samples':findings}
    text=json.dumps(report,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

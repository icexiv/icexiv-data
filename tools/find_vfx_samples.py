"""Bounded readonly AVFX discovery from base vfx/chara indexes.

Args: game/sqpack new-output-directory. Inspect at most 100000 entries per pack,
decode at most 64 MiB per candidate, stop after six AVFXs including a model.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib


def standard(f,entry,max_bytes=4*1024*1024):
    f.seek(entry);header=f.read(24)
    if len(header)!=24:return None
    size,kind,target,_,_,count=struct.unpack('<6I',header)
    if kind!=2 or not 8<=target<=max_bytes or not 24<=size<=2*1024*1024 or count>4096:return None
    assert 24+count*8<=size
    table=f.read(count*8);parts=[];total=0
    for offset,span,expected in struct.iter_unpack('<IHH',table):
        assert span>=16
        f.seek(entry+size+offset);hs,_,stored,expanded=struct.unpack('<4I',f.read(16));assert expected==expanded and expanded<=65535
        n=expanded if stored==32000 else stored;assert 16<=hs<=span and n<=span-hs
        f.seek(entry+size+offset+hs);data=f.read(n)
        if stored!=32000:
            d=zlib.decompressobj(-15);data=d.decompress(data,65536);assert d.eof and not d.unused_data and not d.unconsumed_tail
        assert len(data)==expanded
        total+=len(data);assert total<=target
        parts.append(data)
    raw=b''.join(parts);assert len(raw)==target
    return raw


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    findings=[];examined=0;has_model=False
    with contextlib.ExitStack() as stack:
        for stem in ['080000','040000']:
            index=root/'ffxiv'/f'{stem}.win32.index'
            with index.open('rb') as f:
                f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);offset,size=struct.unpack('<II',f.read(8));assert size<=64<<20
                f.seek(offset);table=f.read(min(size,100000*16))
            files={}
            for ordinal,(key,packed,_) in enumerate(struct.iter_unpack('<QII',table)):
                if packed&1:continue
                dat=f'ffxiv/{stem}.win32.dat{(packed>>1)&7}'
                if dat not in files:files[dat]=stack.enter_context((root/dat).open('rb'))
                f=files[dat];entry=(packed>>4)*128;f.seek(entry);header=f.read(24)
                if len(header)!=24:continue
                size,kind,target=struct.unpack_from('<3I',header)
                if kind!=2 or not 8<=target<=4<<20:continue
                # Inspect only first decoded block before whole-resource extraction.
                examined+=1
                f.seek(entry+24);offset,span,expected=struct.unpack('<IHH',f.read(8));f.seek(entry+size+offset)
                hs,_,stored,expanded=struct.unpack('<4I',f.read(16));n=expanded if stored==32000 else stored
                if not 16<=hs<=span or n>span-hs or expanded>65535:continue
                f.seek(entry+size+offset+hs);prefix=f.read(n)
                if stored!=32000:
                    d=zlib.decompressobj(-15);prefix=d.decompress(prefix,65536);assert d.eof and not d.unconsumed_tail and not d.unused_data
                if prefix[:4]!=b'XFVA':continue
                raw=standard(f,entry);assert raw is not None
                model=b'ldoM' in raw
                if len(findings)>=5 and not model:continue
                filename=f'{len(findings)}.avfx';(out/filename).write_bytes(raw)
                findings.append({'file':filename,'dat':dat,'offset':entry,'index_ordinal':ordinal,'index_hash':f'{key:016x}','bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'contains_model_tag':model})
                has_model|=model
                if len(findings)>=6 and has_model:break
            if len(findings)>=6 and has_model:break
    report={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','entries_examined':examined,'index_limit_per_pack':100000,'samples':findings}
    text=json.dumps(report,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

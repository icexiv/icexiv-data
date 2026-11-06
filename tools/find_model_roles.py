"""Bounded readonly type-3 runtime inventory for uncommon MDL roles/ElementIds.

Args: game/sqpack new-report.json. First 100000 records and at most 1500 models
per base bgcommon/bg/chara index; <=16 MiB per model, <=2 MiB runtime header.
Retains first witnesses for shadow/terrain/fog/light/glass or nonzero rotation.
"""
import contextlib
import json
from pathlib import Path
import struct
import sys
import zlib


def runtime(f,entry,prefix):
    hs,_,target=struct.unpack_from('<III',prefix);assert 208<=hs<=1<<20 and target<=16<<20
    f.seek(entry);header=f.read(hs);start,=struct.unpack_from('<H',header,158);count,=struct.unpack_from('<H',header,180);offset,=struct.unpack_from('<I',header,116)
    assert 208+(start+count)*2<=hs;position=entry+hs+offset;parts=[];total=0
    for j in range(start,start+count):
        f.seek(position);hsize,_,stored,expanded=struct.unpack('<4I',f.read(16));span,=struct.unpack_from('<H',header,208+2*j);n=expanded if stored==32000 else stored
        assert expanded<=65535 and 16<=hsize<=span and n<=span-hsize
        f.seek(position+hsize);data=f.read(n)
        if stored!=32000:
            d=zlib.decompressobj(-15);data=d.decompress(data,65536);assert d.eof and not d.unused_data and not d.unconsumed_tail
        assert len(data)==expanded;total+=len(data);assert total<=2<<20;parts.append(data);position+=span
    return b''.join(parts)


def main():
    root=Path(sys.argv[1]);out=Path(sys.argv[2]);assert not out.resolve().is_relative_to(root.parent.resolve())
    targets={'Shadow','TerrainShadow','VerticalFog','LightShaft','Glass','ElementRotation'};seen=set();findings=[];packs=[];failures=[]
    with contextlib.ExitStack()as stack:
        for stem in['010000','020000','040000']:
            with(root/f'ffxiv/{stem}.win32.index').open('rb')as f:
                f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);table=f.read(min(size,100000*16))
            handles={};entries=models=0
            for ordinal,(key,p,_)in enumerate(struct.iter_unpack('<QII',table)):
                entries+=1
                if p&1:continue
                dat=f'ffxiv/{stem}.win32.dat{(p>>1)&7}';entry=(p>>4)*128
                if dat not in handles:handles[dat]=stack.enter_context((root/dat).open('rb'))
                f=handles[dat];f.seek(entry);prefix=f.read(208)
                if len(prefix)!=208 or struct.unpack_from('<I',prefix,4)[0]!=3:continue
                models+=1
                if struct.unpack_from('<I',prefix,8)[0]>16<<20:continue
                try:
                    raw=runtime(f,entry,prefix);ss,=struct.unpack_from('<I',raw,4);h=raw[8+ss:64+ss];assert len(h)==56
                    elements,=struct.unpack_from('<H',h,24);lods=64+ss+32*elements;assert lods+180<=len(raw)
                    roles=['Normal','Water','Shadow','TerrainShadow','VerticalFog','LightShaft','Glass','MaterialChange','CrestChange'];counts=[0]*9
                    for lod in range(prefix[204]):
                        assert lod<3
                        counts[0]+=struct.unpack_from('<H',raw,lods+lod*60+2)[0]
                        for r in range(1,5):counts[r]+=struct.unpack_from('<H',raw,lods+lod*60+14+(r-1)*4)[0]
                        if h[27]&16:
                            for r in range(5,9):counts[r]+=struct.unpack_from('<H',raw,lods+180+lod*40+(r-5)*4+2)[0]
                    witnessed={role for role,count in zip(roles,counts)if count}
                    nonzero_rotation=any(any(abs(v)>0.001 for v in struct.unpack_from('<3f',raw,64+ss+i*32+20))for i in range(elements))
                    if nonzero_rotation:witnessed.add('ElementRotation')
                    fresh=targets&witnessed-seen
                    if fresh:
                        seen.update(fresh);findings.append({'dat':dat,'offset':entry,'index_ordinal':ordinal,'index_hash':f'{key:016x}','roles':dict(zip(roles,counts)),'element_count':elements,'nonzero_element_rotation':nonzero_rotation,'new_witnesses':sorted(fresh)})
                except (AssertionError,struct.error,IndexError)as error:failures.append({'dat':dat,'offset':entry,'error':str(error)})
                if models>=1500 or seen>=targets:break
            packs.append({'stem':stem,'entries_examined':entries,'model_entries_examined':models})
            if seen>=targets:break
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','index_limit_per_pack':100000,'model_limit_per_pack':1500,'rotation_magnitude_threshold':0.001,'packs':packs,'unobserved_targets':sorted(targets-seen),'observation_failures':failures,'samples':findings}
    text=json.dumps(result,indent=2)+'\n'
    with out.open('x')as f:f.write(text)
    print(text,end='')


if __name__=='__main__':main()

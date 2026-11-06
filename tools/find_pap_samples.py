"""Bounded readonly PAP inventory; no game execution or parser dependencies.

Args: game/sqpack new-output-dir. Inspect first 100000 base chara index records,
at most 200 PAPs, retain at most eight distinct animation encoding profiles.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from find_vfx_samples import standard
from observe_tagfile import read


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    with (root/'ffxiv/040000.win32.index').open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);table=f.read(min(size,100000*16))
    samples=[];profiles=set();examined=pap_count=0;failures=[]
    with contextlib.ExitStack()as stack:
        files={}
        for ordinal,(key,packed,_)in enumerate(struct.iter_unpack('<QII',table)):
            examined+=1
            if packed&1:continue
            dat=f'ffxiv/040000.win32.dat{(packed>>1)&7}';entry=(packed>>4)*128
            if dat not in files:files[dat]=stack.enter_context((root/dat).open('rb'))
            f=files[dat];f.seek(entry);header=f.read(24)
            if len(header)!=24:continue
            size,kind,target,_,_,count=struct.unpack('<6I',header)
            if kind!=2 or not 26<=target<=4<<20 or not 1<=count<=4096 or not 24+8*count<=size<=2<<20:continue
            offset,span,expected=struct.unpack('<IHH',f.read(8));f.seek(entry+size+offset);hs,_,stored,expanded=struct.unpack('<4I',f.read(16));n=expanded if stored==32000 else stored
            if not 16<=hs<=span or n>span-hs or expanded!=expected:continue
            f.seek(entry+size+offset+hs);prefix=f.read(n)
            if stored!=32000:
                d=zlib.decompressobj(-15);prefix=d.decompress(prefix,65536);assert d.eof and not d.unused_data and not d.unconsumed_tail
            if prefix[:4]!=b'pap ':continue
            pap_count+=1;raw=standard(f,entry);assert raw is not None
            candidate=out/'candidate.pap';candidate.write_bytes(raw)
            try:
                graph=read(candidate);animations=[]
                for obj in graph.objects:
                    if not obj or graph.classes[obj['class']]['name']!=b'hkaSplineCompressedAnimation':continue
                    fields=obj['fields'];tracks=fields[b'numberOfTransformTracks'];data=bytes(fields[b'data']);offsets=fields[b'blockOffsets']
                    masks=[tuple(data[o+i*4:o+i*4+4])for o in offsets for i in range(tracks)]
                    animations.append({'tracks':tracks,'float_tracks':fields.get(b'numberOfFloatTracks',0),'frames':fields[b'numFrames'],'blocks':len(offsets),'rotation_encodings':sorted({m[0]>>2&15 for m in masks}),'vector_encodings':sorted({q for m in masks for q in [m[0]&3,m[0]>>6]})})
                profile=json.dumps(animations,sort_keys=True)
                if animations and profile not in profiles:
                    profiles.add(profile);filename=f'{len(samples)}.pap';(out/filename).write_bytes(raw)
                    samples.append({'file':filename,'dat':dat,'offset':entry,'index_hash':f'{key:016x}','index_ordinal':ordinal,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'animations':animations})
            except (AssertionError,ValueError,KeyError,IndexError)as e:failures.append({'ordinal':ordinal,'error':str(e)})
            if pap_count>=200 or len(samples)>=8:break
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','index_limit':100000,'entries_examined':examined,'pap_limit':200,'pap_examined':pap_count,'observation_failures':failures,'samples':samples}
    text=json.dumps(result,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

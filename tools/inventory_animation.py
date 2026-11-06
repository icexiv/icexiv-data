"""Readonly PAP encoding inventory independent of game animation execution.

Args: game/sqpack new-output-dir. At most 100000 base chara index records,
2000 PAPs, 4 MiB per package; retain first witnesses for float tracks, uncommon
quaternion encoding, non-spline animations and zero transform tracks.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
from observe_standard import prefix
from find_vfx_samples import standard
from observe_tagfile import read


def main():
    root,out=map(Path,sys.argv[1:3]);stratified=len(sys.argv)>3 and sys.argv[3]=='--stratified';assert len(sys.argv)==3 or stratified;assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    with(root/'ffxiv/040000.win32.index').open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);table=f.read(min(size,100000*16))
    samples=[];seen=set();failures=[];hist={};empty=0;pap_count=entries=0;families={}
    with contextlib.ExitStack()as stack:
        handles={}
        for ordinal,(key,packed,_)in enumerate(struct.iter_unpack('<QII',table)):
            entries+=1
            if packed&1:continue
            if stratified and families.get(key>>32,0)>=16:continue
            dat=f'ffxiv/040000.win32.dat{(packed>>1)&7}';entry=(packed>>4)*128
            if dat not in handles:handles[dat]=stack.enter_context((root/dat).open('rb'))
            observed=prefix(handles[dat],entry)
            if observed is None or observed[0][:4]!=b'pap ':continue
            pap_count+=1;raw=standard(handles[dat],entry);assert raw is not None
            families[key>>32]=families.get(key>>32,0)+1
            count,=struct.unpack_from('<H',raw,8);start,end=struct.unpack_from('<II',raw,18)
            if count==0 and start<=end<=len(raw)and not any(raw[start:end]):empty+=1
            else:
                candidate=out/'candidate.pap';candidate.write_bytes(raw)
                try:
                    graph=read(candidate,max_array_length=4<<20,max_values=8<<20);container=next(o for o in graph.objects if o and graph.classes[o['class']]['name']==b'hkaAnimationContainer');witnesses=set();animations=[]
                    for ref in container['fields'].get(b'animations',[]):
                        obj=graph.objects[ref['ref']];cls=graph.classes[obj['class']]['name'];fs=obj['fields'];name=cls.decode('ascii');hist[name]=hist.get(name,0)+1
                        if cls!=b'hkaSplineCompressedAnimation':witnesses.add('non-spline');animations.append({'class':name});continue
                        n=fs.get(b'numberOfTransformTracks',0);floats=fs.get(b'numberOfFloatTracks',0);data=bytes(fs[b'data']);masks=[data[offset+4*i]for offset in fs[b'blockOffsets']for i in range(n)];encodings=sorted({m>>2&15 for m in masks})
                        if floats:witnesses.add('float-tracks')
                        if n==0:witnesses.add('zero-transform-tracks')
                        if len(data)>1000000:witnesses.add('large-byte-array')
                        witnesses.update(f'quaternion-{q}'for q in encodings if q not in[0,1,2])
                        animations.append({'class':name,'transform_tracks':n,'float_tracks':floats,'frames':fs.get(b'numFrames'),'blocks':len(fs[b'blockOffsets']),'rotation_encodings':encodings,'data_bytes':len(data)})
                    fresh=witnesses-seen
                    if fresh:
                        seen.update(fresh);file=f'{len(samples)}.pap';(out/file).write_bytes(raw);samples.append({'dat':dat,'offset':entry,'index_ordinal':ordinal,'index_hash':f'{key:016x}','file':file,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'witnesses':sorted(fresh),'animations':animations})
                except (AssertionError,ValueError,KeyError,IndexError,StopIteration)as error:failures.append({'ordinal':ordinal,'error':str(error)})
            if pap_count>=2000:break
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','index_record_limit':100000,'pap_limit':2000,'family_limit':16 if stratified else None,'families_observed':len(families),'entries_examined':entries,'pap_examined':pap_count,'empty_packages':empty,'animation_classes':hist,'observation_failures':failures,'samples':samples}
    text=json.dumps(result,indent=2)+'\n'
    manifest_path=Path(__file__).resolve().parent.parent/'tests/fixtures/pap-large-observations.json'
    if manifest_path.exists():
        manifest=json.loads(manifest_path.read_text());key='stratified_inventory_sha256'if stratified else'inventory_sha256';assert result['version']==manifest['version']and hashlib.sha256(text.encode()).hexdigest()==manifest[key]
    (out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

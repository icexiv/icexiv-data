"""Bounded readonly IMC->weapon AVFX/MDL candidate observation.

Args: game/sqpack new-output-dir. Probe weapon roots w0101..w2901 and bodies
1..2000 in the base chara index; store at most three matching resource groups.
Naming candidates are verified by index lookup, not assumed to be present.
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
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    index=root/'ffxiv/040000.win32.index'
    with index.open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);offset,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(offset);entries={k:p for k,p,_ in struct.iter_unpack('<QII',f.read(size))if not p&1}
    def locate(path):
        directory,filename=path.rsplit('/',1);jam=lambda s:~zlib.crc32(s.encode())&0xffffffff
        p=entries.get(jam(directory)<<32|jam(filename))
        return None if p is None else(f'ffxiv/040000.win32.dat{(p>>1)&7}',(p>>4)*128)
    matches=[];present=0;nonzero=0;tested=0
    with contextlib.ExitStack()as stack:
        handles={}
        def read(location):
            dat,offset=location
            if dat not in handles:handles[dat]=stack.enter_context((root/dat).open('rb'))
            return standard(handles[dat],offset)
        for w in range(101,2902,100):
            for body in range(1,2001):
                stem=f'chara/weapon/w{w:04}/obj/body/b{body:04}';imc=f'{stem}/b{body:04}.imc';tested+=1;where=locate(imc)
                if where is None:continue
                raw=read(where);assert raw is not None and len(raw)>=4;variants,mask=struct.unpack_from('<HH',raw);assert mask in[1,31]and len(raw)==4+(variants+1)*mask.bit_count()*6;present+=1
                ids=sorted({v for _,_,_,v,_ in struct.iter_unpack('<BBHBB',raw[4:])if v})
                if not ids:continue
                nonzero+=1;mdl=f'{stem}/model/w{w:04}b{body:04}.mdl'
                if locate(mdl)is None:continue
                for vid in ids:
                    path=f'{stem}/vfx/eff/vw{vid:04}.avfx';location=locate(path)
                    if location is None:continue
                    avfx=read(location);assert avfx is not None and avfx[:4]==b'XFVA'
                    i=len(matches);(out/f'{i}.imc').write_bytes(raw);(out/f'{i}.avfx').write_bytes(avfx)
                    matches.append({'weapon':w,'body':body,'vfx_id':vid,'imc_path':imc,'imc_sha256':hashlib.sha256(raw).hexdigest(),'avfx_path':path,'avfx_sha256':hashlib.sha256(avfx).hexdigest(),'mdl_path':mdl,'sklb_path':f'chara/weapon/w{w:04}/skeleton/base/b0001/skl_w{w:04}b0001.sklb'})
                    break
                if len(matches)>=3:break
            if len(matches)>=3:break
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','candidate_limit':29*2000,'candidates_tested':tested,'present_imc':present,'nonzero_vfx_imc':nonzero,'samples':matches}
    text=json.dumps(result,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

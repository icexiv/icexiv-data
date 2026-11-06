"""Bounded readonly variant discovery from actual AVFX Data and Tex pointers.

Args: game/sqpack new-output-dir [pixel-formats]. At most 100000 base-vfx index entries, 2000
AVFXs and 20000 unique referenced textures; selected assets only are saved.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from find_vfx_samples import standard
from verify_avfx import fields,label


def prefix(f,entry):
    f.seek(entry);raw=f.read(32)
    if len(raw)!=32:return None
    hs,kind,target=struct.unpack_from('<3I',raw)
    if not 8<=target<=64<<20 or not 24<=hs<=2<<20:return None
    if kind==4:f.seek(entry+hs);return f.read(80)
    if kind!=2:return None
    offset,span,expected=struct.unpack_from('<IHH',raw,24)
    f.seek(entry+hs+offset);bs,_,stored,expanded=struct.unpack('<4I',f.read(16));n=expanded if stored==32000 else stored
    assert 16<=bs<=span and n<=span-bs and expanded==expected and expanded<=65535
    f.seek(entry+hs+offset+bs);p=f.read(n)
    if stored!=32000:
        d=zlib.decompressobj(-15);p=d.decompress(p,65536);assert d.eof and not d.unused_data and not d.unconsumed_tail
    assert len(p)==expanded
    return p


def main():
    root,out=map(Path,sys.argv[1:3]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    pixel_formats = len(sys.argv) == 4 and sys.argv[3] == 'pixel-formats'
    assert len(sys.argv) == 3 or pixel_formats
    known={(b'Emit',3),(b'Emit',4),(b'Emit',5),(b'Ptcl',1),(b'Ptcl',2),(b'Ptcl',5),(b'Ptcl',12),(b'Ptcl',13),(b'Efct',0),(b'Bind',0)}
    found=[];texture_matches=[];texture_labels=set();seen_paths=set();avfx_count=0;entries_examined=0;all_types=set();indexes={};missing_base=0;texture_headers=0;outside_policy=0
    with contextlib.ExitStack()as stack:
        handles={}
        def handle(path):
            if path not in handles:handles[path]=stack.enter_context(path.open('rb'))
            return handles[path]
        def index(stem):
            if stem not in indexes:
                path=root/'ffxiv'/f'{stem}.win32.index'
                with path.open('rb')as f:
                    f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);offset,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(offset);indexes[stem]=list(struct.iter_unpack('<QII',f.read(size)))
            return indexes[stem]
        lookup={key:p for key,p,_ in index('080000')if not p&1}
        for ordinal,(key,packed,_)in enumerate(index('080000')[:100000]):
            entries_examined+=1
            if packed&1:continue
            dat=f'ffxiv/080000.win32.dat{(packed>>1)&7}';entry=(packed>>4)*128;f=handle(root/dat);p=prefix(f,entry)
            if p is None or p[:4]!=b'XFVA':continue
            raw=standard(f,entry)
            if raw is None:continue # beyond bounded whole-AVFX size policy
            avfx_count+=1;roots=fields(raw[8:],8);types=set();textures=[]
            for t,b,o,_ in roots:
                t=label(t)
                if t==b'Tex':textures.append(b.split(b'\0',1)[0].decode('ascii'));continue
                if t not in[b'Emit',b'Ptcl',b'Efct',b'Bind']:continue
                fs=fields(b,o+8)
                if not any(label(t)==b'Data'for t,_,_,_ in fs):continue
                tag={b'Emit':b'EVT',b'Ptcl':b'PrVT',b'Efct':b'EfVT',b'Bind':b'BnVr'}[t];values=[v for n,v,_,_ in fs if label(n)==tag];assert len(values)==1
                value,=struct.unpack('<i',values[0]);types.add((t,value))
            fresh=types-known;all_types|=types;new_textures=[]
            for path in textures:
                if path in seen_paths or not path.startswith('vfx/'):continue
                if len(seen_paths)>=20000:break
                seen_paths.add(path)
                directory,filename=path.rsplit('/',1);jam=lambda s:~zlib.crc32(s.encode('ascii'))&0xffffffff
                loc=lookup.get(jam(directory)<<32|jam(filename))
                if loc is None:missing_base+=1;continue # no matching base index record, not global absence
                td=f'ffxiv/080000.win32.dat{(loc>>1)&7}';te=(loc>>4)*128;tf=handle(root/td);h=prefix(tf,te)
                if h is None or len(h)<80:outside_policy+=1;continue
                texture_headers+=1
                attrs,fmt,w,height,depth=struct.unpack_from('<II3H',h);layout=attrs&0x13c00000;labels=[]
                if layout==0x02000000:labels.append('cube')
                if layout==0x01000000:labels.append('volume')
                if layout==0x10000000:labels.append('array')
                if fmt in [0x2140,0x2150,0x2250,0x2260,0x2350,0x2460,0x2470]:labels.append('float')
                if pixel_formats and fmt in [0x1131,0x3430,0x6432]:labels.append(f'pixel-{fmt:04x}')
                if any(label not in texture_labels for label in labels):
                    new_textures.append({'path':path,'dat':td,'offset':te,'attributes':hex(attrs),'format':hex(fmt),'width':w,'height':height,'depth':depth,'labels':labels});texture_labels.update(labels)
            if (fresh or new_textures)and len(found)<32:
                filename=f'{len(found)}.avfx';(out/filename).write_bytes(raw)
                found.append({'file':filename,'dat':dat,'offset':entry,'index_ordinal':ordinal,'index_hash':f'{key:016x}','sha256':hashlib.sha256(raw).hexdigest(),'data_types':[[t.decode(),v]for t,v in sorted(types)],'new_data_types':[[t.decode(),v]for t,v in sorted(fresh)]})
                known|=fresh
                for record in new_textures:record['owner_file']=filename;texture_matches.append(record)
            if avfx_count>=2000:break
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','entries_examined':entries_examined,'avfx_examined':avfx_count,'referenced_vfx_textures_examined':len(seen_paths),'texture_headers_observed':texture_headers,'paths_without_base_index_record':missing_base,'texture_headers_outside_policy':outside_policy,'all_observed_data_types':[[t.decode(),v]for t,v in sorted(all_types)],'samples':found,'textures':texture_matches}
    text=json.dumps(result,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

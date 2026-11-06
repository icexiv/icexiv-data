"""Bounded offline PE string / RIP-relative LEA candidates for EQP/PBD/CMP.

Args: authorized-ffxiv_dx11.exe new-report.json. This does not launch, attach,
emulate or modify the target. LEA byte patterns are candidates until objdump
confirms instruction boundaries; .pdata supplies recorded function ranges.
"""
import bisect
import hashlib
import json
from pathlib import Path
import re
import struct
import sys


def main():
    path,out=map(Path,sys.argv[1:]);assert path.stat().st_size<=128<<20;assert not out.resolve().is_relative_to(path.parent.resolve())
    b=path.read_bytes();assert b[:2]==b'MZ';pe,=struct.unpack_from('<I',b,60);assert b[pe:pe+4]==b'PE\0\0';machine,n,stamp,_,_,size,_=struct.unpack_from('<HHIIIHH',b,pe+4);optional=pe+24;assert machine==0x8664 and struct.unpack_from('<H',b,optional)[0]==0x20b;image,=struct.unpack_from('<Q',b,optional+24)
    sections=[]
    for i in range(n):
        p=optional+size+40*i;name=b[p:p+8].rstrip(b'\0').decode('ascii');vs,va,rs,off=struct.unpack_from('<4I',b,p+8);assert off+rs<=len(b);sections.append({'name':name,'rva':va,'virtual_size':vs,'file_offset':off,'file_size':rs})
    def virtual(offset):
        for s in sections:
            if s['file_offset']<=offset<s['file_offset']+s['file_size']:return image+s['rva']+offset-s['file_offset']
        return None
    pdata=next(s for s in sections if s['name']=='.pdata');assert pdata['virtual_size']%12==0
    functions=list(struct.iter_unpack('<III',b[pdata['file_offset']:pdata['file_offset']+pdata['virtual_size']]));starts=[f[0]for f in functions];assert starts==sorted(starts)
    strings=[];by_address={}
    for needle in[b'equipmentparameter.eqp',b'human.pbd',b'human.cmp',b'EquipmentParameterResourceHandle',b'PreBoneDeformerResourceHandle',b'atr_inr',b'atr_tlh',b'atr_tls',b'atr_nek',b'atr_hij',b'atr_ude',b'atr_sne',b'atr_hiz']:
        for match in re.finditer(re.escape(needle),b,re.IGNORECASE):
            pos=match.start();begin=b.rfind(b'\0',max(0,pos-1024),pos)+1;end=b.find(b'\0',pos,min(len(b),pos+1024));assert end>=pos
            text=b[begin:end];address=virtual(begin)
            record={'needle':needle.decode(),'file_offset':begin,'va':None if address is None else hex(address),'bytes_hex':text.hex(),'lea_candidates':[]};strings.append(record)
            if address is not None:by_address.setdefault(address,[]).append(record)
    text=next(s for s in sections if s['name']=='.text');code=b[text['file_offset']:text['file_offset']+text['file_size']];base=image+text['rva']
    for match in re.finditer(rb'[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]....',code,re.DOTALL):
        address=base+match.start();target=address+7+struct.unpack_from('<i',match.group(),3)[0]
        if target not in by_address:continue
        rva=address-image;index=bisect.bisect_right(starts,rva)-1;function=None
        if index>=0 and functions[index][0]<=rva<functions[index][1]:function=[hex(image+functions[index][0]),hex(image+functions[index][1])]
        for record in by_address[target]:record['lea_candidates'].append({'va':hex(address),'bytes':match.group().hex(),'pdata_function':function})
    result={'path':str(path.resolve()),'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b),'coff_timestamp':stamp,'image_base':hex(image),'sections':sections,'strings':strings,'limitations':'read-only string/LEA candidates; not instruction-boundary proof, runtime observation, or REA Evidence'}
    data=json.dumps(result,indent=2)+'\n'
    with out.open('x')as f:f.write(data)
    print(data,end='')


if __name__=='__main__':main()

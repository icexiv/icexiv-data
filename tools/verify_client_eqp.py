"""Pinned, read-only client EQP static observations. No target execution.

Args: authorized-ffxiv_dx11.exe new-output-directory. Uses installed objdump;
this is ordinary offline evidence, not REA Evidence or runtime observation.
Only hashes/addresses/field facts are retained in the repository; assembly stays
in the selected external output directory. No client routine is ported.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    path,out=map(Path,sys.argv[1:]);assert path.stat().st_size<=128<<20 and not out.resolve().is_relative_to(path.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/client-eqp-observations.json').read_text());b=path.read_bytes();assert hashlib.sha256(b).hexdigest()==manifest['binary_sha256'];assert b[:2]==b'MZ';pe,=struct.unpack_from('<I',b,60);assert b[pe:pe+4]==b'PE\0\0';machine,n,_,_,_,size,_=struct.unpack_from('<HHIIIHH',b,pe+4);optional=pe+24;assert machine==0x8664 and struct.unpack_from('<H',b,optional)[0]==0x20b;image,=struct.unpack_from('<Q',b,optional+24);assert image==int(manifest['image_base'],0)
    sections=[]
    for i in range(n):
        p=optional+size+40*i;_,rva,length,offset=struct.unpack_from('<4I',b,p+8);assert offset+length<=len(b);sections.append((rva,length,offset))
    def raw(rva,count):
        for base,length,off in sections:
            if base<=rva and rva+count<=base+length:return b[off+rva-base:off+rva-base+count]
        raise ValueError('unbacked RVA')
    spans=[]
    for s in manifest['ranges']:
        assert s['bytes']<=65536 and hashlib.sha256(raw(s['rva'],s['bytes'])).hexdigest()==s['sha256']
        text=subprocess.check_output(['objdump','-d','-Mintel',f'--start-address={image+s["rva"]:#x}',f'--stop-address={image+s["rva"]+s["bytes"]:#x}',str(path)],text=True,timeout=30);(out/f'{s["label"]}.asm').write_text(text);spans.append(s)
    fields=[]
    for s in manifest['source_fields']:
        rva=int(s['instruction_rva'],0);ins=raw(rva,4);assert ins[0]==0xf6 and ins[1]>>6==1 and(ins[1]>>3)&7==0 and ins[1]&7!=4 and ins[3].bit_count()==1
        bit=ins[2]*8+ins[3].bit_length()-1;assert bit==s['global_bit'];fields.append({**s,'byte_offset':ins[2],'byte_mask':ins[3]})
    names=[]
    for s in manifest['name_references']:
        rva=int(s['instruction_rva'],0);ins=raw(rva,7);assert ins[:2]in[b'\x48\x8d',b'\x4c\x8d']and ins[2]&0xc7==5;target=rva+7+struct.unpack_from('<i',ins,3)[0];expected=s['name'].encode()+b'\0';assert raw(target,len(expected))==expected;names.append({**s,'target_rva':hex(target)})
    # Coupled selectors and masked fields, checked at disassembled boundaries.
    for rva in[0x447cc5,0x447dca]:assert raw(rva,5)==bytes.fromhex('d1e983e103')
    assert raw(0x447cb9,2)==bytes.fromhex('2418')and raw(0x447dbb,2)==bytes.fromhex('240c')
    # Head byte is global byte five; its bits 1..3 feed a coupled switch.
    assert raw(0x448a2a,4)==bytes.fromhex('0fb64505')
    assert raw(0x448a65,10)==bytes.fromhex('d1e883e007ffc883f805')
    hair_targets=list(struct.unpack('<6I',raw(0x448e54,24)))
    assert hair_targets==[0x448a8b,0x448aee,0x448b4d,0x448aee,0x448b5a,0x448bc9]
    for anchor in manifest['anchors']:
        value=bytes.fromhex(anchor['hex']);assert raw(int(anchor['rva'],0),len(value))==value
    ear_targets=list(struct.unpack('<7I',raw(0x449fe0,28)))
    assert ear_targets==[0x449fb7,0x449fb7,0x449fbf,0x449fdc,0x449fc7,0x449fcf,0x449fd7]
    # Only fixed return-immediate blocks and the literal jump table are checked;
    # no character objects or visibility routines are executed or emulated.
    for at,value in[(0x449fb7,1),(0x449fbf,2),(0x449fc7,3),(0x449fcf,5),(0x449fd7,4)]:
        assert raw(at,5)==b'\xba'+struct.pack('<I',value)
    # Attribute table count, zero-based name index, map value and mask shift.
    assert raw(0x322ba5,5)==bytes.fromhex('0fb7443906')
    assert raw(0x3231b4,2)==bytes.fromhex('33db')and raw(0x3233b8,5)==bytes.fromhex('6641897228')
    assert raw(0x2744e9,4)==bytes.fromhex('0fb74028')and raw(0x447c61,2)==bytes.fromhex('d3e0')
    # Direct bridge from resolved EQP bytes on the stack to the attribute consumer.
    for rva,target in[(0x44746a,0x42bae0),(0x447597,0x447b70),(0x44790a,0x4489a0)]:
        ins=raw(rva,5);assert ins[0]==0xe8 and rva+5+struct.unpack_from('<i',ins,1)[0]==target
    result={'binary_sha256':manifest['binary_sha256'],'image_base':hex(image),'ranges':spans,'source_fields':fields,'name_references':names,'hair_code_jump_targets':[hex(v) for v in hair_targets],'ear_class_jump_targets':[hex(v)for v in ear_targets],'anchors':manifest['anchors'],'objdump':subprocess.check_output(['objdump','--version'],text=True).splitlines()[0],'limitations':'pinned static instructions and strings; no client execution/emulation, no full visibility policy, not REA Evidence'}
    text=json.dumps(result,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

"""Readonly extra-stage SHPK evidence: full metadata, DXBC chunks, raw passes.

Args: game/sqpack new-output-dir. No shader execution or third-party parser.
The raw pass tail words are observations, not asserted runtime stage selectors.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from find_vfx_samples import standard
from verify_materials import observe_shpk


def observe_code_and_passes(raw):
    version,=struct.unpack_from('<I',raw,4);code,names=struct.unpack_from('<II',raw,16);counts=struct.unpack_from('<2I',raw,24)+struct.unpack_from('<3I',raw,72);p=88 if version==0xe01 else 84;shaders=[]
    for stage,count in zip(['Vertex','Pixel','Hull','Domain','Geometry'],counts):
        for index in range(count):
            off,size,*resources,_=struct.unpack_from('<II4HI',raw,p);p+=20+16*sum(resources)
            b=raw[code+off:code+off+size];b=b[8:]if stage=='Vertex'else b
            assert b[:4]==b'DXBC'and struct.unpack_from('<I',b,24)[0]==len(b)
            count,=struct.unpack_from('<I',b,28);assert count<=1024 and 32+4*count<=len(b)
            chunks=[];ranges=[]
            for offset in struct.unpack_from(f'<{count}I',b,32):
                assert 32+4*count<=offset<=len(b)-8;size,=struct.unpack_from('<I',b,offset+4);assert offset+8+size<=len(b)
                for start,end in ranges:assert offset+8+size<=start or offset>=end
                ranges.append((offset,offset+8+size));tag=b[offset:offset+4].decode('ascii');chunk={'tag':tag,'bytes':size}
                if tag in['SHDR','SHEX']:
                    token,words=struct.unpack_from('<II',b,offset+8);assert size%4==0 and words==size//4;chunk['program_token_high16']=token>>16;chunk['version_low16']=token&65535
                chunks.append(chunk)
            shaders.append({'stage':stage,'index':index,'sha256':hashlib.sha256(b).hexdigest(),'chunks':chunks})
    params,defaults=struct.unpack_from('<2H',raw,36);p+=8*params+(struct.unpack_from('<I',raw,32)[0]if defaults else 0)
    c,_,s,t,u,_=struct.unpack_from('<6H',raw,40);p+=16*(c+s+t+u);keys=struct.unpack_from('<3I',raw,52);p+=8*sum(keys)+8
    nodes=[]
    for _ in range(struct.unpack_from('<I',raw,64)[0]):
        selector,count=struct.unpack_from('<II',raw,p);p+=32+4*(sum(keys)+2);passes=[]
        for _ in range(count):passes.append(list(struct.unpack_from('<6I',raw,p)));p+=24
        nodes.append({'selector':selector,'passes':passes})
    assert p<=code
    return {'shaders':shaders,'nodes':nodes}


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    fixture=Path(__file__).resolve().parent.parent/'tests/fixtures/shpk-variant-observations.json';manifest=json.loads(fixture.read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version'];reports=[]
    for i,s in enumerate(manifest['samples']):
        with(root/'ffxiv/050000.win32.index').open('rb')as f:
            f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert 16*(s['index_ordinal']+1)<=size;f.seek(off+16*s['index_ordinal']);key,p,_=struct.unpack('<QII',f.read(16));assert not p&1 and f'{key:016x}'==s['index_hash']and(p>>4)*128==s['offset']and(p>>1)&7==int(s['dat'][-1])
        with(root/s['dat']).open('rb')as f:raw=standard(f,s['offset'])
        target=out/f'{i}.shpk';subprocess.check_output(['target/debug/examples/extract_entry',str(root),s['dat'],str(s['offset']),str(target)]);assert raw==target.read_bytes()and hashlib.sha256(raw).hexdigest()==s['sha256']
        rows,report,_=observe_shpk(raw);expected='\n'.join(rows)+'\n';actual=subprocess.check_output(['target/debug/examples/validate_materials',str(target)],text=True);assert expected==actual and hashlib.sha256(actual.encode()).hexdigest()==s['rows_sha256'];(out/f'{i}.rust.tsv').write_text(actual);(out/f'{i}.python.tsv').write_text(expected)
        report.update(observe_code_and_passes(raw));report['sha256']=s['sha256'];reports.append(report)
    text=json.dumps({'version':version,'platform':'Win32','samples':reports},indent=2)+'\n'
    if'results_sha256'in manifest:assert hashlib.sha256(text.encode()).hexdigest()==manifest['results_sha256']
    (out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

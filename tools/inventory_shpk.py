"""Readonly SHPK stage-count inventory in the indexed sm5/shpk directory.

Args: game/sqpack new-report.json. At most 100000 base shader index records,
128 MiB declared bytes per entry; only first decompressed block is observed.
This inventory is not a shader-bytecode validation or execution claim.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from observe_standard import prefix


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve())
    directory=~zlib.crc32(b'shader/sm5/shpk')&0xffffffff
    with(root/'ffxiv/050000.win32.index').open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);table=f.read(min(size,100000*16))
    samples=[];failures=[]
    with contextlib.ExitStack()as stack:
        handles={}
        for ordinal,(key,packed,_)in enumerate(struct.iter_unpack('<QII',table)):
            if key>>32!=directory:continue
            dat=f'ffxiv/050000.win32.dat{(packed>>1)&7}';entry=(packed>>4)*128
            if packed&1:failures.append({'ordinal':ordinal,'reason':'synonym'});continue
            if dat not in handles:handles[dat]=stack.enter_context((root/dat).open('rb'))
            observed=prefix(handles[dat],entry,128<<20)
            if observed is None:failures.append({'ordinal':ordinal,'reason':'entry outside policy'});continue
            b,target=observed
            version,=struct.unpack_from('<I',b,4);assert version in[0xd01,0xe01];header_size=84 if version==0xd01 else 88
            assert b[:4]==b'ShPk'and len(b)>=header_size and struct.unpack_from('<I',b,12)[0]==target
            samples.append({'dat':dat,'offset':entry,'index_ordinal':ordinal,'index_hash':f'{key:016x}','bytes':target,'version':version,'header_bytes':header_size,'platform':b[8:12].decode('ascii'),'counts':dict(zip(['Vertex','Pixel','Hull','Domain','Geometry'],struct.unpack_from('<2I',b,24)+struct.unpack_from('<3I',b,72))),'header_sha256':hashlib.sha256(b[:header_size]).hexdigest()})
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','index_record_limit':100000,'records_examined':len(table)//16,'directory':'shader/sm5/shpk','observation_failures':failures,'samples':samples}
    text=json.dumps(result,indent=2)+'\n'
    manifest_path=Path(__file__).resolve().parent.parent/'tests/fixtures/shpk-variant-observations.json'
    if manifest_path.exists():
        manifest=json.loads(manifest_path.read_text());assert result['version']==manifest['version']and hashlib.sha256(text.encode()).hexdigest()==manifest['inventory_sha256']
    with out.open('x')as f:f.write(text)
    print(text,end='')


if __name__=='__main__':main()

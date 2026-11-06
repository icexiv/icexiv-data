"""Readonly accessory EQDP slot/path/MDL/MTRL evidence, not runtime routing.

Args: game/sqpack new-output-directory. Table fixture selects six races; first
indexed high-bit MDL per slot for c0101/c0201 and c0101's low-only materials
are fully observed. Raw extracted assets and semantic rows stay outside repo.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
from find_vfx_samples import standard
from verify_model import observe as extract_model
from verify_mdl import observe as observe_model
from verify_materials import observe_mtrl


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    fixture=Path(__file__).resolve().parent.parent/'tests/fixtures/eqdp-variant-observations.json';manifest=json.loads(fixture.read_text())
    version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    with(root/'ffxiv/040000.win32.index').open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);index={key:p for key,p,_ in struct.iter_unpack('<QII',f.read(size))if not p&1}
    crc=lambda s:~zlib.crc32(s.encode('ascii'))&0xffffffff
    def location(path):
        folder,name=path.rsplit('/',1);p=index.get(crc(folder)<<32|crc(name))
        return None if p is None else(root/f'ffxiv/040000.win32.dat{(p>>1)&7}',(p>>4)*128)
    def extract(path,target):
        dat,entry=location(path)
        with dat.open('rb')as f:raw=standard(f,entry)
        assert raw is not None
        subprocess.check_output(['target/debug/examples/local_sqpack',str(root),path,str(target)])
        assert target.read_bytes()==raw;return raw
    slots=['ear','nek','wrs','rir','ril'];tables=[];models=[];materials=[]
    for sample in manifest['samples']:
        path=sample['path']
        if '/accessorydeformerparameter/'not in path:continue
        race=int(Path(path).stem[1:]);raw=extract(path,out/f'c{race:04}.eqdp');assert hashlib.sha256(raw).hexdigest()==sample['sha256']
        _,size,count=struct.unpack_from('<3H',raw);pointers=struct.unpack_from(f'<{count}H',raw,6);values=[]
        for pointer in pointers:values.extend([0]*size if pointer==65535 else struct.unpack_from(f'<{size}H',raw,6+2*count+2*pointer))
        hist={};chosen=set()
        for identity,value in enumerate(values):
            pairs=[value>>(2*i)&3 for i in range(5)]
            candidates=[f'chara/accessory/a{identity:04}/model/c{race:04}a{identity:04}_{suffix}.mdl'for suffix in slots]
            hits=[location(p)is not None for p in candidates]
            for slot,pair in enumerate(pairs):
                # Include both low/high bits and explicit path absence. A missing
                # resource is an index observation, not an inferred fallback.
                key=f'slot{slot}/pair{pair}/mdl{int(hits[slot])}';hist[key]=hist.get(key,0)+1
                if race in[101,201]and pair&2 and hits[slot]and slot not in chosen:
                    chosen.add(slot);i=len(models);p=candidates[slot];report=extract_model(root,p,out,f'model-{i}');model=out/f'model-{i}.mdl'
                    rows,geometry=observe_model(model);text='\n'.join(rows)+'\n';actual=subprocess.check_output(['target/debug/examples/validate_mdl',str(model)],text=True);assert actual==text;(out/f'model-{i}.tsv').write_text(text)
                    names=lambda tag:[bytes.fromhex(r.split('\t')[2]).decode('ascii')for r in rows if r.startswith(tag+'\t')]
                    models.append({'race':race,'slot':slot,'suffix':slots[slot],'id':identity,'pair':pair,'path':p,'sha256':report['sha256'],'rows_sha256':hashlib.sha256(text.encode()).hexdigest(),'vertices':geometry['vertices'],'bones':names('bone'),'materials':names('material')})
                if race==101 and pair==1:
                    for variant in[1,2,3]:
                        p=f'chara/accessory/a{identity:04}/material/v{variant:04}/mt_c{race:04}a{identity:04}_{slots[slot]}_a.mtrl'
                        if location(p)is None:continue
                        i=len(materials);raw_mtrl=extract(p,out/f'material-{i}.mtrl');rows,_,_=observe_mtrl(raw_mtrl);text='\n'.join(rows)+'\n';actual=subprocess.check_output(['target/debug/examples/validate_materials',str(out/f'material-{i}.mtrl')],text=True);assert actual==text;(out/f'material-{i}.tsv').write_text(text)
                        materials.append({'race':race,'slot':slot,'id':identity,'pair':pair,'path':p,'sha256':hashlib.sha256(raw_mtrl).hexdigest(),'rows_sha256':hashlib.sha256(text.encode()).hexdigest(),'same_race_model_in_index':hits[slot]});break
        if race in[101,201]:assert len(chosen)==5
        tables.append({'race':race,'entries':len(values),'slot_correlations':hist})
    result={'version':version,'platform':'Win32','tables':tables,'models':models,'low_only_materials':materials}
    text=json.dumps(result,indent=2)+'\n'
    if'links_sha256'in manifest:assert hashlib.sha256(text.encode()).hexdigest()==manifest['links_sha256']
    (out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

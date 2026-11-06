"""Readonly furniture SGB structure, dependency edges and target MDL validation.

Args: game/sqpack new-output-dir. Independent struct observer and type-2 raw
deflate reconstruction; target MDLs use verify_mdl's full scalar/index oracle.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
from find_vfx_samples import standard
from observe_sgb import observe
from verify_mdl import observe as observe_mdl


def rows(raw,o):
    result=[f'sgb\t{len(raw)}\t{o["group_offset"]}\t{o["group_id"]}\t{o["group_name"].hex()}\t{len(o["layers"])}'];deps=[]
    for li,layer in enumerate(o['layers']):
        result.append(f'layer\t{layer["offset"]}\t{layer["id"]}\t{layer["name"].hex()}\t{len(layer["instances"])}')
        for ii,instance in enumerate(layer['instances']):
            words=struct.unpack('<9I',struct.pack('<9f',*instance['trs']))
            result.append(f'instance\t{instance["offset"]}\t{instance["type"]}\t{instance["id"]}\t{instance["name"].hex()}\t'+','.join(f'{w:08x}'for w in words))
            for kind,path in instance['paths']:
                result.append(f'path\t{instance["offset"]}\t{kind}\t{path.hex()}')
                if path:deps.append((kind,path,li,ii,instance['id'],instance['offset']))
    for kind,path,li,ii,identity,offset in deps:result.append(f'dependency\t'+{'model':'Model','collision':'Collision','shared_group':'SharedGroup'}[kind]+f'\t{li}\t{ii}\t{identity}\t{offset}\t{path.hex()}')
    return '\n'.join(result)+'\n',deps


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/sgb-observations.json').read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    with (root/'ffxiv/010000.win32.index').open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);offset,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(offset);entries={k:p for k,p,_ in struct.iter_unpack('<QII',f.read(size))if not p&1}
    def locate(path):
        directory,filename=path.rsplit('/',1);jam=lambda s:~zlib.crc32(s.encode())&0xffffffff
        p=entries[jam(directory)<<32|jam(filename)];return root/f'ffxiv/010000.win32.dat{(p>>1)&7}',(p>>4)*128
    def extract(path,target):
        log=subprocess.check_output(['target/debug/examples/local_sqpack',str(root),path,str(target)],text=True);target.with_suffix(target.suffix+'.extraction.txt').write_text(log);return target.read_bytes()
    reports=[];all_deps=[];sgb_names={s['path']for s in manifest['samples']}
    for i,source in enumerate(manifest['samples']):
        path=source['path'];target=out/f'{i}.sgb';raw=extract(path,target);dat,entry=locate(path)
        with dat.open('rb')as f:independent=standard(f,entry)
        assert raw==independent
        o=observe(raw);expected,deps=rows(raw,o);actual=subprocess.check_output(['target/debug/examples/validate_sgb',str(target)],text=True)
        (out/f'{i}.rust.tsv').write_text(actual);(out/f'{i}.python.tsv').write_text(expected);assert actual==expected
        report={'path':path,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),'layers':len(o['layers']),'instances':sum(len(l['instances'])for l in o['layers']),'model_paths':[p.decode()for k,p,*_ in deps if k=='model'],'shared_group_paths':[p.decode()for k,p,*_ in deps if k=='shared_group'],'other_instance_types':sorted({inst['type']for l in o['layers']for inst in l['instances']if inst['type']not in [1,6]})}
        if 'sha256'in source:assert all(source[k]==report[k]for k in ['sha256','rows_sha256'])
        reports.append(report)
        for kind,p,*_ in deps:
            path=p.decode('ascii');assert path.startswith('bgcommon/') and '..'not in path.split('/')
            locate(path) # verify presence independently, including collisions
            if kind=='shared_group':assert path in sgb_names
            all_deps.append((kind,path))
    models=[];pinned_models={s['path']:s for s in manifest['target_models']}
    for j,path in enumerate(sorted({p for k,p in all_deps if k=='model'})):
        target=out/f'model-{j}.mdl';raw=extract(path,target);expected,report=observe_mdl(target);actual=subprocess.check_output(['target/debug/examples/validate_mdl',str(target)],text=True);assert actual=='\n'.join(expected)+'\n'
        (out/f'model-{j}.rust.tsv').write_text(actual);(out/f'model-{j}.python.tsv').write_text('\n'.join(expected)+'\n');report['path']=path;report['rows_sha256']=hashlib.sha256(actual.encode()).hexdigest();assert all(report[k]==pinned_models[path][k]for k in ['sha256','rows_sha256']);models.append(report)
    assert len(models)==len(pinned_models)
    result={'version':version,'platform':'Win32','python':sys.version,'samples':reports,'target_models':models,'comparison':'all SGB structure/name/TRS/path rows equal; every dependency exists; linked SGB closure present; target MDL scalar/index rows equal'}
    text=json.dumps(result,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

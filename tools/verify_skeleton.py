"""Read-only installed SKLB extraction and complete independent rest-TRS check.

Args: game/sqpack new-output-directory. Uses the independently authored Python
v3 wire observer, not a Rust round trip or third-party Havok parser.
"""
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
from observe_tagfile import read


def observe(path):
    b=path.read_bytes();version=b[4:8]
    if version==b'0031':alpha,offset=struct.unpack_from('<II',b,8)
    elif version==b'0021':alpha,offset=struct.unpack_from('<HH',b,8)
    else:raise ValueError(version)
    observer=read(path);classes=observer.classes;objects=observer.objects
    skeletons=[(i,obj) for i,obj in enumerate(objects) if obj and classes[obj['class']]['name']==b'hkaSkeleton']
    ids=[]
    for obj in objects:
        if obj and classes[obj['class']]['name']==b'hkaAnimationContainer':
            ids.extend(v['ref'] for v in obj['fields'].get(b'skeletons',[]) if v['ref'])
    rows=[f'sklb\t{len(b)}\t{offset}\t{offset-alpha}\t{len(classes)-1}\t{len(objects)-1}\t{len(skeletons)}',
          'animation_skeletons\t'+','.join(map(str,ids))]
    descriptions=[]
    for identity,obj in skeletons:
        f=obj['fields'];name=f.get(b'name');parents=f[b'parentIndices'];bones=f[b'bones'];pose=f[b'referencePose'];n=len(bones)
        assert len(parents)==len(pose)==n
        rows.append(f'skeleton\t{identity}\t{name.hex() if name is not None else "null"}\t{n}')
        lengths=[];names=[];root_count=0
        for i,(parent,bone,trs) in enumerate(zip(parents,bones,pose)):
            name=bone['fields'][b'name'];lock=bone['fields'].get(b'lockTranslation',0)
            assert -1<=parent<n and lock in [0,1] and len(trs)==12 and all(map(math.isfinite,trs))
            visited=set();at=i
            while at!=-1:
                assert at not in visited
                visited.add(at);at=parents[at]
            words=struct.unpack('<12I',struct.pack('<12f',*trs))
            rows.append(f'bone\t{i}\t{name.hex()}\t{parent}\t{lock}\t'+','.join(f'{v:08x}' for v in words))
            lengths.append(sum(v*v for v in trs[4:8]));names.append(name);root_count+=parent==-1
        descriptions.append({'object_id':identity,'bones':n,'roots':root_count,'unique_names':len(set(names)),
                             'minimum_quaternion_squared_length':min(lengths,default=None),'maximum_quaternion_squared_length':max(lengths,default=None),
                             'selected_by_animation_container':identity in ids})
    for identity in ids:assert objects[identity] and classes[objects[identity]['class']]['name']==b'hkaSkeleton'
    text='\n'.join(rows)+'\n'
    return text,{'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'rows_sha256':hashlib.sha256(text.encode()).hexdigest(),
                 'container_version':version.decode()[::-1],'tagfile_version':observer.version,'classes':len(classes)-1,'objects':len(objects)-1,
                 'animation_skeleton_ids':ids,'skeletons':descriptions}


def main():
    root,output=Path(sys.argv[1]),Path(sys.argv[2]);assert not output.resolve().is_relative_to(root.parent.resolve())
    output.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/skeleton-observations.json').read_text())
    version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    reports=[]
    for sample in manifest['samples']:
        target=output/Path(sample['path']).name
        log=subprocess.check_output(['target/debug/examples/local_sqpack',str(root),sample['path'],str(target)],text=True)
        (output/(target.stem+'.extraction.txt')).write_text(log)
        expected,report=observe(target)
        actual=subprocess.check_output(['target/debug/examples/validate_skeleton',str(target)],text=True)
        (output/(target.stem+'.rust.tsv')).write_text(actual);(output/(target.stem+'.python.tsv')).write_text(expected)
        assert expected==actual,f'all-bone mismatch for {sample["path"]}'
        if 'sha256' in sample:
            assert all(report[k]==sample[k] for k in ['sha256','rows_sha256'])
            report['pinned_regression']='equal'
        else:report['pinned_regression']='unregistered sample'
        report['path']=sample['path'];reports.append(report)
    result={'version':version,'platform':'Win32','python':sys.version,'samples':reports,
            'comparison':'all skeleton names, parent indices, boolean flags and all 12 local rest-transform float bits equal'}
    text=json.dumps(result,indent=2)+'\n';(output/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

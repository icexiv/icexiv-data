"""Actual multi-usage-index MDLs: complete reconstruction, geometry and queries.

Args: game/sqpack new-output-dir [manifest.json]. Independent type-3 reconstruction and struct
geometry oracle; no stream remapping, shader interpretation or game execution.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_model import observe as reconstruct
from verify_mdl import observe as geometry
from verify_mdl_semantics import expected as queries


def targets(b):
    stack,=struct.unpack_from('<I',b,4)
    pool,=struct.unpack_from('<I',b,68+stack+4)
    header=68+stack+8+pool
    elements,=struct.unpack_from('<H',b,header+24)
    lods=header+56+elements*32
    extra=lods+180
    for lod in range(b[64]):
        at=lods+lod*60
        ranges=[struct.unpack_from('<HH',b,at+(0 if r==0 else 12+(r-1)*4)) for r in range(5)]
        ranges += [struct.unpack_from('<HH',b,extra+lod*40+(r-5)*4) if b[header+27]&16 else (0,0) for r in range(5,9)]
        for mesh in sorted({m for r,(start,count) in enumerate(ranges) if r!=3 for m in range(start,start+count)}):
            yield lod,mesh


def main():
    root,out=[Path(p).resolve() for p in sys.argv[1:3]]
    repo=Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True,exist_ok=False)
    manifest=Path(sys.argv[3]) if len(sys.argv)==4 else repo/'tests/fixtures/mdl-semantic-variant-observations.json'
    pin=json.loads(manifest.read_text())
    version=(root.parent/'ffxivgame.ver').read_text().strip()
    assert version==pin['version'] and 0<len(pin['samples'])<=16
    reports=[]
    for i,source in enumerate(pin['samples']):
        with (root/source['index']).open('rb') as f:
            f.seek(12)
            hs,=struct.unpack('<I',f.read(4))
            assert hs<=2<<20
            f.seek(hs+8)
            offset,size=struct.unpack('<II',f.read(8))
            assert size<=64<<20 and source['ordinal']<size//16
            f.seek(offset+source['ordinal']*16)
            key,packed,_=struct.unpack('<QII',f.read(16))
        assert not packed&1 and f'{key:016x}'==source['hash'] and (packed>>4)*128==source['offset']
        assert source['dat']==source['index'].removesuffix('.index')+f'.dat{(packed>>1)&7}'
        reconstruct(root,f'@{source["dat"]}:{source["offset"]}',out,i)
        path=out/f'{i}.mdl'
        b=path.read_bytes()
        rows,report=geometry(path,require_positions_inside_bounds=False)
        expected_geometry='\n'.join(rows)+'\n'
        actual_geometry=subprocess.check_output(['target/debug/examples/validate_mdl',str(path)],text=True,timeout=120)
        assert actual_geometry==expected_geometry
        (out/f'{i}.geometry.python.tsv').write_text(expected_geometry)
        (out/f'{i}.geometry.rust.tsv').write_text(actual_geometry)
        expected=[]
        both=only1=different=0
        higher=[]
        for lod,mesh in targets(b):
            expected.append(f'mesh\t{lod}\t{mesh}\n')
            text,declarations,count=queries(b,lod,mesh,include_colors=True)
            expected.append(text)
            identities={(e[3],e[4]) for e in declarations}
            if (7,0) in identities and (7,1) in identities:
                both+=1
                colors={}
                for line in text.splitlines():
                    fields=line.split('\t')
                    if fields[0]=='attribute' and fields[2]=='7' and int(fields[1])<count:
                        colors[int(fields[1]),int(fields[3])]=fields[4]
                different+=sum(colors[v,0]!=colors[v,1] for v in range(count))
            only1 += (7,1) in identities and (7,0) not in identities
            higher.extend({'lod':lod,'mesh':mesh,'usage':u,'usage_index':n} for u,n in sorted(identities) if n)
        oracle=''.join(expected)
        actual=subprocess.check_output(['target/debug/examples/validate_mdl_semantics',str(path),'--all-meshes'],text=True,timeout=120)
        assert actual==oracle
        (out/f'{i}.queries.python.tsv').write_text(oracle)
        (out/f'{i}.queries.rust.tsv').write_text(actual)
        report.update({'file':path.name,'geometry_rows_sha256':hashlib.sha256(expected_geometry.encode()).hexdigest(),
            'queries_sha256':hashlib.sha256(oracle.encode()).hexdigest(),'query_rows':len(oracle.splitlines()),
            'meshes_with_both_colors':both,'meshes_with_only_color1':only1,'vertices_with_distinct_color_inputs':different,
            'nonzero_usage_indices':higher})
        assert report['sha256']==source['sha256']
        reports.append(report)
    result={'version':version,'platform':'Win32','samples':reports,
        'limitations':pin.get('limitations','Four actual bgcommon MDLs and all their declared ordinary geometry; raw semantic identity and scalar queries, not shader color/flow meanings or exhaustive usage-index coverage.')}
    text=json.dumps(result,indent=2)+'\n'
    assert hashlib.sha256(text.encode()).hexdigest()==pin['report_sha256']
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__=='__main__':
    main()

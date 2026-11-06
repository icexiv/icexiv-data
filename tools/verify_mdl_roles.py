"""Independent full MDL/terrain buffers and role-range comparison.

Args: game/sqpack new-output-dir. Hash-named index identities are not recovered
resource paths. Type-3 reconstruction, normal geometry, terrain half4/index/
submesh records and nonzero ElementId scalar words are all compared.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_model import observe as extract
from verify_mdl import observe as observe_model


def observe_roles(path):
    b=path.read_bytes();u16=lambda p:struct.unpack_from('<H',b,p)[0];u32=lambda p:struct.unpack_from('<I',b,p)[0]
    runtime=68+u32(4);h=runtime+8+u32(runtime+4);elements=u16(h+24);lods=h+56+32*elements;extra=lods+180;meshes=extra+(120 if b[h+27]&16 else 0);terrain=meshes+36*u16(h+4)+4*u16(h+6);subs=terrain+20*b[h+26]+16*u16(h+8)
    names=['Normal','Water','Shadow','TerrainShadow','VerticalFog','LightShaft','Glass','MaterialChange','CrestChange'];rows=[];counts=[0]*9;vertices=indices=matches=0
    for lod in range(b[64]):
        ranges=[struct.unpack_from('<HH',b,lods+60*lod+(0 if role==0 else 12+4*(role-1)))for role in range(5)]
        ranges.extend(struct.unpack_from('<HH',b,extra+40*lod+4*(role-5))if b[h+27]&16 else(0,0)for role in range(5,9))
        for role,(start,count)in enumerate(ranges):rows.append(f'range\t{lod}\t{names[role]}\t{start}\t{count}');counts[role]+=count
        vo,io=u32(16+4*lod),u32(28+4*lod);normal=set()
        # For this observed terrain sample, the normal table declares half4
        # position in stream zero. Compare XYZ independently to shadow records.
        for mesh in range(ranges[0][0],ranges[0][0]+ranges[0][1]):
            decl=b[68+mesh*136:68+mesh*136+8]
            if decl[:5]!=bytes([0,0,14,0,0]):continue
            meta=meshes+36*mesh;stride=b[meta+32];offset=u32(meta+20)
            for vertex in range(u16(meta)):normal.add(struct.unpack_from('<4e',b,vo+offset+vertex*stride)[:3])
        start,count=ranges[3]
        for mesh in range(start,start+count):
            ic,first,offset,vc,ss,sc,stride,_=struct.unpack_from('<IIIHHHBB',b,terrain+mesh*20);assert stride==8
            rows.append(f'terrain\t{lod}\t{mesh}\t{ic}\t{first}\t{offset}\t{vc}\t{ss}\t{sc}\t{stride}')
            for vertex in range(vc):
                value=struct.unpack_from('<4e',b,vo+offset+vertex*stride);words=struct.unpack('<4I',struct.pack('<4f',*value));rows.append(f'terrain_vertex\t{lod}\t{mesh}\t{vertex}\t'+','.join(f'{v:08x}'for v in words));vertices+=1;matches+=value[:3]in normal
            values=struct.unpack_from(f'<{ic}H',b,io+first*2);assert all(v<vc for v in values);indices+=ic
            rows.append(f'terrain_indices\t{lod}\t{mesh}\t'+','.join(map(str,values)))
            for sub in range(ss,ss+sc):
                first,count,a,c=struct.unpack_from('<IIHH',b,subs+12*sub);rows.append(f'terrain_submesh\t{sub}\t{first}\t{count}\t{a},{c}')
    return '\n'.join(rows)+'\n',{'role_counts':dict(zip(names,counts)),'terrain_vertices':vertices,'terrain_indices':indices,'terrain_xyz_matching_normal':matches}


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/mdl-role-observations.json').read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version'];reports=[]
    for i,s in enumerate(manifest['samples']):
        index=root/s['dat'].replace('.dat'+s['dat'][-1],'.index')
        with index.open('rb')as f:
            f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert 16*(s['index_ordinal']+1)<=size;f.seek(off+16*s['index_ordinal']);key,p,_=struct.unpack('<QII',f.read(16))
            assert not p&1 and f'{key:016x}'==s['index_hash']and(p>>4)*128==s['offset']and(p>>1)&7==int(s['dat'][-1])
        reconstruction=extract(root,f'@{s["dat"]}:{s["offset"]}',out,i);path=out/f'{i}.mdl';assert reconstruction['sha256']==s['sha256']
        rows,geometry=observe_model(path,require_positions_inside_bounds=False);expected='\n'.join(rows)+'\n';actual=subprocess.check_output(['target/debug/examples/validate_mdl',str(path)],text=True);assert expected==actual;(out/f'{i}.geometry.tsv').write_text(actual)
        expected,roles=observe_roles(path);actual_roles=subprocess.check_output(['target/debug/examples/validate_mdl_roles',str(path)],text=True);assert expected==actual_roles;(out/f'{i}.roles.tsv').write_text(actual_roles)
        report={'sha256':s['sha256'],'rows_sha256':hashlib.sha256(actual.encode()).hexdigest(),'role_rows_sha256':hashlib.sha256(actual_roles.encode()).hexdigest(),'vertices':geometry['vertices'],'position_outside_bounds':geometry['position_outside_bounds'],**roles}
        if'rows_sha256'in s:assert all(report[k]==s[k]for k in ['rows_sha256','role_rows_sha256'])
        reports.append(report)
    text=json.dumps({'version':version,'platform':'Win32','samples':reports},indent=2)+'\n';assert hashlib.sha256(text.encode()).hexdigest()==manifest['results_sha256'];(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

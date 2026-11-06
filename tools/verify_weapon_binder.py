"""Readonly weapon IMC/AVFX/MDL/SKLB metadata correspondence, not pose assembly.

Args: game/sqpack new-output-dir. Independent type-2/type-3 reconstruction,
complete IMC/MDL/skeleton rows, and binder selector/ElementId/bone-name matching.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
from find_vfx_samples import standard
from verify_avfx import fields,label
from verify_model import observe as extract_model
from verify_mdl import observe as observe_model
from verify_skeleton import observe as observe_skeleton
from observe_tagfile import read as tagfile


def number(fs,tag):
    matched=[p for t,p,_,_ in fs if label(t)==tag];assert len(matched)<=1
    return struct.unpack('<i',matched[0])[0]if matched else None
def opt(v):return 'none'if v is None else str(v)
def float_word(fs,tag):
    matched=[p for t,p,_,_ in fs if label(t)==tag];assert len(matched)<=1
    return f'{struct.unpack("<I",matched[0])[0]:08x}'if matched else'none'


def observe_binders(raw,elements=None,skeleton=None):
    rows=[];stats={'properties':0,'slots':{},'bind_point_ids':[],'element_candidates':0,'bone_candidates':0}
    for t,p,owner,_ in fields(raw[8:],8):
        if label(t)!=b'Bind':continue
        for t,p,offset,_ in fields(p,owner+8):
            slot={b'PrpS':'Start',b'Prp1':'First',b'Prp2':'Second',b'PrpG':'Goal'}.get(label(t))
            if slot is None:continue
            fs=fields(p,offset+8);names=[p for t,p,_,_ in fs if label(t)==b'Name'];assert len(names)<=1
            name='none'
            if names:
                data=names[0]
                if data:
                    end=data.index(0);assert not any(data[end:]);data=data[:end]
                name=data.hex()
            pos=[(p,q)for t,p,q,_ in fs if label(t)==b'Pos'];assert len(pos)<=1;pos_count=str(len(fields(pos[0][0],pos[0][1]+8)))if pos else'none'
            values=[owner,offset,slot,opt(number(fs,b'BPT')),opt(number(fs,b'BPTP')),opt(number(fs,b'BPID')),name,opt(number(fs,b'GenD')),opt(number(fs,b'CoUF')),opt(number(fs,b'BCT')),opt(number(fs,b'bRng')),opt(number(fs,b'RnPT')),float_word(fs,b'RnPX'),float_word(fs,b'RnPY'),float_word(fs,b'RnPZ'),float_word(fs,b'RnRd'),pos_count]
            rows.append('binder\t'+'\t'.join(map(str,values)));stats['properties']+=1;stats['slots'][slot]=stats['slots'].get(slot,0)+1;identity=number(fs,b'BPID');stats['bind_point_ids'].append(identity)
            if elements is not None and skeleton is not None and identity is not None:
                matches=bone_matches=0
                for i,eid,parent,trs in elements:
                    if eid!=identity:continue
                    matches+=1;rows.append(f'element_candidate\t{offset}\t{i}\t{eid}\t{parent}\t{trs}')
                    for object_id,bones in skeleton:
                        for j,bone in enumerate(bones):
                            if bone.hex()!=parent:continue
                            bone_matches+=1;rows.append(f'bone_candidate\t{offset}\t{object_id}\t{j}\t{parent}')
                rows.append(f'candidate_counts\t{offset}\t{matches}\t{bone_matches}');assert matches==bone_matches==1
                stats['element_candidates']+=matches;stats['bone_candidates']+=bone_matches
                # Only this observed numeric selector combination is used for
                # correspondence evidence. No global bind-point resolver.
                assert number(fs,b'BPT')==0 and number(fs,b'BPTP')==3 and name==b'null'.hex()
    return '\n'.join(rows)+('\n'if rows else''),stats


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/weapon-binder-observations.json').read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    with (root/'ffxiv/040000.win32.index').open('rb')as f:
        f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off);index={key:packed for key,packed,_ in struct.iter_unpack('<QII',f.read(size))if not packed&1}
    def extract(path,target):
        directory,filename=path.rsplit('/',1);jam=lambda s:~zlib.crc32(s.encode())&0xffffffff;packed=index[jam(directory)<<32|jam(filename)]
        dat=root/f'ffxiv/040000.win32.dat{(packed>>1)&7}';entry=(packed>>4)*128
        with dat.open('rb')as f:raw=standard(f,entry)
        assert raw is not None
        subprocess.check_output(['target/debug/examples/local_sqpack',str(root),path,str(target)],text=True);assert target.read_bytes()==raw;return raw
    reports=[]
    for i,s in enumerate(manifest['samples']):
        imc=extract(s['imc_path'],out/f'{i}.imc');assert hashlib.sha256(imc).hexdigest()==s['imc_sha256'];variants,mask=struct.unpack_from('<HH',imc);parts=[b for b in range(16)if mask>>b&1];records=list(struct.iter_unpack('<BBHBB',imc[4:]));assert len(records)==(variants+1)*len(parts)
        expected=[f'file\t{s["imc_path"]}\t{len(imc)}\t{variants}\t{mask:#06x}'];nonzero=[]
        for n,(material,decal,flags,vfx,animation)in enumerate(records):
            values=[n//len(parts),parts[n%len(parts)],material,decal,flags,flags&1023,flags>>10,vfx,animation];expected.append('entry\t'+'\t'.join(map(str,values)))
            if vfx==s['vfx_id']:nonzero.append(n//len(parts))
        assert nonzero;actual=subprocess.check_output(['target/debug/examples/validate_imc',str(root),s['imc_path']],text=True);assert actual=='\n'.join(expected)+'\n';(out/f'{i}.imc.tsv').write_text(actual)
        avfx_path=out/f'{i}.avfx';raw=extract(s['avfx_path'],avfx_path);assert hashlib.sha256(raw).hexdigest()==s['avfx_sha256']
        reconstruction=extract_model(root,s['mdl_path'],out,f'model-{i}');model=out/f'model-{i}.mdl';model_rows,model_report=observe_model(model);actual=subprocess.check_output(['target/debug/examples/validate_mdl',str(model)],text=True);assert actual=='\n'.join(model_rows)+'\n';(out/f'{i}.mdl.tsv').write_text(actual)
        elements=[]
        for line in model_rows:
            row=line.split('\t')
            if row[0]=='element':elements.append((len(elements),int(row[1]),row[2],row[3]))
        sklb=out/f'{i}.sklb';sraw=extract(s['sklb_path'],sklb);skeleton_rows,skeleton_report=observe_skeleton(sklb);actual=subprocess.check_output(['target/debug/examples/validate_skeleton',str(sklb)],text=True);assert actual==skeleton_rows;(out/f'{i}.sklb.tsv').write_text(actual)
        graph=tagfile(sklb);ids=[v['ref']for obj in graph.objects if obj and graph.classes[obj['class']]['name']==b'hkaAnimationContainer'for v in obj['fields'].get(b'skeletons',[])]
        bones=[(identity,[b['fields'][b'name']for b in graph.objects[identity]['fields'][b'bones']])for identity in ids]
        expected,report=observe_binders(raw,elements,bones);actual=subprocess.check_output(['target/debug/examples/validate_binder',str(avfx_path),str(model),str(sklb)],text=True);assert actual==expected;(out/f'{i}.binder.rust.tsv').write_text(actual);(out/f'{i}.binder.python.tsv').write_text(expected)
        report.update({'imc_path':s['imc_path'],'avfx_path':s['avfx_path'],'mdl_path':s['mdl_path'],'sklb_path':s['sklb_path'],'nonzero_vfx_variants':nonzero,'mdl_sha256':reconstruction['sha256'],'sklb_sha256':hashlib.sha256(sraw).hexdigest(),'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),'model_vertices':model_report['vertices'],'skeleton_bones':sum(len(b)for _,b in bones)})
        assert all(report[k]==manifest['results'][i][k]for k in ['mdl_sha256','sklb_sha256','rows_sha256'])
        reports.append(report)
    text=json.dumps({'version':version,'platform':'Win32','samples':reports,'comparison':'all IMC/binder/MDL/skeleton rows equal; each observed BPID matches one owner ElementId and one selected skeleton bone name; no pose composition'},indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

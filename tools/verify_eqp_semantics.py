"""Readonly EQP field and MDL-local attribute-bit comparisons, not assembly.

Args: game/sqpack new-output-directory [fixture.json]. Independent sequential EQP expansion,
type-2/type-3 reconstruction, full MDL rows and name-to-mask-bit observations.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from find_vfx_samples import standard
from verify_model import locate,observe as extract_model
from verify_mdl import observe as observe_model


def main():
    root,out=map(Path,sys.argv[1:3]);repo=Path(__file__).resolve().parent.parent;assert not out.resolve().is_relative_to(root.parent.resolve()) and not out.resolve().is_relative_to(repo);out.mkdir(parents=True,exist_ok=False)
    fixture_root=repo/'tests/fixtures';fixture=Path(sys.argv[3])if len(sys.argv)>3 else fixture_root/'eqp-semantic-observations.json';manifest=json.loads(fixture.read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    queries=manifest.get('additional_attribute_queries',[]);assert len(queries)<=64 and all(len(q.encode())<=256 for q in queries)
    pin=json.loads((fixture_root/'eqp-observations.json').read_text());resource='chara/xls/equipmentparameter/equipmentparameter.eqp';target=out/'table.eqp';dat,offset=locate(root,resource)
    with dat.open('rb')as f:raw=standard(f,offset)
    subprocess.check_output(['target/debug/examples/local_sqpack',str(root),resource,str(target)]);assert target.read_bytes()==raw and hashlib.sha256(raw).hexdigest()==pin['sha256']
    bitmap,=struct.unpack_from('<Q',raw);stored=0;rows=[];present=0;selectors=[{},{}]
    for block in range(64):
        if not bitmap>>block&1:rows.extend(f'fields\t{block*160+i}\tabsent'for i in range(160)if block*160+i);continue
        values=struct.unpack_from('<160Q',raw,stored*1280);stored+=1
        for i,v in enumerate(values):
            identity=block*160+i
            if identity==0:continue
            fields=[(v>>10)&1,(v>>9)&1,(v>>8)&1,(v>>21)&1,(v>>3)&3,(v>>18)&3,(v>>25)&3,(v>>33)&3,(v>>27)&1,(v>>35)&1]
            rows.append(f'fields\t{identity}\t{v:016x}\t'+','.join(map(str,fields)));present+=1
            if manifest.get('ear_tail_fields',False):
                raw_fields=[v//(2**bit)%2 for bit in[0,13,16,22,40]]+[v//(2**41)%8]+[v//(2**bit)%2 for bit in[50,51,52,53,56,57,59]]
                rows.append(f'ear_tail\t{identity}\t'+','.join(map(str,raw_fields)))
            for hist,value in zip(selectors,fields[6:8]):hist[str(value)]=hist.get(str(value),0)+1
    expected='\n'.join(rows)+'\n';args=['--ear-tail']if manifest.get('ear_tail_fields',False)else[];actual=subprocess.check_output(['target/debug/examples/validate_eqp_fields',str(target),*args],text=True);assert actual==expected;field_rows_sha256=hashlib.sha256(actual.encode()).hexdigest();(out/'fields.rust.tsv').write_text(actual);(out/'fields.python.tsv').write_text(expected)
    reports=[]
    for i,s in enumerate(manifest['models']):
        reconstruction=extract_model(root,s['path'],out,i);p=out/f'{i}.mdl';rows,report=observe_model(p);expected_model='\n'.join(rows)+'\n';actual_model=subprocess.check_output(['target/debug/examples/validate_mdl',str(p)],text=True);assert expected_model==actual_model;(out/f'{i}.geometry.tsv').write_text(actual_model)
        attributes=[r.split('\t')for r in rows if r.startswith('attribute\t')];names={r[2]:int(r[1])for r in attributes};assert len(names)==len(attributes)and len(attributes)<=32
        expected=[f'name\t{index}\t{name}\t{1<<index:08x}'for name,index in names.items()]
        for name in[b'atr_nek',b'atr_hij',b'atr_ude',b'atr_sne',b'atr_hiz',b'atr_tlh',b'atr_tls',b'atr_inr',b'missing_attribute']+[q.encode()for q in queries]:
            index=names.get(name.hex());bit='none'if index is None else f'{1<<index:08x}';expected.append(f'lookup\t{name.hex()}\t{bit}')
        for row in rows:
            r=row.split('\t')
            if r[0]=='submesh':expected.append(f'submesh_mask\t{r[1]}\t{int(r[4]):08x}')
        expected='\n'.join(expected)+'\n';actual=subprocess.check_output(['target/debug/examples/validate_mdl_attributes',str(p),*queries],text=True);assert actual==expected;(out/f'{i}.attributes.tsv').write_text(actual)
        result={'path':s['path'],'sha256':reconstruction['sha256'],'rows_sha256':hashlib.sha256(actual_model.encode()).hexdigest(),'attribute_rows_sha256':hashlib.sha256(actual.encode()).hexdigest(),'vertices':report['vertices'],'attribute_names':[bytes.fromhex(r[2]).decode('ascii')for r in attributes]}
        if'sha256'in s:assert result['sha256']==s['sha256']
        reports.append(result)
    result={'version':version,'platform':'Win32','eqp_sha256':pin['sha256'],'field_rows_sha256':field_rows_sha256,'present_entries':present,'hand_foot_selector_histograms':selectors,'models':reports}
    text=json.dumps(result,indent=2)+'\n'
    if'results_sha256'in manifest:assert hashlib.sha256(text.encode()).hexdigest()==manifest['results_sha256']
    (out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

"""Independent EXH/EXD numeric/string observations and exact extraction.

Args: game/sqpack new-output-dir. Strings in variant-2 are observed relative to
the common pool after all subrows, not an individual subrow's fixed-data end.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from find_excel_subrow import ExcelInstallation

SUFFIX=['','_ja','_en','_de','_fr','_chs','_cht','_ko']
SCALARS={1:('B','b'),2:('b','i8'),3:('B','u8'),4:('h','i16'),5:('H','u16'),6:('i','i32'),7:('I','u32'),9:('I','f'),10:('q','i64'),11:('Q','u64')}


def header(b):
    assert b[:4]==b'EXHF' and struct.unpack_from('>H',b,4)[0]==3
    fixed,c,p,l=struct.unpack_from('>4H',b,6);variant=b[17];rows,=struct.unpack_from('>I',b,20);assert variant in[1,2]
    cols=list(struct.iter_unpack('>HH',b[32:32+c*4]));pages=list(struct.iter_unpack('>II',b[32+c*4:32+c*4+p*8]));langs=list(v for v, in struct.iter_unpack('<H',b[32+c*4+p*8:32+c*4+p*8+l*2]));assert len(cols)==c and len(pages)==p and len(langs)==l
    result=[f'header\t{fixed}\t{variant}\t{rows}\t{c}\t{p}\t{l}']
    result.extend(f'column\t{i}\t{t}\t{o}'for i,(t,o)in enumerate(cols));result.extend(f'page\t{s}\t{n}'for s,n in sorted(pages));result.extend(f'language\t{lang}\t{SUFFIX[lang]}'for lang in langs)
    return result,{'fixed':fixed,'variant':variant,'columns':cols,'pages':pages,'languages':langs,'declared_rows':rows}


def page(b,h):
    assert b[:4]==b'EXDF'and struct.unpack_from('>H',b,4)[0]==2
    index,data=struct.unpack_from('>II',b,8);assert index%8==0 and 32+index+data==len(b)
    parents=sorted(struct.iter_unpack('>II',b[32:32+index]));rows=[];stats={'parents':len(parents),'rows':0,'max_subrows_per_parent':0,'strings':0,'nonempty_strings':0,'string_bytes':0,'uint64_nonzero_high_words':0,'float_values':0,'fields':0}
    for identity,offset in parents:
        size,count=struct.unpack_from('>IH',b,offset);start=offset+6;end=start+size;assert end<=len(b) and offset>=32+index
        n=count if h['variant']==2 else 1;assert h['variant']==2 or count==1;stride=h['fixed']+(2 if h['variant']==2 else 0);pool=start+n*stride;assert pool<=end
        stats['max_subrows_per_parent']=max(count,stats['max_subrows_per_parent'])
        for i in range(n):
            fixed=start+i*stride+(2 if h['variant']==2 else 0);sid=struct.unpack_from('>H',b,fixed-2)[0]if h['variant']==2 else'none';values=[]
            for kind,off in h['columns']:
                at=fixed+off;stats['fields']+=1
                if kind==0:
                    relative,=struct.unpack_from('>I',b,at);p=pool+relative;assert p<end;nul=b.index(0,p,end);value=b[p:nul];values.append('s:'+value.hex());stats['strings']+=1;stats['nonempty_strings']+=bool(value);stats['string_bytes']+=len(value)
                elif 25<=kind<=32:values.append('b:'+str(int(bool(b[at]&(1<<(kind-25))))))
                else:
                    code,prefix=SCALARS[kind];value,=struct.unpack_from('>'+code,b,at)
                    if kind==9:values.append(f'f:{value:08x}');stats['float_values']+=1
                    elif kind==1:values.append('b:'+str(int(bool(value))))
                    else:values.append(f'{prefix}:{value}')
                    if kind==11:stats['uint64_nonzero_high_words']+=value>>32!=0
            rows.append(f'row\t{identity}\t{sid}\t'+'\t'.join(values));stats['rows']+=1
    return [f'exd\t{len(b)}\t{len(parents)}\t{stats["rows"]}']+rows,stats


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/excel-observations.json').read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version'];install=ExcelInstallation(root);reports=[]
    def extract(path,target):
        raw=install.read(path);log=subprocess.check_output(['target/debug/examples/local_sqpack',str(root),path,str(target)],text=True);target.with_suffix(target.suffix+'.extraction.txt').write_text(log);assert target.read_bytes()==raw;return raw
    for source in manifest['sheets']:
        name=source['name'];hp=out/f'{name}.exh';raw=extract(f'exd/{name.lower()}.exh',hp);expected,h=header(raw);assert source['language']in h['languages'];pages=[];page_paths=[]
        header_hash=hashlib.sha256(raw).hexdigest()
        for start in source['pages']:
            assert start in dict(h['pages']);target=out/f'{name}-{start}.exd';page_paths.append(target);path=f'exd/{name.lower()}_{start}{SUFFIX[source["language"]]}.exd';raw=extract(path,target);r,stats=page(raw,h);expected.extend(r);stats.update({'path':path,'sha256':hashlib.sha256(raw).hexdigest()});pages.append(stats)
        if len(source['pages'])==len(h['pages']):assert sum(p['rows']for p in pages)==h['declared_rows']
        actual=subprocess.check_output(['target/debug/examples/validate_excel',str(hp),*map(str,page_paths)],text=True);text='\n'.join(expected)+'\n';(out/f'{name}.rust.tsv').write_text(actual);(out/f'{name}.python.tsv').write_text(text);assert actual==text,f'complete field mismatch for {name}'
        report={'name':name,'header_sha256':header_hash,'rows_sha256':hashlib.sha256(text.encode()).hexdigest(),'variant':h['variant'],'columns':len(h['columns']),'pages':pages}
        if'header_sha256'in source:assert all(report[k]==source[k]for k in ['header_sha256','rows_sha256'])and[p['sha256']for p in pages]==source['page_sha256']
        reports.append(report)
    text=json.dumps({'version':version,'platform':'Win32','python':sys.version,'sheets':reports,'comparison':'all independently extracted bytes, EXH metadata and every EXD typed field/string byte equal'},indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

"""Independent recursive AVFX byte/half-float observations vs Rust asset views.

Args: game/sqpack new-output-directory. No FFXIV parser dependency or execution.
All extracted data and full vertex/key reports remain outside the repository.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from find_vfx_samples import standard


def fields(b,origin):
    out=[];p=0
    while p<len(b):
        assert len(b)-p>=8
        tag,size=struct.unpack_from('<4sI',b,p);end=p+8+size;next_pos=end+(-size)%4
        assert next_pos<=len(b)
        out.append((tag[::-1],b[p+8:end],origin+p,b[end:next_pos]));p=next_pos
    assert p==len(b);return out


def label(t):return t.strip(b'\0')
def bits(values):return ','.join(f'{v:08x}'for v in struct.unpack('<'+str(len(values))+'I',struct.pack('<'+str(len(values))+'f',*values)))
def ints(b):return struct.unpack('<'+str(len(b)//4)+'i',b)
def only(fs,tag):
    values=[b for t,b,_,_ in fs if label(t)==tag];assert len(values)==1;return values[0]


def observe(raw):
    outer=fields(raw,0);assert len(outer)==1 and outer[0][0]==b'AVFX';root=fields(outer[0][1],8)
    node_tags=[b'Schd',b'TmLn',b'Emit',b'Ptcl',b'Efct',b'Bind'];nodes=[r for r in root if label(r[0])in node_tags]
    textures=[b.rstrip(b'\0')for t,b,_,_ in root if label(t)==b'Tex'];models=[r for r in root if label(r[0])==b'Modl']
    lines=[f'avfx\t{len(raw)}\t{len(nodes)}\t{len(textures)}\t{len(models)}']
    for i,t in enumerate(textures):lines.append(f'texture\t{i}\t{t.hex()}')
    stats={'nodes':len(nodes),'textures':len(textures),'models':len(models),'blocks':0,'keys':0,'draw_vertices':0,'emit_vertices':0,'indices':0,'clips':0}
    def walk(fs,depth):
        assert depth<32
        for t,b,offset,pad in fs:
            name=label(t);stats['blocks']+=1;lines.append(f'block\t{offset}\t{t.hex()}\t{len(b)}\t{pad.hex()}')
            if name==b'Clip':
                assert len(b)==164;tag=b[:4][::-1];iv=ints(b[4:20]);rv=struct.unpack_from('<4f',b,20);names=[b[36+i*32:68+i*32].split(b'\0',1)[0].hex()for i in range(4)]
                lines.append(f'clip\t{offset}\t{tag.hex()}\t'+','.join(map(str,iv))+'\t'+bits(rv)+'\t'+','.join(names));stats['clips']+=1;continue
            # Independent framing inventory: attempt exact nested framing only
            # for non-scalar, non-opaque-array fields. Name is a string union.
            if name in [b'Keys',b'Cols',b'Frms',b'VDrw',b'VIdx',b'VNum',b'VEmt',b'Name',b'SdNm',b'TLst'] or len(b)<8:continue
            if not 0<=b[3]<=127:continue
            # Four-byte tags must be printable or NUL. Binary array payloads
            # are excluded above, not silently reclassified as missing data.
            if not all(x==0 or 32<=x<127 for x in b[:4]):continue
            nested=fields(b,offset+8)
            if any(label(t)==b'Keys'for t,_,_,_ in nested):
                keys=only(nested,b'Keys');assert len(keys)%16==0
                assert struct.unpack('<i',only(nested,b'KeyC'))[0]==len(keys)//16
                for time,kind,x,y,z in struct.iter_unpack('<hH3f',keys):lines.append(f'key\t{offset}\t{time}\t{kind}\t'+bits([x,y,z]));stats['keys']+=1
            walk(nested,depth+1)
    for t,b,offset,_ in nodes:
        fs=fields(b,offset+8);name=label(t);variant='none'
        if name in [b'Emit',b'Ptcl',b'Efct',b'Bind']:variant=str(struct.unpack('<i',only(fs,{b'Emit':b'EVT',b'Ptcl':b'PrVT',b'Efct':b'EfVT',b'Bind':b'BnVr'}[name]))[0])
        lines.append(f'node\t{offset}\t{t.hex()}\t{variant}');walk(fs,0)
        if name==b'Schd':
            for trigger,tag in enumerate([b'Item',b'Trgr']):
                records=[r for r in fs if label(r[0])==tag]
                if not records:continue
                last=records[-1];columns=fields(last[1],last[2]+8);assert len(columns)%3==0
                for i in range(len(columns)//3):
                    row=columns[i*3:i*3+3];assert list(map(lambda r:label(r[0]),row))==[b'bEna',b'StTm',b'TlNo']
                    enabled,start,timeline=[struct.unpack('<i',r[1])[0]for r in row];lines.append(f'schedule\t{offset}\t{trigger}\t{i}\t{enabled}\t{start}\t{timeline}')
        if name==b'TmLn':
            records=[r for r in fs if label(r[0])==b'Item']
            if records:
                last=records[-1];columns=fields(last[1],last[2]+8);rows=[]
                for t,p,_,_ in columns:
                    if label(t)==b'bEna':rows.append({})
                    assert rows and label(t)not in rows[-1];rows[-1][label(t)]=struct.unpack('<i',p)[0]
                for i,row in enumerate(rows):
                    values=[row[t]for t in [b'bEna',b'StTm',b'EdTm',b'BdNo',b'EfNo',b'EmNo',b'Plfm']]+[row.get(b'ClNo','none')]
                    lines.append(f'timeline_item\t{offset}\t{i}\t'+'\t'.join(map(str,values)))
    for i,(_,b,offset,_) in enumerate(models):
        fs=fields(b,offset+8);data={label(t):v for t,v,_,_ in fs};draw=data.get(b'VDrw',b'');idx=data.get(b'VIdx',b'');emit=data.get(b'VEmt',b'');nums=data.get(b'VNum',b'')
        assert len(draw)%36==len(idx)%6==len(emit)%28==0 and len(nums)//2==len(emit)//28
        stats['draw_vertices']+=len(draw)//36;stats['emit_vertices']+=len(emit)//28;stats['indices']+=len(idx)//2
        lines.append(f'model\t{i}\t{len(draw)//36}\t{len(idx)//2}\t{len(emit)//28}')
        for row in struct.iter_unpack('<4e4B4B4B8e',draw):
            p,n,t,color,uv=row[:4],row[4:8],row[8:12],row[12:16],row[16:]
            lines.append(f'draw\t{i}\t'+bits(p)+'\t'+','.join(str(v-128)for v in n)+'\t'+','.join(str(v-128)for v in t)+'\t'+bytes(color).hex()+'\t'+bits(uv))
        indices=[v for v, in struct.iter_unpack('<H',idx)];assert all(v<len(draw)//36 for v in indices)
        lines.append(f'indices\t{i}\t'+','.join(map(str,indices)))
        for row,(num,) in zip(struct.iter_unpack('<6f4B',emit),struct.iter_unpack('<H',nums)):
            lines.append(f'emit\t{i}\t'+bits(row[:3])+'\t'+bits(row[3:6])+'\t'+bytes(row[6:]).hex()+f'\t{num}')
    return '\n'.join(lines)+'\n',stats


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/avfx-observations.json').read_text());version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version'];reports=[]
    for i,source in enumerate(manifest['samples']):
        target=out/f'{i}.avfx';log=subprocess.check_output(['target/debug/examples/extract_entry',str(root),source['dat'],str(source['offset']),str(target)],text=True);(out/f'{i}.extraction.txt').write_text(log)
        with (root/source['dat']).open('rb') as f:raw=standard(f,source['offset'])
        assert raw==target.read_bytes() and hashlib.sha256(raw).hexdigest()==source['sha256']
        expected,stats=observe(raw);actual=subprocess.check_output(['target/debug/examples/validate_avfx',str(target)],text=True)
        (out/f'{i}.rust.tsv').write_text(actual);(out/f'{i}.python.tsv').write_text(expected)
        assert actual==expected,f'all-field mismatch in {i}'
        stats.update(source);stats['rows_sha256']=hashlib.sha256(expected.encode()).hexdigest();assert stats['rows_sha256']==manifest['rows_sha256'][i];reports.append(stats)
    text=json.dumps({'version':version,'platform':'Win32','python':sys.version,'samples':reports,'comparison':'all node/curve/clip/geometry rows equal, all independently extracted bytes equal'},indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

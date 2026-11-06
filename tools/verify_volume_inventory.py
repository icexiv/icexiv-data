"""Read-only volume inventory, complete pixels and explicit incomplete entries.

Args: game/sqpack new-output-dir [--all-records]. Same selection ceiling as the texture header
inventory: 2M records, 200K/index, 128 indexes, 64 MiB/index table. Each volume
is capped at 16 MiB and cumulative decoded groups at 128 MiB. Pixel output is
capped at 64M pixels. No missing bytes are created or guessed.
--all-records removes only the 200K/index selection ceiling; the 2M total budget
remains mandatory, and visiting every discovered main-table record is asserted.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
import PIL
from verify_tex import observe


def groups(f,entry):
    f.seek(entry)
    hs,kind,target,_,_,count=struct.unpack('<6I',f.read(24))
    assert kind==4 and 128<=hs<=2<<20 and target<=16<<20 and 0<count<=2048
    f.seek(entry)
    header=f.read(hs)
    assert len(header)==hs and 24+count*20<=hs
    rows=list(struct.iter_unpack('<5I',header[24:24+count*20]))
    n=sum(r[4]for r in rows)
    assert 24+count*20+n*2<=hs
    spans=struct.unpack_from(f'<{n}H',header,24+count*20)
    f.seek(entry+hs)
    assert rows[0][0]==80
    prefix=f.read(80)
    outputs=[]
    at=total=0
    for relative,stored_span,declared,first,blocks in rows:
        assert first==at and blocks and stored_span==sum(spans[at:at+blocks])
        position=entry+hs+relative
        decoded=bytearray()
        for _ in range(blocks):
            f.seek(position)
            bh,_,stored,expanded=struct.unpack('<4I',f.read(16))
            length=expanded if stored==32000 else stored
            assert 16<=bh<=spans[at] and length<=spans[at]-bh and expanded<=65535
            f.seek(position+bh)
            raw=f.read(length)
            assert len(raw)==length
            if stored!=32000:
                d=zlib.decompressobj(-15)
                raw=d.decompress(raw,65536)
                assert d.eof and not d.unused_data and not d.unconsumed_tail
            assert len(raw)==expanded
            decoded.extend(raw)
            total+=expanded
            assert total+80<=target
            position+=spans[at]
            at+=1
        assert declared in (0,len(decoded))
        outputs.append(bytes(decoded))
    assert at==len(spans)
    return prefix,outputs,target


def main():
    root,out=[Path(p).resolve()for p in sys.argv[1:3]]
    assert len(sys.argv) in (3,4) and (len(sys.argv)==3 or sys.argv[3]=='--all-records')
    all_records=len(sys.argv)==4
    repo=Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True,exist_ok=False)
    indexes=sorted(root.glob('*/*.win32.index'),key=lambda p:(p.parent.name!='ffxiv',str(p)))
    assert len(indexes)<=128
    remaining=2_000_000
    decoded_budget=128<<20
    pixel_budget=64_000_000
    packs,samples=[],[]
    paths={'declared':[],'packed':[]}
    seen=set()
    for index in indexes:
        if not remaining:
            break
        with index.open('rb') as f:
            f.seek(12)
            hs,=struct.unpack('<I',f.read(4))
            assert hs<=2<<20
            f.seek(hs+8)
            offset,size=struct.unpack('<II',f.read(8))
            assert size<=64<<20 and size%16==0
            f.seek(offset)
            table=f.read(size)
            assert len(table)==size
        total=size//16
        if all_records:
            assert total<=remaining, 'all-record inventory exceeds total record budget'
            selected=total
        else:
            selected=min(total,200000,remaining)
        remaining-=selected
        stat={'index':str(index.relative_to(root)),'records':total,'selected':selected,
            'index_table_sha256':hashlib.sha256(table).hexdigest(),'volume_headers':0,'duplicate_volume_locations':0}
        with contextlib.ExitStack()as stack:
            files={}
            for i in range(selected):
                ordinal=i*total//selected
                key,packed,_=struct.unpack_from('<QII',table,ordinal*16)
                if packed&1:
                    continue
                dat=index.with_suffix(f'.dat{(packed>>1)&7}')
                if dat not in files:
                    files[dat]=stack.enter_context(dat.open('rb'))
                f=files[dat]
                entry=(packed>>4)*128
                f.seek(entry)
                hs,kind,target=struct.unpack('<3I',f.read(12))
                if kind!=4:
                    continue
                assert 128<=hs<=2<<20
                f.seek(entry+hs)
                tex=f.read(80)
                assert len(tex)==80
                flags,fmt,w,h,depth,mips,array=struct.unpack_from('<II3HBB',tex)
                if flags&0x13c00000!=0x1000000:
                    continue
                stat['volume_headers']+=1
                identity=(str(dat.relative_to(root)),entry)
                if identity in seen:
                    stat['duplicate_volume_locations']+=1
                    continue
                seen.add(identity)
                assert 80<=target<=16<<20 and target-80<=decoded_budget
                prefix,payloads,target=groups(f,entry)
                assert prefix==tex and fmt in (0x1130,0x1450,0x1451) and array==0
                count=mips&127
                assert 0<count<=13 and len(payloads)==count
                offsets=list(struct.unpack_from('<13I',tex,28))[:count]
                byte_width=1 if fmt==0x1130 else 4
                slices=[max(1,w>>m)*max(1,h>>m)*byte_width for m in range(count)]
                expected=[s*max(1,depth>>m)for m,s in enumerate(slices)]
                double=[s*max(1,depth>>(2*m))for m,s in enumerate(slices)]
                actual=list(map(len,payloads))
                decoded_budget-=sum(actual)
                assert decoded_budget>=0
                canonical=[80+sum(expected[:m])for m in range(count)]
                profile='declared' if offsets==canonical else 'packed' if offsets==[80]*count else 'unknown'
                assert profile!='unknown' and 80+sum(expected)==target, {
                    'dat':identity[0],'offset':entry,'dimensions':[w,h,depth],'mips':count,
                    'format':hex(fmt),'offsets':offsets,'expected':expected,'target':target,'groups':actual}
                sample={'index':stat['index'],'ordinal':ordinal,'hash':f'{key:016x}',
                    'dat':identity[0],'offset':entry,'width':w,'height':h,'depth':depth,'mips':count,
                    'format':hex(fmt),'declared_bytes':target,'decoded_bytes':80+sum(actual),
                    'offset_profile':profile,'declared_mip_offsets':offsets,
                    'group_bytes':actual,'mip_slot_bytes':expected,'double_depth_shift_profile':double,
                    'header_sha256':hashlib.sha256(tex).hexdigest(),
                    'group_sha256':[hashlib.sha256(p).hexdigest()for p in payloads]}
                target_path=out/f'{len(samples)}.tex'
                extracted=subprocess.run(['target/debug/examples/extract_entry',str(root),identity[0],str(entry),str(target_path)],text=True,capture_output=True,timeout=120)
                if actual==expected:
                    assert extracted.returncode==0
                    raw=tex+b''.join(payloads)
                    assert target_path.read_bytes()==raw
                    pixels=sum(max(1,w>>m)*max(1,h>>m)*max(1,depth>>m)for m in range(count))
                    pixel_budget-=pixels
                    assert pixel_budget>=0
                    sample.update({'status':'complete','sha256':hashlib.sha256(raw).hexdigest(),'pixel_sample':len(paths[profile])})
                    paths[profile].append(target_path)
                else:
                    assert extracted.returncode!=0 and 'decoded size disagrees with entry size'in extracted.stderr
                    assert not target_path.exists()
                    sample.update({'status':'incomplete','missing_bytes':target-80-sum(actual),
                        'matches_double_depth_shift_profile':actual==double})
                samples.append(sample)
        packs.append(stat)
    assert sum(map(len,paths.values()))<=512
    rust={}
    expected={profile:[]for profile in paths}
    for profile,files in paths.items():
        if files:
            flags=['--packed-volume']if profile=='packed'else[]
            rust[profile]=subprocess.check_output(['target/debug/examples/validate_tex',str(out/('pixels-'+profile)),*flags,*map(str,files)],text=True,timeout=120)
    for sample in samples:
        if sample['status']!='complete':
            continue
        i=sample['pixel_sample']
        profile=sample['offset_profile']
        rows,report=observe(paths[profile][i].read_bytes(),i,out/('pixels-'+profile),packed_volume=profile=='packed')
        sample['pixels']=report['pixels']
        sample['images']=report['images']
        sample['rows_sha256']=hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest()
        expected[profile].extend(rows)
    for profile,actual in rust.items():
        oracle='\n'.join(expected[profile])+'\n'
        assert actual==oracle
        (out/(profile+'.rust.tsv')).write_text(actual)
        (out/(profile+'.python.tsv')).write_text(oracle)
    complete=sum(map(len,paths.values()))
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','pillow':PIL.__version__,
        'packs':packs,'samples':samples,'complete_entries':complete,'incomplete_entries':len(samples)-complete,
        'images_verified':sum(s.get('images',0)for s in samples),'pixels_verified':sum(s.get('pixels',0)for s in samples),
        'limitations':'Bounded selected-index volume entries, complete bytes/pixels only for fully backed groups; double-depth profile is a length observation, not missing-byte reconstruction or native-path proof.'}
    if all_records:
        assert len(packs)==len(indexes) and all(p['selected']==p['records']for p in packs)
        result['index_coverage']='All records in each discovered primary Win32 index main hash table; not index2, orphan DAT records, unknown names or uninstalled packages.'
        result['limitations']='Complete discovered main-table record selection within the fixed installation; complete bytes/pixels only for fully backed groups. Double-depth profile remains a length observation, not missing-byte reconstruction or native-path execution.'
    text=json.dumps(result,indent=2)+'\n'
    pin_name='volume-full-inventory-observations.json'if all_records else 'volume-inventory-observations.json'
    pin=json.loads((repo/'tests/fixtures'/pin_name).read_text())
    assert hashlib.sha256(text.encode()).hexdigest()==pin['report_sha256'], hashlib.sha256(text.encode()).hexdigest()
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__=='__main__':
    main()

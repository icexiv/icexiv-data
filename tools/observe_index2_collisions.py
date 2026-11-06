"""Read-only primary/index2 coverage and collision-name wire observations.

Args: game/sqpack new-output-directory. No DAT extraction or native execution.
The auxiliary layout is checked against independent full-path and split hashes,
and all packed locations are reconciled with the corresponding primary table.
"""
import collections
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib


def crc(path):
    return zlib.crc32(path)^0xffffffff


def main():
    root,out=[Path(p).resolve()for p in sys.argv[1:]]
    repo=Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent)and not out.is_relative_to(repo)
    indexes=sorted(root.glob('*/*.win32.index'))
    assert 0<len(indexes)<=128
    assert {p.with_suffix('.index2')for p in indexes}==set(root.glob('*/*.win32.index2'))
    budget=128<<20
    name_budget=16<<20
    def read_table(path,stride,auxiliary=False):
        nonlocal budget
        file_size=path.stat().st_size
        assert file_size<=128<<20
        with path.open('rb')as f:
            prefix=f.read(24)
            assert prefix[:8]==b'SqPack\0\0'
            header,version,kind=struct.unpack_from('<3I',prefix,12)
            assert 0<header<=2<<20 and version==1 and kind==2
            f.seek(header)
            h=f.read(92);assert len(h)==92
            hs,hv,offset,size=struct.unpack_from('<4I',h)
            assert 92<=hs<=2<<20 and hv==1 and offset>=header+hs
            assert header+hs<=file_size
            if auxiliary:offset,size=struct.unpack_from('<II',h,84)
            assert size<=64<<20 and size%stride==0
            budget-=size;assert budget>=0
            if size:assert offset>=header+hs
            assert offset+size<=file_size
            f.seek(offset);data=f.read(size);assert len(data)==size
            return data
    reports=[]
    for primary in indexes:
        secondary=primary.with_suffix('.index2')
        a=read_table(primary,16);b=read_table(secondary,8);names=read_table(secondary,256,True)
        primary_records={};locations=collections.Counter()
        for hash64,packed,_ in struct.iter_unpack('<QII',a):
            assert not packed&1 and hash64 not in primary_records
            primary_records[hash64]=packed;locations[packed]+=1
        direct=collections.Counter();flagged={}
        for hash32,packed in struct.iter_unpack('<II',b):
            if packed&1:
                assert hash32 not in flagged;flagged[hash32]=packed
            else:direct[packed]+=1
        entries=[];sentinels=[]
        for start in range(0,len(names),256):
            row=names[start:start+256]
            hash32,unknown,packed,sequence=struct.unpack_from('<4I',row)
            if hash32==0xffffffff and packed==0 and sequence==0xffffffff and row[16]==0:
                assert start+256==len(names) and row[16]==0
                sentinels.append({'unknown_word':unknown});continue
            assert not packed&1 and hash32 in flagged,(str(secondary),start,hash32,unknown,packed,sequence)
            end=row.find(b'\0',16);assert end>=16
            path=row[16:end];name_budget-=len(path)+1;assert name_budget>=0
            directory,filename=path.rsplit(b'/',1)
            assert path and crc(path)==hash32
            hash64=(crc(directory)<<32)|crc(filename)
            assert primary_records[hash64]==packed
            entries.append({'hash':f'{hash32:08x}','unknown_word':unknown,
                'packed':f'{packed:08x}','sequence_word':sequence,'path':path.decode('ascii'),
                'dat':(packed>>1)&7,'offset':(packed&~15)*8})
            direct[packed]+=1
        assert direct==locations
        groups=collections.Counter(int(e['hash'],16)for e in entries)
        assert set(groups)==set(flagged) and all(n==2 for n in groups.values())
        reports.append({'index':str(primary.relative_to(root)),'primary_records':len(a)//16,
            'index2_records':len(b)//8,'primary_table_sha256':hashlib.sha256(a).hexdigest(),
            'index2_table_sha256':hashlib.sha256(b).hexdigest(),'auxiliary_sha256':hashlib.sha256(names).hexdigest(),
            'flagged':[{ 'hash':f'{key:08x}','packed':f'{value:08x}'}for key,value in flagged.items()],
            'collision_records':entries,'sentinels':sentinels})
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32',
        'indexes':reports,'primary_records':sum(p['primary_records']for p in reports),
        'index2_records':sum(p['index2_records']for p in reports),
        'flagged_hashes':sum(len(p['flagged'])for p in reports),
        'collision_name_records':sum(len(p['collision_records'])for p in reports),
        'table_bytes_read':(128<<20)-budget,'index2_only_locations':0,
        'limitations':'Complete discovered paired main tables and index2 collision records in one installation. Location multiset equality after collision expansion, not DAT extraction or auxiliary-field business semantics. No orphan DAT records, missing packages, native execution or collision parsing acceptance claim.'}
    text=json.dumps(result,indent=2)+'\n'
    pin=json.loads((repo/'tests/fixtures/index2-collision-observations.json').read_text())
    assert hashlib.sha256(text.encode()).hexdigest()==pin['report_sha256'],hashlib.sha256(text.encode()).hexdigest()
    out.mkdir(parents=True,exist_ok=False);(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

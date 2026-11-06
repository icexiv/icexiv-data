"""Bounded readonly MDL declaration discovery for multiple usage-index inputs.

Args: game/sqpack new-report.json [--extended-inputs]. At most 500000 records, 100000/index,
128 indexes, 1 MiB/stack and 256 MiB cumulative decoded stack capacity. Stops
after four candidates; declaration observations are not geometry verification.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib


def main():
    root,out=[Path(p).resolve() for p in sys.argv[1:3]]
    assert len(sys.argv) in (3,4)
    extended=len(sys.argv)==4
    if extended:
        assert sys.argv[3]=='--extended-inputs'
    repo=Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    indexes=sorted([p for p in root.glob('*/*.win32.index') if p.name[:2] in ('01','02','04','08')],
        key=lambda p:(p.parent.name!='ffxiv',str(p)))
    assert len(indexes)<=128
    remaining,budget=500000,256<<20
    packs,matches=[],[]
    for index in indexes:
        if not remaining or not budget or len(matches)==4:
            break
        with index.open('rb') as f:
            f.seek(12)
            ih,=struct.unpack('<I',f.read(4))
            assert ih<=2<<20
            f.seek(ih+8)
            offset,size=struct.unpack('<II',f.read(8))
            assert size<=64<<20 and size%16==0
            f.seek(offset)
            table=f.read(size)
            assert len(table)==size
        total=size//16
        selected=min(total,100000,remaining)
        stats={'index':str(index.relative_to(root)),'records':total,'selected':selected,
            'scanned':0,'stacks':0,'oversized_stacks':0,'index_table_sha256':hashlib.sha256(table).hexdigest()}
        observed=hashlib.sha256()
        with contextlib.ExitStack() as resources:
            files={}
            for i in range(selected):
                if not budget or len(matches)==4:
                    break
                ordinal=i*total//selected
                key,packed,_=struct.unpack_from('<QII',table,ordinal*16)
                remaining-=1
                stats['scanned']+=1
                if packed&1:
                    continue
                dat=index.with_suffix(f'.dat{(packed>>1)&7}')
                if dat not in files:
                    files[dat]=resources.enter_context(dat.open('rb'))
                f=files[dat]
                entry=(packed>>4)*128
                f.seek(entry)
                prefix=f.read(24)
                assert len(prefix)==24
                observed.update(prefix)
                hs,kind,target=struct.unpack_from('<3I',prefix)
                if kind!=3:
                    continue
                assert 208<=hs<=1<<20 and target<=64<<20
                f.seek(entry)
                header=f.read(hs)
                capacity,=struct.unpack_from('<I',header,24)
                if capacity>1<<20:
                    stats['oversized_stacks']+=1
                    continue
                if capacity>budget:
                    budget=0
                    break
                budget-=capacity
                counts=struct.unpack_from('<11H',header,178)
                starts=struct.unpack_from('<11H',header,156)
                assert 208+2*sum(counts)<=hs
                spans=struct.unpack_from(f'<{sum(counts)}H',header,208)
                relative,=struct.unpack_from('<I',header,112)
                position=entry+hs+relative
                stack=bytearray()
                for block in range(starts[0],starts[0]+counts[0]):
                    f.seek(position)
                    bh,_,stored,expanded=struct.unpack('<4I',f.read(16))
                    n=expanded if stored==32000 else stored
                    assert 16<=bh<=spans[block] and n<=spans[block]-bh and expanded<=65535
                    f.seek(position+bh)
                    data=f.read(n)
                    assert len(data)==n
                    if stored!=32000:
                        d=zlib.decompressobj(-15)
                        data=d.decompress(data,65536)
                        assert d.eof and not d.unused_data and not d.unconsumed_tail
                    assert len(data)==expanded and len(stack)+expanded<=capacity
                    stack.extend(data)
                    position+=spans[block]
                assert (len(stack)+127)//128*128==capacity
                declarations,=struct.unpack_from('<H',header,200)
                assert declarations*136<=len(stack)
                stats['stacks']+=1
                observed.update(hashlib.sha256(stack).digest())
                found=[]
                for mesh in range(declarations):
                    seen={}
                    for element in range(17):
                        e=tuple(stack[mesh*136+element*8:mesh*136+element*8+5])
                        if e[0]==255:
                            break
                        seen.setdefault(e[3],set()).add(e[4])
                    qualifies = any((usage!=7 and any(index!=0 for index in values)) or usage>=8
                        for usage,values in seen.items()) if extended else any(
                            any(index!=0 for index in values) for values in seen.values())
                    if qualifies:
                        found.append({'mesh':mesh,'usage_indices':{str(k):sorted(v) for k,v in seen.items()}})
                if found:
                    matches.append({'index':str(index.relative_to(root)),'ordinal':ordinal,
                        'hash':f'{key:016x}','dat':str(dat.relative_to(root)),'offset':entry,
                        'bytes':target,'stack_sha256':hashlib.sha256(stack).hexdigest(),'meshes':found})
        stats['observations_sha256']=observed.hexdigest()
        packs.append(stats)
    result={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32',
        'packs':packs,'matches':matches,'remaining_stack_capacity_budget':budget,
        'limitations':'Bounded declaration-only observations, not complete reconstruction, scalar geometry or absence proof.'}
    if extended:
        result['filter']='Non-color usage-index > 0 or raw usage ID >= 8; no inferred shader meaning.'
    text=json.dumps(result,indent=2)+'\n'
    with out.open('x') as f:
        f.write(text)
    print(text,end='')


if __name__=='__main__':
    main()

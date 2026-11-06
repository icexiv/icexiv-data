"""Independent full PBD/EPBD observation using Python struct (no FFXIV library).

Args: extracted.pbd new-output-directory. Includes every nested matrix/name and
connection; report distinguishes raw coefficient decoding from runtime meaning.
"""
from pathlib import Path
import hashlib
import json
import math
import struct
import subprocess
import sys


def observe(data, path, rows, stats):
    core = data
    extension = None
    if data[-24:-20] == b'PACK':
        _,version,count,prior,total = struct.unpack_from('<4sHHqQ',data,len(data)-24)
        assert version == count == 1 and total <= len(data)
        start = len(data)-total
        kind,version,count,length = struct.unpack_from('<4sHHQ',data,start)
        assert kind == b'EPBD' and version == 1 and count == 0 and length == total-40 and prior == -(total-24)
        core, extension = data[:start], data[start+16:-24]
    count, = struct.unpack_from('<I',core)
    headers = list(struct.iter_unpack('<HHII',core[4:4+count*12]))
    links = list(struct.iter_unpack('<4h',core[4+count*12:4+count*20]))
    rows.append(f'pbd\t{path}\t{count}')
    stats['pbd_tables'] += 1; stats['deformers'] += count
    for i,(identity,connection,offset,scale_bits) in enumerate(headers):
        assert links[connection][3] == i
        names, matrices = [], []
        if offset:
            n, = struct.unpack_from('<I',core,offset)
            pointers = struct.unpack_from(f'<{n}H',core,offset+4)
            matrix_at = (offset+4+n*2+3)//4*4
            matrix_words = list(struct.iter_unpack('<12I',core[matrix_at:matrix_at+n*48]))
            for p,words in zip(pointers,matrix_words):
                name = core[offset+p:core.index(0,offset+p)]
                name.decode('utf-8')
                names.append(name); matrices.append(words)
        rows.append(f'deformer\t{i}\t{identity}\t{connection}\t{scale_bits:08x}\t{int(offset!=0)}\t{len(names)}')
        for j,(name,words) in enumerate(zip(names,matrices)):
            rows.append(f'bone\t{j}\t{name.hex()}\t'+','.join(f'{v:08x}' for v in words))
            floats = struct.unpack('<12f',struct.pack('<12I',*words))
            assert all(map(math.isfinite,floats))
            stats['bones'] += 1; stats['float_coefficients'] += 12
            stats['nonidentity_matrices'] += tuple(floats) != (1,0,0,0,0,1,0,0,0,0,1,0)
            stats['translated_matrices'] += any(floats[k] != 0 for k in [3,7,11])
            stats['offdiagonal_matrices'] += any(floats[k] != 0 for k in [1,2,4,6,8,9])
    for i,(parent,child,sibling,deformer) in enumerate(links):
        assert headers[deformer][1] == i
        if child >= 0: assert links[child][0] == i
        if sibling >= 0: assert links[sibling][0] == parent
        visited = set(); cursor = i
        while cursor >= 0:
            assert cursor not in visited
            visited.add(cursor); cursor = links[cursor][0]
        rows.append(f'connection\t{i}\t{parent}\t{child}\t{sibling}\t{deformer}')
    if extension is not None:
        b = extension
        assert b[4:8] == b'EPBD'
        u32 = lambda p: struct.unpack_from('<I',b,p)[0]
        def field(table,index):
            vtable = table - struct.unpack_from('<i',b,table)[0]
            length, = struct.unpack_from('<H',b,vtable)
            if index*2+4 >= length: return None
            relative, = struct.unpack_from('<H',b,vtable+4+index*2)
            return table+relative if relative else None
        def vector(table,index):
            p = field(table,index)
            if p is None: return 0,0
            v = p+u32(p); return v+4,u32(v)
        start,n = vector(u32(0),2)
        rows.append(f'epbd\t{n}');stats['extended_entries'] += n
        for i in range(n):
            at = start+i*4; table = at+u32(at)
            codes = [u32(p) if (p:=field(table,k)) is not None else 0 for k in [0,1]]
            p,c = vector(table,2);conditions = b[p:p+c*4]
            p,c = vector(table,3);inner = b[p:p+c]
            rows.append(f'extended\t{i}\t{codes[0]}\t{codes[1]}\t{conditions.hex()}\t{len(inner)}')
            observe(inner,f'{path}/{i}',rows,stats)


def main():
    source,output = Path(sys.argv[1]),Path(sys.argv[2]);output.mkdir(parents=True,exist_ok=False)
    b = source.read_bytes();assert len(b) <= 64*1024*1024
    rows=[];stats={k:0 for k in ['pbd_tables','deformers','bones','float_coefficients','nonidentity_matrices','translated_matrices','offdiagonal_matrices','extended_entries']}
    observe(b,'root',rows,stats)
    expected='\n'.join(rows)+'\n'
    rust=subprocess.check_output(['target/debug/examples/validate_pbd',str(source)],text=True)
    (output/'rust.tsv').write_text(rust);(output/'python.tsv').write_text(expected)
    assert expected==rust,'PBD metadata, names or matrix coefficients differ'
    result={'platform':'Win32','version':'2026.10.06.0000.0000','resource':'chara/xls/boneDeformer/human.pbd',
            'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),
            'python':sys.version,'statistics':stats,'comparison':'all base/embedded relations, names, scales and matrix coefficient bits equal'}
    pin=Path(__file__).resolve().parent.parent/'tests/fixtures/pbd-observations.json'
    if pin.exists():
        sample=json.loads(pin.read_text())
        assert all(result[k]==sample[k] for k in ['sha256','rows_sha256'])
        result['pinned_regression']='equal'
    else:result['pinned_regression']='unregistered sample'
    text=json.dumps(result,indent=2)+'\n';(output/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

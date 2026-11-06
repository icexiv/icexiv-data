"""Full EQP bitmap/block expansion. Args: extracted.eqp new-output-directory.

Uses sequential block traversal, not the Rust rank lookup. Absent blocks are
kept absent; no unverified runtime default is fabricated.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    source,output=Path(sys.argv[1]),Path(sys.argv[2]);output.mkdir(parents=True,exist_ok=False)
    b=source.read_bytes();bitmap,=struct.unpack_from('<Q',b)
    assert bitmap&1 and len(b)==bitmap.bit_count()*160*8
    rows=[f'eqp\t{len(b)}\t{bitmap:016x}\t{bitmap.bit_count()}'];stored=0;populations=[0]*64;present=0
    blocks=[]
    for logical in range(64):
        if not bitmap>>logical&1:
            rows.extend(f'entry\t{logical*160+i}\tabsent' for i in range(160) if logical*160+i)
            continue
        payload=list(struct.iter_unpack('<Q',b[stored*1280:(stored+1)*1280]));stored+=1;blocks.append(logical)
        for within,(value,) in enumerate(payload):
            identity=logical*160+within
            if identity==0:continue
            body,legs,hands,feet,head=struct.unpack('<HBBB3s',struct.pack('<Q',value));head=int.from_bytes(head,'little')
            rows.append(f'entry\t{identity}\t{value:016x}\t{body:04x},{legs:02x},{hands:02x},{feet:02x},{head:06x}')
            for bit in range(64):populations[bit]+=(value>>bit)&1
            present+=1
    expected='\n'.join(rows)+'\n';actual=subprocess.check_output(['target/debug/examples/validate_eqp',str(source)],text=True)
    (output/'python.tsv').write_text(expected);(output/'rust.tsv').write_text(actual);assert expected==actual,'EQP logical entries or slot groups differ'
    result={'version':'2026.10.06.0000.0000','platform':'Win32','resource':'chara/xls/equipmentparameter/equipmentparameter.eqp',
            'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),
            'bitmap':f'{bitmap:016x}','stored_blocks':blocks,'present_entries_excluding_bitmap':present,'absent_entries':10239-present,
            'bit_populations':populations,'comparison':'all 10239 logical entry states, raw bits and slot regions equal','python':sys.version}
    pin=Path(__file__).resolve().parent.parent/'tests/fixtures/eqp-observations.json'
    if pin.exists():
        sample=json.loads(pin.read_text());assert all(result[k]==sample[k] for k in ['sha256','rows_sha256'])
        result['pinned_regression']='equal'
    else:result['pinned_regression']='unregistered sample'
    text=json.dumps(result,indent=2)+'\n';(output/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

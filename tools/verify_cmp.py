"""Independent full CMP color/range observation; args: human.cmp new-output.

No FFXIV library or third-party implementation is executed. Record ordering is
observed sequentially and compared to Rust's coordinate-addressed lookup.
"""
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys


def main():
    source,output=Path(sys.argv[1]),Path(sys.argv[2]);output.mkdir(parents=True,exist_ok=False)
    b=source.read_bytes();assert len(b)==0x2d980
    # Derive the 80-record tail, independently of the Rust color-prefix constant.
    start=len(b)-80*14*4;assert start==0x2c800
    colors=list(struct.iter_unpack('4B',b[:start]));parameters=list(struct.iter_unpack('<14I',b[start:]))
    rows=[f'cmp\t{len(b)}\t{len(colors)}\t{len(parameters)}'];alpha=Counter()
    for i,color in enumerate(colors):
        rows.append(f'color\t{i}\t'+bytes(color).hex());alpha[color[3]]+=1
    for i,bits in enumerate(parameters):
        floats=struct.unpack('<14f',struct.pack('<14I',*bits));assert all(map(math.isfinite,floats))
        for low,high in [(0,1),(2,3),(4,5),(6,7),(8,11),(9,12),(10,13)]:assert floats[low]<=floats[high]
        race,within=divmod(i,10);body,clan=divmod(within,2)
        rows.append(f'parameters\t{race}\t{body+1}\t{clan}\t'+','.join(f'{v:08x}' for v in bits))
        for name,base in [('Male',0),('Female',4)]:rows.append(f'ranges\t{name}\t'+','.join(f'{v:08x}' for v in bits[base:base+4]))
        rows.append('bust\t'+','.join(f'{v:08x}' for v in bits[8:14]))
    expected='\n'.join(rows)+'\n'
    actual=subprocess.check_output(['target/debug/examples/validate_cmp',str(source)],text=True)
    (output/'rust.tsv').write_text(actual);(output/'python.tsv').write_text(expected)
    assert expected==actual,'full CMP color/range bits differ'
    result={'version':'2026.10.06.0000.0000','platform':'Win32','resource':'chara/xls/charamake/human.cmp',
            'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'rows_sha256':hashlib.sha256(expected.encode()).hexdigest(),
            'colors':len(colors),'palettes':len(colors)//256,'parameter_records':len(parameters),'float_scalars':len(parameters)*14,
            'ordered_ranges':len(parameters)*7,'alpha_histogram':dict(sorted(alpha.items())),
            'comparison':'all RGBA bytes, range scalar bits and coordinate lookups equal','python':sys.version}
    pin=Path(__file__).resolve().parent.parent/'tests/fixtures/cmp-observations.json'
    if pin.exists():
        sample=json.loads(pin.read_text());assert all(result[k]==sample[k] for k in ['sha256','rows_sha256'])
        result['pinned_regression']='equal'
    else:result['pinned_regression']='unregistered sample'
    text=json.dumps(result,indent=2)+'\n';(output/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

"""Exact-rational cofactor oracle for general affine inverses of real PBD words.

Args: extracted-human.pbd new-output-directory. Not native execution, reverse
race selection, reconstruction of native inverse bind data or skeleton assembly.
"""
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
from verify_pbd import observe


def main():
    source,out=[Path(p).resolve()for p in sys.argv[1:]]
    repo=Path(__file__).resolve().parent.parent
    assert source.stat().st_size<=64<<20 and not out.is_relative_to(repo)
    assert not out.is_relative_to(Path.home()/'Games/ffxiv')
    fixtures=repo/'tests/fixtures'
    pin=json.loads((fixtures/'pbd-observations.json').read_text())
    data=source.read_bytes();assert hashlib.sha256(data).hexdigest()==pin['sha256']
    rows=[]
    stats=dict.fromkeys(['pbd_tables','deformers','bones','float_coefficients',
        'nonidentity_matrices','translated_matrices','offdiagonal_matrices','extended_entries'],0)
    observe(data,'root',rows,stats)
    assert hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest()==pin['rows_sha256']
    expected=[];path='';deformer=bone=0;singular=0
    for row in rows:
        f=row.split('\t')
        if f[0]=='pbd':path=f[1];deformer=-1
        elif f[0]=='deformer':deformer+=1;bone=0
        elif f[0]=='bone':
            words=[int(v,16)for v in f[3].split(',')]
            values=struct.unpack('<12f',struct.pack('<12I',*words))
            assert all(map(math.isfinite,values))
            a=[[Fraction(v)for v in values[r*4:r*4+4]]for r in range(3)]
            def cofactor(r,c):
                minor=[[a[i][j]for j in range(3)if j!=c]for i in range(3)if i!=r]
                return (-1)**(r+c)*(minor[0][0]*minor[1][1]-minor[0][1]*minor[1][0])
            det=sum(a[0][c]*cofactor(0,c)for c in range(3))
            if det==0:
                expected.append((['inverse_error',path,str(deformer),str(bone)],'Singular'));singular+=1
            else:
                inv=[[cofactor(c,r)/det for c in range(3)]for r in range(3)]
                for r in range(3):inv[r].append(-sum(inv[r][c]*a[c][3]for c in range(3)))
                expected.append((['inverse',path,str(deformer),str(bone)],[float(v)for row in inv for v in row]))
            bone+=1
    actual=subprocess.check_output(['target/debug/examples/validate_pbd_inverse',str(source)],text=True,timeout=120)
    lines=actual.splitlines();assert len(lines)==len(expected)
    max_error=max_scaled=0.;components=0
    for line,(prefix,values)in zip(lines,expected):
        f=line.split('\t');assert f[:4]==prefix and len(f)==5
        if isinstance(values,str):assert f[4]==values;continue
        got=list(map(float,f[4].split(',')));assert len(got)==len(values)==12
        for a,b in zip(got,values):
            assert math.isfinite(a)and math.isfinite(b)
            error=abs(a-b);scaled=error/max(1.,abs(b));assert scaled<=2e-6,(prefix,a,b,scaled)
            max_error=max(max_error,error);max_scaled=max(max_scaled,scaled);components+=1
    oracle='\n'.join('\t'.join(p)+'\t'+(v if isinstance(v,str)else ','.join(format(x,'.17e')for x in v))for p,v in expected)+'\n'
    result={'resource_sha256':pin['sha256'],'matrices':len(expected),'singular':singular,
        'compared_components':components,'scaled_tolerance':2e-6,
        'max_absolute_error':max_error,'max_scaled_error':max_scaled,
        'oracle_rows_sha256':hashlib.sha256(oracle.encode()).hexdigest(),
        'limitations':'Exact-rational general affine math on all observed coefficient words, not a reverse race route, native reference-pose output or final deformation-space oracle.'}
    text=json.dumps(result,indent=2)+'\n'
    extra=fixtures/'pbd-inverse-observations.json'
    assert hashlib.sha256(text.encode()).hexdigest()==json.loads(extra.read_text())['report_sha256']
    out.mkdir(parents=True,exist_ok=False)
    (out/'rust.tsv').write_text(actual);(out/'python.tsv').write_text(oracle);(out/'report.json').write_text(text)
    print(text,end='')


if __name__=='__main__':main()

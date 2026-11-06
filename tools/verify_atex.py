"""Actual AVFX-referenced ATEXs: type-2 bytes and every mip pixel vs Pillow.

Args: game/sqpack new-output-directory. See TEX provenance for Pillow license.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from find_vfx_samples import standard
from verify_tex import locate,observe


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve())
    manifest=json.loads((Path(__file__).resolve().parent.parent/'tests/fixtures/atex-observations.json').read_text())
    version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version'];out.mkdir(parents=True,exist_ok=False)
    paths=[];sources=[]
    for i,source in enumerate(manifest['samples']):
        path=out/f'{i}.atex';log=subprocess.check_output(['target/debug/examples/local_sqpack',str(root),source['path'],str(path)],text=True);(out/f'{i}.extraction.txt').write_text(log)
        dat,entry=locate(root,source)
        with dat.open('rb') as f:raw=standard(f,entry)
        assert raw==path.read_bytes();paths.append(path);sources.append((source,dat,entry))
    pixels=out/'pixels';rust=subprocess.check_output(['target/debug/examples/validate_tex',str(pixels),*map(str,paths)],text=True);(out/'rust.tsv').write_text(rust)
    rows=[];reports=[]
    for i,(path,(source,dat,entry)) in enumerate(zip(paths,sources)):
        raw=path.read_bytes();r,report=observe(raw,i,pixels);rows.extend(r)
        report.update({'path':source['path'],'dat':str(dat.relative_to(root)),'offset':entry,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'rows_sha256':hashlib.sha256(('\n'.join(r)+'\n').encode()).hexdigest()})
        if 'sha256'in source:assert all(source[k]==report[k]for k in ['sha256','rows_sha256'])
        reports.append(report)
    assert rust=='\n'.join(rows)+'\n'
    import PIL
    text=json.dumps({'version':version,'platform':'Win32','pillow':PIL.__version__,'samples':reports,'comparison':'all type-2 extracted bytes and every mip pixel equal'},indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

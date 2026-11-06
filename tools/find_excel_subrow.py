"""Bounded readonly EXH inventory for real subrow/string evidence.

Args: game/sqpack new-output-dir. Examine at most 5000 root.exl sheet headers,
stop after two variant-2 sheets with string columns. Independent type-2 bytes.
"""
import contextlib
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from find_vfx_samples import standard


class ExcelInstallation:
    def __init__(self,root):
        self.root=root;self.entries={}
        for index in sorted((root/'ffxiv').glob('0a00*.win32.index')):
            with index.open('rb')as f:
                f.seek(12);hs,=struct.unpack('<I',f.read(4));f.seek(hs+8);off,size=struct.unpack('<II',f.read(8));assert size<=64<<20;f.seek(off)
                for key,packed,_ in struct.iter_unpack('<QII',f.read(size)):
                    if packed&1:continue
                    self.entries[key]=(index.with_suffix(f'.dat{(packed>>1)&7}'),(packed>>4)*128)
    def locate(self,path):
        directory,filename=path.lower().rsplit('/',1);jam=lambda s:~zlib.crc32(s.encode('ascii'))&0xffffffff
        return self.entries[jam(directory)<<32|jam(filename)]
    def read(self,path):
        dat,entry=self.locate(path)
        with dat.open('rb')as f:raw=standard(f,entry)
        assert raw is not None
        return raw


def main():
    root,out=map(Path,sys.argv[1:]);assert not out.resolve().is_relative_to(root.parent.resolve());out.mkdir(parents=True,exist_ok=False)
    install=ExcelInstallation(root);root_exl=install.read('exd/root.exl');assert root_exl.startswith(b'EXLT,')
    findings=[];seen=0
    for row in root_exl.decode('utf-8-sig').splitlines()[1:5001]:
        name=row.split(',')[0]
        if not name or name=='#':continue
        try:raw=install.read(f'exd/{name.lower()}.exh')
        except KeyError:continue # only confirmed index absence
        seen+=1;assert raw[:4]==b'EXHF';cols,pages,langs=struct.unpack_from('>3H',raw,8)
        if raw[17]!=2 or not any(t==0 for t,_ in struct.iter_unpack('>HH',raw[32:32+cols*4])):continue
        path=f'exd/{name.lower()}.exh';(out/f'{name}.exh').write_bytes(raw)
        page_records=list(struct.iter_unpack('>II',raw[32+cols*4:32+cols*4+pages*8]));lang=raw[32+cols*4+pages*8:]
        findings.append({'sheet':name,'path':path,'sha256':hashlib.sha256(raw).hexdigest(),'pages':page_records,'language_bytes':lang.hex()})
        if len(findings)>=2:break
    report={'version':(root.parent/'ffxivgame.ver').read_text().strip(),'platform':'Win32','headers_examined':seen,'header_limit':5000,'samples':findings}
    text=json.dumps(report,indent=2)+'\n';(out/'report.json').write_text(text);print(text,end='')


if __name__=='__main__':main()

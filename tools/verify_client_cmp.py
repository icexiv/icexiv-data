"""Pinned CMP field/interpolation static observations, never target execution.

Args: authorized-ffxiv_dx11.exe new-output-directory. GNU objdump output stays
outside the repository. No native customization or pose algorithm is ported.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    exe, out = map(Path, sys.argv[1:])
    assert exe.stat().st_size <= 128 << 20 and not out.resolve().is_relative_to(exe.parent.resolve())
    out.mkdir(parents=True, exist_ok=False)
    pin = json.loads((Path(__file__).resolve().parent.parent / 'tests/fixtures/client-cmp-observations.json').read_text())
    b = exe.read_bytes()
    assert hashlib.sha256(b).hexdigest() == pin['binary_sha256']
    pe, = struct.unpack_from('<I', b, 60)
    assert b[:2] == b'MZ' and b[pe:pe+4] == b'PE\0\0'
    machine, count, _, _, _, size, _ = struct.unpack_from('<HHIIIHH', b, pe+4)
    assert machine == 0x8664
    opt = pe+24
    image, = struct.unpack_from('<Q', b, opt+24)
    assert image == 0x140000000 and struct.unpack_from('<H', b, opt)[0] == 0x20b
    sections = [struct.unpack_from('<4I', b, opt+size+i*40+8) for i in range(count)]
    def raw(rva, length):
        for _, at, size, offset in sections:
            if at <= rva and rva+length <= at+size:
                assert offset+size <= len(b)
                return b[offset+rva-at:offset+rva-at+length]
        raise ValueError('unbacked RVA')
    for span in pin['ranges']:
        start = int(span['rva'], 0)
        assert span['bytes'] <= 65536 and hashlib.sha256(raw(start, span['bytes'])).hexdigest() == span['sha256']
        asm = subprocess.check_output(['objdump','-d','-Mintel',f'--start-address={image+start:#x}',f'--stop-address={image+start+span["bytes"]:#x}',str(exe)], text=True, timeout=30)
        (out / (span['label']+'.asm')).write_text(asm)
    for a in pin['anchors']:
        expected = bytes.fromhex(a['hex'])
        assert raw(int(a['rva'], 0), len(expected)) == expected
    for rva, target in [(0x46d555,0x211bcd8),(0x46d684,0x211bcd8),(0x46d221,0x211bcd8)]:
        ins = raw(rva, 8)
        assert ins[:3] == bytes.fromhex('f30f5e') and ins[3] & 0xc7 == 5
        assert rva+8+struct.unpack_from('<i', ins, 4)[0] == target
    for rva, target in [(0x44601c,0x46d180),(0x44607d,0x46d490)]:
        ins = raw(rva, 5)
        assert ins[0] == 0xe8 and rva+5+struct.unpack_from('<i', ins, 1)[0] == target
    ins = raw(0x426898, 7)
    assert ins[:3] == bytes.fromhex('4c8d05')
    path = b'chara/xls/charaMake/human.cmp\0'
    assert raw(0x426898+7+struct.unpack_from('<i', ins, 3)[0],len(path)) == path
    assert 0xcb7*56 == 0x2c808
    result = {'binary_sha256': pin['binary_sha256'], 'static_spans':len(pin['ranges']),
        'static_anchors':len(pin['anchors']), 'objdump':subprocess.check_output(['objdump','--version'],text=True).splitlines()[0],
        'limitations':'Static field/range and percentage-consumer observations, not runtime or REA Evidence. No full bone-space, NPC policy or pose application claim.'}
    text = json.dumps(result,indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__ == '__main__':
    main()

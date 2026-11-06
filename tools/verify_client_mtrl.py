"""MTRL native declarations/Legacy dye fields and full-u16 controlled evidence.

Args: authorized-ffxiv_dx11.exe extracted-Legacy-no-dye.mtrl new-output-dir.
No target execution/emulation or native conversion/dye application is ported.
Controlled files retain actual color rows, not actual game dye provenance.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
from verify_materials import observe_mtrl


def main():
    exe, source, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(exe.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    pin = json.loads((repo/'tests/fixtures/client-mtrl-observations.json').read_text())
    assert exe.stat().st_size <= 128 << 20 and source.stat().st_size <= 65535
    binary = exe.read_bytes()
    assert hashlib.sha256(binary).hexdigest() == pin['binary_sha256']
    pe, = struct.unpack_from('<I', binary, 60)
    assert binary[:2] == b'MZ' and binary[pe:pe+4] == b'PE\0\0'
    machine, count, _, _, _, size, _ = struct.unpack_from('<HHIIIHH', binary, pe+4)
    opt = pe+24
    assert machine == 0x8664 and count <= 96
    assert struct.unpack_from('<Q', binary, opt+24)[0] == 0x140000000
    sections = [struct.unpack_from('<4I', binary, opt+size+i*40+8) for i in range(count)]
    def raw(rva, length):
        for _, at, size, offset in sections:
            if at <= rva and rva+length <= at+size:
                assert offset+size <= len(binary)
                return binary[offset+rva-at:offset+rva-at+length]
        raise ValueError('unbacked RVA')
    for span in pin['ranges']:
        start = int(span['rva'], 0)
        assert span['bytes'] <= 65536 and hashlib.sha256(raw(start, span['bytes'])).hexdigest() == span['sha256']
        asm = subprocess.check_output(['objdump','-d','-Mintel',f'--start-address={0x140000000+start:#x}',
            f'--stop-address={0x140000000+start+span["bytes"]:#x}',str(exe)],text=True,timeout=30)
        (out/(span['label']+'.asm')).write_text(asm)
    for anchor in pin['anchors']:
        value = bytes.fromhex(anchor['hex'])
        assert raw(int(anchor['rva'], 0), len(value)) == value
    dye = raw(0x42b1bf, 620)
    for offset in range(2, 32, 2):
        assert bytes.fromhex('410fb75424')+bytes([offset]) in dye
    legacy_table = list(struct.iter_unpack('<II', raw(0x28aaa40, 40)))
    modern_table = list(struct.iter_unpack('<II', raw(0x28aa9e0, 96)))
    assert legacy_table == [(0,0),(1,4),(2,8),(3,3),(4,7)]
    assert modern_table == [(0,0),(1,4),(2,8),(3,11),(4,18),(5,16),(6,12),(7,13),(8,14),(9,19),(10,27),(11,21)]
    width_ref=raw(0x1883d58,7)
    assert width_ref[:3]==bytes.fromhex('488d2d') and 0x1883d5f+struct.unpack_from('<i',width_ref,3)[0]==0x22dcfd0
    assert struct.unpack('<2Q',raw(0x22dcfd0,16))==(3,1)
    shader_ref=raw(0x426c63,7)
    assert shader_ref[:3]==bytes.fromhex('4c8d05')
    shader_rva=0x426c6a+struct.unpack_from('<i',shader_ref,3)[0]
    shader_name=b'shader/sm5/shpk/characterlegacy.shpk\0'
    assert raw(shader_rva,len(shader_name))==shader_name
    for at,opcode,target in[(0x42b80f,'488d1d',0x28aaa44),(0x42b81b,'488d0d',0x28aa9e4)]:
        ins=raw(at,7);assert ins[:3]==bytes.fromhex(opcode) and at+7+struct.unpack_from('<i',ins,3)[0]==target
    for source_rva,target in[(0x42b891,0x1883d40),(0x42b949,0x1883d40)]:
        call=raw(source_rva,5);assert call[0]==0xe8 and source_rva+5+struct.unpack_from('<i',call,1)[0]==target
    data = bytearray(source.read_bytes())
    assert hashlib.sha256(data).hexdigest() == pin['source_mtrl_sha256']
    assert struct.unpack_from('<H', data, 6)[0] == 512
    strings, = struct.unpack_from('<H', data, 8)
    additional = 16+4*sum(data[12:15])+strings
    dataset = additional+data[15]
    assert data[15] >= 4 and dataset+512 <= len(data)
    # One controlled material, then every u16 value in pages of sixteen rows.
    data[dataset+512:dataset+512] = bytes(32)
    data[additional] |= 8
    struct.pack_into('<HH', data, 4, len(data), 544)
    controlled = out/'controlled-legacy-dye.mtrl'
    controlled.write_bytes(data)
    rows, _, _ = observe_mtrl(data)
    expected = '\n'.join(rows)+'\n'
    actual = subprocess.check_output(['target/debug/examples/validate_materials', str(controlled)],text=True,timeout=120)
    assert actual == expected
    (out/'controlled.python.tsv').write_text(expected)
    (out/'controlled.rust.tsv').write_text(actual)
    expected = ''.join(f'dye\t{word}\t{word%32}\t{word//32}\tnone\t0\n' for word in range(65536))
    actual = subprocess.check_output(['target/debug/examples/validate_legacy_dye',str(controlled)],text=True,timeout=120)
    assert actual == expected
    (out/'words.python.tsv').write_text(expected)
    (out/'words.rust.tsv').write_text(actual)
    result = {'binary_sha256': pin['binary_sha256'], 'source_mtrl_sha256': pin['source_mtrl_sha256'],
        'controlled_mtrl_sha256': hashlib.sha256(data).hexdigest(), 'words_sha256': hashlib.sha256(expected.encode()).hexdigest(),
        'controlled_rows_sha256': hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest(),
        'static_spans': len(pin['ranges']), 'legacy_words': 65536,
        'legacy_override_lanes': legacy_table, 'modern_override_lanes': modern_table,
        'limitations': 'Native offline static observations plus controlled dye words on actual color rows. No actual 544-byte-dataset game material, native execution, native color conversion or complete shader/dye application claim.'}
    if 'controlled_mtrl_sha256' in pin:
        assert all(result[k] == pin[k] for k in ['controlled_mtrl_sha256','words_sha256','controlled_rows_sha256'])
    text = json.dumps(result,indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text,end='')


if __name__ == '__main__':
    main()

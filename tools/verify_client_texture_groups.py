"""Pinned native texture-group dispatch/stride facts, not volume reconstruction.

Args: authorized-ffxiv_dx11.exe new-output-directory. Offline GNU objdump only;
no target execution or instruction emulation. Does not invent missing bytes.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys


def main():
    exe, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert exe.stat().st_size <= 128 << 20
    assert not out.is_relative_to(exe.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    pin = json.loads((repo/'tests/fixtures/client-texture-group-observations.json').read_text())
    b = exe.read_bytes()
    assert hashlib.sha256(b).hexdigest() == pin['binary_sha256']
    pe, = struct.unpack_from('<I', b, 60)
    assert b[:2] == b'MZ' and b[pe:pe+4] == b'PE\0\0'
    machine, count, _, _, _, size, _ = struct.unpack_from('<HHIIIHH', b, pe+4)
    opt = pe+24
    assert machine == 0x8664 and count <= 96
    assert struct.unpack_from('<H', b, opt)[0] == 0x20b
    image, = struct.unpack_from('<Q', b, opt+24)
    assert image == 0x140000000
    sections = [struct.unpack_from('<4I', b, opt+size+i*40+8) for i in range(count)]

    def raw(rva, length):
        for _, at, size, offset in sections:
            if at <= rva and rva+length <= at+size:
                assert offset+size <= len(b)
                return b[offset+rva-at:offset+rva-at+length]
        raise ValueError('unbacked RVA')

    for span in pin['ranges']:
        start = int(span['rva'], 0)
        assert span['bytes'] <= 65536
        assert hashlib.sha256(raw(start, span['bytes'])).hexdigest() == span['sha256']
        asm = subprocess.check_output(['objdump', '-d', '-Mintel',
            f'--start-address={image+start:#x}', f'--stop-address={image+start+span["bytes"]:#x}', str(exe)], text=True, timeout=30)
        (out/(span['label']+'.asm')).write_text(asm)
    for anchor in pin['anchors']:
        value = bytes.fromhex(anchor['hex'])
        assert raw(int(anchor['rva'], 0), len(value)) == value
    calls = [(0x30394f, 0x304510), (0x30458a, 0x479600),
        (0x479640, 0x5e6df0), (0x303970, 0x219d30),
        (0xcfbdf, 0x338260), (0x336b5a, 0x336420), (0x336c4d, 0x336420),
        (0x30485d, 0x216dc0), (0x304af6, 0x216dc0), (0x216dc9, 0x218390),
        (0x2185a1, 0x219d30), (0x218772, 0x219d30)]
    for source, target in calls:
        ins = raw(source, 5)
        assert ins[0] == 0xe8 and source+5+struct.unpack_from('<i', ins, 1)[0] == target
    # Actual descriptor lookup, not a direct call to the callback.
    ins = raw(0x5e6e1a, 7)
    assert ins[:3] == bytes.fromhex('488d15')
    table = 0x5e6e21+struct.unpack_from('<i', ins, 3)[0]
    assert table == 0x28ac090
    descriptor, = struct.unpack('<Q', raw(table+6*8, 8))
    assert descriptor == image+0x28cb8c0
    callback, = struct.unpack('<Q', raw(descriptor-image, 8))
    assert callback == image+0x1d0cb30
    # Constructor-installed allocator vtables, not an assertion about all live
    # heap replacements or an execution of the allocation path.
    vtables = [(0x2154878, 0x3377f0), (0x21547d8, 0x3358c0),
        (0x2154828, 0x336ae0)]
    for slot, target in vtables:
        assert struct.unpack('<Q', raw(slot, 8))[0] == image+target
    result = {'binary_sha256': pin['binary_sha256'], 'static_spans': len(pin['ranges']),
        'static_anchors': len(pin['anchors']), 'direct_calls': len(calls),
        'descriptor_index': 6, 'descriptor_rva': hex(descriptor-image), 'callback_rva': hex(callback-image),
        'allocator_vtable_slots': [{'slot_rva': hex(slot), 'target_rva': hex(target)} for slot, target in vtables],
        'allocation_observation': 'Constructor-installed heap chain reaches existing-region suballocation with metadata writes but no payload initialization in 0x140336420..0x1403366fc. This does not determine live heap state or gap bytes.',
        'initialization_observation': 'Conditional staging initialization reaches 0x140218390. For BGRA8, mip descriptors borrow source bytes and advance by slice size times clamped mip depth. Volume creation receives full depth/mip count and those descriptors via interface slot +0x30. Stack descriptor clearing does not establish zeroed pixels; indirect target and actual resource path selection are not executed.',
        'objdump': subprocess.check_output(['objdump', '--version'], text=True).splitlines()[0],
        'limitations': 'Offline static dispatch, destination-stride and allocator-path observations, not runtime or REA Evidence. The existing-region path does not establish zero-filled payload. Actual path selection, missing byte values and complete volume reconstruction remain unresolved.'}
    text = json.dumps(result, indent=2)+'\n'
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

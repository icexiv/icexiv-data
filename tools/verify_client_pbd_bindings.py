"""Pinned offline SKLB-handle binding and PBD input-cache evidence.

Args: authorized-ffxiv_dx11.exe new-output-directory. Checks static identity,
field/call sites and resource factory metadata; never runs/emulates target code.
No native implementation or state machine is copied into the library.
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
    assert not out.is_relative_to(exe.parent) and not out.is_relative_to(repo)
    assert exe.stat().st_size <= 128 << 20
    b = exe.read_bytes()
    pin = json.loads((repo/'tests/fixtures/client-pbd-binding-observations.json').read_text())
    assert hashlib.sha256(b).hexdigest() == pin['binary_sha256']
    pe, = struct.unpack_from('<I', b, 60)
    assert b[:2] == b'MZ' and b[pe:pe+4] == b'PE\0\0'
    machine, count, _, _, _, size, _ = struct.unpack_from('<HHIIIHH', b, pe+4)
    opt = pe + 24
    assert machine == 0x8664 and count <= 96
    image, = struct.unpack_from('<Q', b, opt+24)
    assert image == int(pin['image_base'], 0)
    sections = [struct.unpack_from('<4I', b, opt+size+40*i+8) for i in range(count)]

    def raw(rva, length):
        for _, start, size, offset in sections:
            if start <= rva and rva+length <= start+size:
                assert offset+size <= len(b)
                return b[offset+rva-start:offset+rva-start+length]
        raise ValueError('unbacked RVA')

    out.mkdir(parents=True, exist_ok=False)
    for span in pin['ranges']:
        start = int(span['rva'], 0)
        assert span['bytes'] <= 65536
        assert hashlib.sha256(raw(start, span['bytes'])).hexdigest() == span['sha256']
        text = subprocess.check_output(['objdump', '-d', '-Mintel',
            f'--start-address={image+start:#x}',
            f'--stop-address={image+start+span["bytes"]:#x}', str(exe)], text=True, timeout=30)
        (out/(span['label']+'.asm')).write_text(text)
    # These are identified direct edges, not an exhaustive list of aliases or
    # indirect invocations. Relative branch arithmetic is independently checked.
    calls = [(site, 0x259110) for site in (0x43aa6e, 0x43bf3b, 0x43c21b, 0x4521aa, 0x4547f3, 0x454ad8)]
    calls += [(0x4399d0, 0x430970), (0x430a28, 0x2fe9b0), (0x45466c, 0x2fe9b0),
        (0x2eed0c, 0x20b830), (0x2eef05, 0x20b830), (0x2681a2, 0x2eec10),
        (0x2eecdc, 0x33a7d0), (0x2eee81, 0x33a7d0), (0x2eeece, 0x33a7d0),
        (0x313b93, 0x178c0c0)]
    calls += [(0x26802e, 0x2f5c70), (0x26805f, 0x2f6490), (0x2f64b2, 0x2f5610),
        (0x2f6548, 0x42d620), (0x2f6574, 0x2fcd40), (0x2f6640, 0x2f1ba0),
        (0x2f1cf1, 0x1efe510), (0x2f1cfb, 0x1efd6b0), (0x2f1d05, 0x1efd960),
        (0x2f299f, 0x2f8a20), (0x2f8a95, 0x33b4c0)]
    for at, target in calls:
        ins = raw(at, 5)
        assert ins[0] == 0xe8 and at+5+struct.unpack_from('<i', ins, 1)[0] == target
    # Match resource request tags and the registered factory, not just a class
    # name found somewhere in the image.
    assert raw(0x4399c3, 6) == bytes.fromhex('41b8626c6b73')
    assert raw(0x454646, 8) == bytes.fromhex('c7442434626c6b73')
    assert raw(0x31ba6e, 7) == bytes.fromhex('c74518626c6b73')
    for at, prefix, target in [(0x31ba61, '488d0d', 0x214e868),
            (0x313dff, '488d05', 0x214e700), (0x313750, '488d05', 0x214c8c8)]:
        ins = raw(at, 7)
        assert ins[:3] == bytes.fromhex(prefix)
        assert at+7+struct.unpack_from('<i', ins, 3)[0] == target
    for at, target in [(0x214e870, 0x313da0), (0x214e780, 0x313750), (0x214e808, 0x3138a0)]:
        assert struct.unpack('<Q', raw(at, 8))[0] == image+target
    name = b'Client.System.Resource.Handle.SkeletonResourceHandle\0'
    assert raw(0x214c8c8, len(name)) == name
    anchors = [(0x258083, '48894758'), (0x25915b, '48893ce8'),
        (0x430a2d, '488903'), (0x454687, '48898800010000'),
        (0x25aadf, '480382f8000000'), (0x2eed00, '48896c2420'),
        (0x20b985, '488b8708020000'), (0x20b98c, '0f110402')]
    for at, expected in anchors:
        assert raw(at, len(bytes.fromhex(expected))) == bytes.fromhex(expected)
    for at, name, offset in [(0x25403b8, b'parentIndices', 0x18), (0x2540408, b'referencePose', 0x38)]:
        target = struct.unpack('<Q', raw(at, 8))[0] - image
        assert raw(target, len(name)+1) == name+b'\0'
        assert struct.unpack('<H', raw(at+30, 2))[0] == offset
    assert raw(0x1efe51a, 4) == bytes.fromhex('488b5038')
    assert raw(0x2f6544, 4) == bytes.fromhex('0fb7531a')
    result = {'version': pin['version'], 'platform': pin['platform'],
        'binary_sha256': pin['binary_sha256'], 'static_ranges': len(pin['ranges']),
        'direct_edges_checked': len(calls), 'resource_tag': '0x736b6c62',
        'factory_pointer_rva': '0x214e868', 'resource_vtable_rva': '0x214e700',
        'resource_type_name': name[:-1].decode(), 'field_anchors_checked': len(anchors),
        'part_setter_direct_calls': 6, 'nonzero_setter_paths': 3,
        'cache_writer_direct_calls': 2,
        'context_reference_copy_checked': True, 'reflection_fields_checked': 2,
        'objdump': subprocess.check_output(['objdump', '--version'], text=True).splitlines()[0],
        'limitations': 'Static instructions, metadata and direct-edge identities only; not execution or REA Evidence. Selected SKLB request-to-part fallback bindings, referencePose context initialization and two cache-writer inputs are traced, without exhaustive indirect writer coverage, native selection/pose emulation or full PBD source/target/bind-space acceptance.'}
    text = json.dumps(result, indent=2)+'\n'
    digest = hashlib.sha256(text.encode()).hexdigest()
    assert digest == pin['report_sha256'], digest
    (out/'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__': main()

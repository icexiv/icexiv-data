"""Check 40 extracted <ActionTimeline row>.tmb files using an independent graph.

Args: ActionTimeline_0.exd directory-of-extracted-TMBs inspect_motion
"""
import hashlib
from pathlib import Path
import struct
import subprocess
import sys


def keys(exd):
    if hashlib.sha256(exd).hexdigest() != 'f5cc45c6154bbb237e9e5d7bd7b98d20298d7526cb038758b7a30d2d62a0bc2b':
        raise ValueError("unexpected ActionTimeline page")
    seen = set()
    for row, offset in struct.iter_unpack('>II', exd[32:32+struct.unpack_from('>I', exd, 8)[0]]):
        base = offset + 6
        start = base + 24 + struct.unpack_from('>I', exd, base)[0]
        key = exd[start:exd.index(0, start)]
        group = key.rsplit(b'/', 1)[0]
        if key.startswith((b'magic/', b'ability/')) and group not in seen:
            seen.add(group)
            yield row, key.decode('ascii')


def events(b):
    magic, size, n = struct.unpack_from('<4sII', b)
    assert magic == b'TMLB' and size == len(b)
    roots, children, resources, conditions = [], {}, {}, []
    p = 12
    for _ in range(n):
        tag, size = struct.unpack_from('<4sI', b, p)
        s = p + 8
        ident, = struct.unpack_from('<H', b, s)
        if tag in (b'TMAL', b'TMAC', b'TMTR'):
            at = {b'TMAL': 0, b'TMAC': 12, b'TMTR': 4}[tag]
            distance, count = struct.unpack_from('<II', b, s + at)
            ids = struct.unpack_from(f'<{count}H', b, s + distance)
            if tag == b'TMAL':
                roots.extend(ids)
            else:
                children[ident] = ids
            if tag == b'TMTR' and (rel := struct.unpack_from('<I', b, s+12)[0]):
                header, ncond = struct.unpack_from('<II', b, s+rel)
                assert header == 8
                time, = struct.unpack_from('<h', b, s+2)
                for op, value, bits in struct.iter_unpack('<III', b[s+rel+8:s+rel+8+12*ncond]):
                    conditions.append(f'condition\t{ident}\t{time}\t{op}\t{value}\t{bits:08x}')
        refs = {b'C002': (16, 'Timeline'), b'C009': (12, 'Animation'), b'C010': (24, 'Animation'),
                b'C012': (12, 'Effect'), b'C063': (12, 'Sound')}
        if tag in refs:
            at, kind = refs[tag]
            start, duration = struct.unpack_from('<hi', b, s+2)
            address = s + struct.unpack_from('<I', b, s+at)[0]
            path = b[address:b.index(0, address)].decode()
            resources[ident] = '\t'.join(map(str, (p, tag.decode(), ident, start, duration, kind, path)))
        p += size
    return conditions + [resources[c] for a in roots for t in children[a] for c in children[t] if c in resources]


def main(page, directory, exe):
    manifest = hashlib.sha256()
    total = effects = 0
    for row, key in keys(Path(page).read_bytes()):
        path = Path(directory) / f'{row}.tmb'
        data = path.read_bytes()
        manifest.update(f'{row}\t{key}\t{hashlib.sha256(data).hexdigest()}\n'.encode())
        expected = events(data)
        output = subprocess.check_output([exe, path], text=True).splitlines()
        assert expected == [s for s in output if not s.startswith('effect\t')], key
        effects += sum(s.startswith('effect\t') for s in output)
        total += sum(not s.startswith('condition\t') for s in expected)
    assert manifest.hexdigest() == '261f23438a2e209e948908745cbee9a879fd4c89c5bede0b46077b3609a8bafd'
    print(f'{total} resource references matched ({effects} reachable C012)')


if __name__ == '__main__':
    main(*sys.argv[1:])

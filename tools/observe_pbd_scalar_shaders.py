"""Observe pinned PBD scalar shader consumers; never evaluates a shader.

Args: extracted-seven-SHPK-directory reviewed-token-header new-output-directory.
The Microsoft declarative header is external, SHA-pinned and reviewed under
University of Illinois/NCSA terms; see docs/provenance/pbd-scalar-consumer.md.
RDEF is the observed SM5 profile, not a general DXBC parser or library API.
"""
from pathlib import Path
import collections
import hashlib
import json
import re
import struct
import sys
import zlib


NAME = 'g_ModelParameter'
INPUT_DIGESTS = {
    'bg.shpk': '9617282ff8a947ceecd11846a73fc1d5ec7782cf19fe22ec363cc6d3195375be',
    'bgcrestchange.shpk': 'fa60e4f6a706c5ee0ff9190d9740248d8fc1802df0bfcb572049a6af5e6ad3d6',
    'character.shpk': 'c2453552ce2527f1f92a9f62d04d2c5fbed67226306629826edc0c1d9c646abe',
    'characterlegacy.shpk': '5f95ae36f1cfbf603f85817d6872190a56ddd323fb44ade17779bbaf3a28049e',
    'hair.shpk': 'ed0c9953e6aa6db7822a205e051c0628e2ad31bd87fb38e6b28ff042570a015f',
    'skin.shpk': '4e2f77df0399de59af5e6383358cb7d071ca0c7734f2e161504b086239593e57',
    'water.shpk': '40b749201ceea5332d60500ea37df61bae43372f8db55f23982ac5c5dbfe69fd',
}


def words(b, p, n):
    assert 0 <= p <= len(b) - 4 * n
    return struct.unpack_from('<' + 'I' * n, b, p)


def text(b, p):
    assert 0 <= p < len(b)
    end = b.index(0, p)
    return b[p:end].decode('ascii')


def digest(b):
    return hashlib.sha256(b).hexdigest()


def rdef(b):
    nc, cp, nr, rp = words(b, 0, 4)
    assert b[28:32] == b'RD11' and words(b, 32, 7)[:6] == (60, 24, 32, 40, 36, 12)
    assert nc < 256 and nr < 256

    def ty(p, depth=0):
        assert depth < 6 and 0 <= p <= len(b) - 36
        cls, typ, rows, cols, elems, nm = struct.unpack_from('<6H', b, p)
        mp, = words(b, p + 12, 1)
        namep, = words(b, p + 32, 1)
        assert nm < 256
        members = []
        for i in range(nm):
            np, tp, off = words(b, mp + i * 12, 3)
            members.append({'name': text(b, np), 'offset': off, 'type': ty(tp, depth + 1)})
        return {'at': hex(p), 'class': cls, 'type': typ, 'rows': rows, 'columns': cols,
                'elements': elems, 'name': text(b, namep) if namep else None, 'members': members}

    cbs = []
    for i in range(nc):
        at = cp + i * 24
        np, nv, vp, sz, flags, kind = words(b, at, 6)
        assert nv < 256
        vs = []
        for j in range(nv):
            vat = vp + j * 40
            row = words(b, vat, 10)
            vs.append({'at': hex(vat), 'name': text(b, row[0]), 'offset': row[1], 'size': row[2],
                       'flags': row[3], 'words': list(row), 'type': ty(row[4])})
        cbs.append({'at': hex(at), 'name': text(b, np), 'size': sz, 'flags': flags, 'kind': kind, 'variables': vs})
    resources = []
    for i in range(nr):
        at = rp + i * 32
        row = words(b, at, 8)
        resources.append({'at': hex(at), 'name': text(b, row[0]), 'kind': row[1],
                          'bind_point': row[5], 'bind_count': row[6], 'words': list(row)})
    return {'constant_buffers': cbs, 'resources': resources}


def operand(w, p, end, depth=0):
    assert depth < 8 and p < end
    start = p
    t = w[p]
    p += 1
    nc = t & 3
    assert nc != 3
    mode = (t >> 2) & 3
    typ = (t >> 12) & 255
    nd = (t >> 20) & 3
    extensions = []
    ext = t >> 31
    while ext:
        assert p < end and len(extensions) < 4
        e = w[p]
        extensions.append(e)
        p += 1
        ext = e >> 31
    idx = []
    for d in range(nd):
        representation = (t >> (22 + 3 * d)) & 7
        value = None
        relative = None
        if representation in (0, 3):
            assert p < end
            value = w[p]
            p += 1
        elif representation in (1, 4):
            assert p + 2 <= end
            value = [w[p], w[p + 1]]
            p += 2
        if representation in (2, 3, 4):
            relative, p = operand(w, p, end, depth + 1)
        assert representation <= 4
        idx.append({'representation': representation, 'value': value, 'relative': relative})
    immediate = []
    if typ in (4, 5):
        count = {0: 0, 1: 1, 2: 4}[nc] * (2 if typ == 5 else 1)
        assert p + count <= end
        immediate = list(w[p:p + count])
        p += count
    if nc == 2:
        assert mode in (0, 1, 2)
        component = ''.join('xyzw'[i] for i in range(4) if t & (1 << (4 + i))) if mode == 0 else (
            ''.join('xyzw'[(t >> (4 + i * 2)) & 3] for i in range(4)) if mode == 1 else 'xyzw'[(t >> 4) & 3])
    else:
        component = ''
    return {'token_word': start, 'token': hex(t), 'type': typ, 'indices': idx,
            'component': component, 'selection_mode': mode, 'extensions': extensions,
            'immediate_bits': immediate}, p


def operand_text(o):
    typ = o['type']
    if typ in (4, 5):
        vals = o['immediate_bits']
        if typ == 4:
            s = 'l(' + ','.join(f'{struct.unpack("<f", struct.pack("<I", v))[0]:.9g}/0x{v:08x}' for v in vals) + ')'
        else:
            s = f'imm64({vals})'
    else:
        s = {0: 'r', 1: 'v', 2: 'o', 3: 'x', 6: 's', 7: 't', 8: 'cb', 9: 'icb', 13: 'null', 30: 'u'}.get(typ, f'type{typ}')
        for i, ind in enumerate(o['indices']):
            val = '' if ind['value'] is None else str(ind['value'])
            if ind['relative'] is not None:
                val += '+' + operand_text(ind['relative'])
            s += val if i == 0 else '[' + val + ']'
        if o['component']:
            s += '.' + o['component']
    for ext in o['extensions']:
        if ext & 63 == 1:
            mod = (ext >> 6) & 255
            if mod in (2, 3):
                s = 'abs(' + s + ')'
            if mod in (1, 3):
                s = '-' + s
    return s


def decode(b, opcodes):
    assert len(b) % 4 == 0
    w = words(b, 0, len(b) // 4)
    assert w[0] == 0x10050 and w[1] == len(w)
    p = 2
    result = []
    while p < len(w):
        t = w[p]
        op = t & 2047
        assert op < len(opcodes)
        name = opcodes[op]
        n = w[p + 1] if name == 'CUSTOMDATA' else (t >> 24) & 127
        assert n >= 1 and p + n <= len(w)
        end = p + n
        args = []
        q = p + 1
        extended = t >> 31
        while extended:
            assert q < end
            extended = w[q] >> 31
            q += 1
        if name == 'CUSTOMDATA':
            pass
        elif name in ('DCL_TEMPS', 'DCL_INDEXABLE_TEMP', 'DCL_GLOBAL_FLAGS'):
            pass
        elif name.startswith('DCL_'):
            a, q = operand(w, q, end)
            args.append(a)
        else:
            while q < end:
                a, q = operand(w, q, end)
                args.append(a)
            assert q == end
        suffix = '_sat' if t & 0x2000 and not name.startswith('DCL_') else ''
        result.append({'word': p, 'opcode': name, 'opcode_token': hex(t), 'operands': args,
                       'words': [hex(x) for x in w[p:end]],
                       'text': name.lower() + suffix + ' ' + ', '.join(operand_text(a) for a in args)})
        p = end
    assert p == len(w)
    return result


def signature(b):
    """Observed ISGN/OSGN 24-byte records, retaining all words."""
    count, start = words(b, 0, 2)
    assert count < 256 and start == 8
    result = []
    for i in range(count):
        at = start + i * 24
        row = words(b, at, 6)
        result.append({'at': hex(at), 'semantic': text(b, row[0]),
                       'semantic_index': row[1], 'system_value': row[2],
                       'component_type': row[3], 'register': row[4],
                       'mask': row[5] & 255, 'words': list(row)})
    return result


def direct(o, typ, indices, component):
    return (o['type'] == typ and o['component'] == component and not o['extensions']
            and len(o['indices']) == len(indices)
            and all(a['representation'] == 0 and a['value'] == i and a['relative'] is None
                    for a, i in zip(o['indices'], indices)))


def all_operands(operands):
    for o in operands:
        yield o
        for index in o['indices']:
            if index['relative'] is not None:
                yield from all_operands([index['relative']])


def same_scalar(source, dest):
    return (dest['type'] in (0, 2) and len(dest['component']) == 1
            and direct(source, dest['type'], [a['value'] for a in dest['indices']], dest['component']))


def wetness_observation(reflection, ins, model_slot, shex_base):
    instance_name = 'g_InstanceParameter'
    resource, = [r for r in reflection['resources'] if r['name'] == instance_name]
    cb, = [r for r in reflection['constant_buffers'] if r['name'] == instance_name]
    variable, = [v for v in cb['variables'] if v['name'] == instance_name]
    field, = [m for m in variable['type']['members'] if m['name'] == 'm_Wetness']
    assert resource['kind'] == 0 and resource['bind_count'] == 1
    offset = variable['offset'] + field['offset']
    assert offset == 64 and field['type']['name'] == 'float4'
    assert (field['type']['class'], field['type']['type'], field['type']['rows'], field['type']['columns']) == (1, 3, 1, 4)
    slot, element = resource['bind_point'], offset // 16
    starts = [i for i, instruction in enumerate(ins) if not instruction['opcode'].startswith('DCL_')
              and any(a['type'] == 8 and a['indices'][0]['value'] == model_slot
                      for a in all_operands(instruction['operands']))]
    chains = []
    for start in starts:
        sequence = ins[start:start + 4]
        pattern = [x['opcode'] for x in sequence] == ['MAD', 'MUL', 'MAX', 'MIN']
        if pattern:
            mad = sequence[0]['operands']
            pattern = (len(mad) == 4 and direct(mad[2], 8, [model_slot, 0], 'x')
                       and direct(mad[3], 8, [slot, element], 'y'))
            for previous, instruction, component in zip(sequence, sequence[1:], 'xzw'):
                operands = instruction['operands']
                pattern = (pattern and len(operands) == 3 and same_scalar(operands[1], previous['operands'][0])
                           and direct(operands[2], 8, [slot, element], component))
            pattern = pattern and all(not (int(x['opcode_token'], 16) & 0x80002000) for x in sequence)
        chains.append({'exact_adjacent_mad_mul_max_min': pattern,
                       'instructions': [{'file_offset': hex(shex_base + x['word'] * 4), **x} for x in sequence]})
    return {'rdef_resource': resource, 'cb_size': cb['size'], 'variable_record': variable['at'],
            'variable_offset': variable['offset'], 'struct_name': variable['type']['name'],
            'field': field, 'absolute_cb_byte_offset': offset, 'chains': chains}


def inspect(path, out, opcodes):
    assert path.stat().st_size <= 128 << 20
    b = path.read_bytes()
    assert digest(b) == INPUT_DIGESTS[path.name]
    assert b[:4] == b'ShPk' and b[8:12] == b'DX11' and words(b, 12, 1)[0] == len(b)
    version, = words(b, 4, 1)
    assert version in (0xd01, 0xe01)
    code, names = words(b, 16, 2)
    counts = words(b, 24, 2) + words(b, 72, 3)
    assert sum(counts) < 65536
    p = 88 if version == 0xe01 else 84
    summary = {'file': path.name, 'bytes': len(b), 'sha256': digest(b), 'stages': list(counts), 'matched': []}
    for stage, count in zip(('VS', 'PS', 'HS', 'DS', 'GS'), counts):
        for index in range(count):
            meta = p
            off, size, *nr, unknown = struct.unpack_from('<II4HI', b, p)
            p += 20
            resources = []
            for kind, n in zip(('CB', 'Sampler', 'UAV', 'Texture'), nr):
                for _ in range(n):
                    at = p
                    rid, no, nl, flags, slot, sz = struct.unpack_from('<II4H', b, p)
                    p += 16
                    name = b[names + no:names + no + nl]
                    assert names + no + nl < len(b) and b[names + no + nl] == 0
                    assert rid == (~zlib.crc32(name, 0xffffffff) & 0xffffffff)
                    resources.append({'at': hex(at), 'kind': kind, 'id': hex(rid), 'name': name.decode(), 'slot': slot, 'size': sz, 'flags': flags})
            matches = [r for r in resources if r['name'] == NAME]
            if not matches:
                continue
            assert stage == 'VS' and len(matches) == 1 and matches[0]['kind'] == 'CB'
            slot = matches[0]['slot']
            base = code + off + 8
            raw = b[base:code + off + size]
            assert raw[:4] == b'DXBC' and words(raw, 24, 1)[0] == len(raw)
            nc, = words(raw, 28, 1)
            assert nc < 32
            chunks = {}
            spans = []
            for at in words(raw, 32, nc):
                tag = raw[at:at + 4].decode()
                sz, = words(raw, at + 4, 1)
                assert at >= 32 + 4 * nc and at + 8 + sz <= len(raw) and tag not in chunks
                assert all(at + 8 + sz <= lo or hi <= at for lo, hi in spans)
                spans.append((at, at + 8 + sz))
                chunks[tag] = (base + at + 8, raw[at + 8:at + 8 + sz])
            reflection = rdef(chunks['RDEF'][1])
            rr = [r for r in reflection['resources'] if r['name'] == NAME]
            assert len(rr) == 1 and rr[0]['kind'] == 0 and rr[0]['bind_point'] == slot and rr[0]['bind_count'] == 1
            model_cb = [r for r in reflection['constant_buffers'] if r['name'] == NAME]
            assert len(model_cb) == 1 and model_cb[0]['size'] == 16
            variable, = model_cb[0]['variables']
            field, = variable['type']['members']
            assert variable['offset'] == 0 and variable['size'] == 16 and variable['type']['name'] == 'ModelParameter'
            assert field['name'] == 'm_Params' and field['offset'] == 0 and field['type']['name'] == 'float4'
            assert (field['type']['class'], field['type']['type'], field['type']['rows'], field['type']['columns']) == (1, 3, 1, 4)
            ins = decode(chunks['SHEX'][1], opcodes)
            assert all(a['indices'][0]['representation'] == 0 and a['indices'][0]['relative'] is None
                       for i in ins for a in all_operands(i['operands']) if a['type'] == 8)
            wetness = wetness_observation(reflection, ins, slot, chunks['SHEX'][0])
            reads = []
            for instruction in ins:
                refs = [a for a in all_operands(instruction['operands']) if a['type'] == 8 and a['indices'][0]['value'] == slot]
                if refs and not instruction['opcode'].startswith('DCL_'):
                    assert all(a['indices'][1]['value'] == 0 and a['indices'][1]['relative'] is None for a in refs)
                    reads.append({'file_offset': hex(chunks['SHEX'][0] + instruction['word'] * 4), **instruction})
            entry = {'stage': stage, 'index': index, 'metadata_offset': hex(meta), 'resource': matches[0], 'dxbc_offset': hex(base),
                     'dxbc_sha256': digest(raw), 'rdef_offset': hex(chunks['RDEF'][0]), 'shex_offset': hex(chunks['SHEX'][0]),
                     'rdef_resource': rr[0], 'model_cb': model_cb[0], 'reads': reads, 'wetness': wetness}
            signatures = {tag: {'file_offset': hex(chunks[tag][0]), 'records': signature(chunks[tag][1])}
                          for tag in ('ISGN', 'OSGN')}
            assert len(reads) == len(wetness['chains']) == 1 and wetness['chains'][0]['exact_adjacent_mad_mul_max_min']
            source = reads[0]['operands'][1]
            assert source['type'] in (0, 1) and direct(source, source['type'], [0], 'y')
            dest = wetness['chains'][0]['instructions'][-1]['operands'][0]
            entry['direct_input_signature'] = ([s for s in signatures['ISGN']['records']
                                                if s['register'] == source['indices'][0]['value']]
                                               if source['type'] == 1 else [])
            entry['direct_output_signature'] = ([s for s in signatures['OSGN']['records']
                                                 if s['register'] == dest['indices'][0]['value']]
                                                if dest['type'] == 2 else [])
            if source['type'] == 1:
                sig, = entry['direct_input_signature']
                assert (sig['semantic'], sig['semantic_index']) == ('POSITION', 0)
            if dest['type'] == 2:
                sig, = entry['direct_output_signature']
                assert (sig['semantic'], sig['semantic_index'], dest['component']) == ('TEXCOORD', 4, 'w')
            summary['matched'].append(entry)
            # One witness per SHPK/slot retains complete instructions externally.
            if not any(x['resource']['slot'] == slot for x in summary['matched'][:-1]):
                detail = {'summary': entry, 'reflection': reflection, 'signatures': signatures, 'instructions': ins}
                stem = f'{path.stem}-{stage}-{index:03}'
                (out / (stem + '.json')).write_text(json.dumps(detail, indent=2) + '\n')
                (out / (stem + '.tokens.txt')).write_text(''.join(
                    f"file {chunks['SHEX'][0] + x['word'] * 4:#x} SHEX+{x['word'] * 4:#x} {x['text']}\n" for x in ins))
    print(path.name, 'matches', len(summary['matched']), 'wetness_patterns',
          dict(collections.Counter(c['exact_adjacent_mad_mul_max_min']
                                   for m in summary['matched'] for c in m['wetness']['chains'])))
    return summary


def main():
    root, header_path, out = [Path(p).resolve() for p in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(repo) and not out.is_relative_to(root)
    assert header_path.stat().st_size <= 1 << 20
    header = header_path.read_bytes()
    pin = json.loads((repo / 'tests/fixtures/pbd-scalar-consumer-observations.json').read_text())
    assert digest(header) == pin['token_header_sha256']
    enum = header.decode().split('typedef enum D3D10_SB_OPCODE_TYPE {', 1)[1].split('} D3D10_SB_OPCODE_TYPE;', 1)[0]
    enum = re.sub(r'//[^\n]*', '', enum)
    opcodes = re.findall(r'\bD3D\w+_SB_OPCODE_(\w+)\s*,', enum)
    paths = [root / name for name in sorted(INPUT_DIGESTS)]
    out.mkdir(parents=True, exist_ok=False)
    reports = [inspect(path, out, opcodes) for path in paths]
    data = json.dumps(reports, indent=2) + '\n'
    (out / 'report.json').write_text(data)
    execution = {'python': sys.version, 'input_directory': str(root), 'token_header': str(header_path),
                 'tool_sha256': digest(Path(__file__).read_bytes()), 'report_sha256': digest(data.encode())}
    (out / 'execution.json').write_text(json.dumps(execution, indent=2) + '\n')
    assert digest(data.encode()) == pin['shader_report_sha256']
    print('shader report', digest(data.encode()))


if __name__ == '__main__':
    main()

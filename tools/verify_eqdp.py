"""Independent full EQDP sparse-table expansion, no FFXIV parser dependency.

Args: game/sqpack new-output-directory. Only reads installation; all outputs go
outside it. Table observations and route correlations are distinct evidence.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib
from collections import Counter
from find_vfx_samples import standard
from verify_model import locate


def observe(b,slots=False):
    version, size, count = struct.unpack_from('<3H', b)
    assert version == 0x0101 and size > 0
    pointers = struct.unpack_from(f'<{count}H', b, 6)
    body = b[6 + count * 2:]
    used = max([0] + [(p + size) * 2 for p in pointers if p != 65535])
    assert used <= len(body)
    rows = [f'eqdp\t{len(b)}\t{size}\t{count}\t{len(body)-used}']
    # Sequential block expansion is independent of Rust's random-access lookup.
    values = []
    for p in pointers:
        values.extend([(0, 1)] * size if p == 65535 else [(v[0], 0) for v in struct.iter_unpack('<H', body[p * 2:(p + size) * 2])])
    rows.extend(f'entry\t{i}\t{v}\t{missing}'+('\t'+','.join(str(v>>(2*s)&3)for s in range(5))if slots else'') for i, (v, missing) in enumerate(values))
    text = '\n'.join(rows) + '\n'
    return text, {'sha256':hashlib.sha256(b).hexdigest(),'rows_sha256':hashlib.sha256(text.encode()).hexdigest(),
                  'bytes':len(b),'version':version,'block_size':size,'blocks':count,
                  'absent_blocks':pointers.count(65535),'entries':len(values),'trailing_bytes':len(body)-used,
                  'tail_all_zero':not any(body[used:]),'unknown_bit_entries':sum(bool(v & ~1023) for v,_ in values)}


def route_correlations(root, paths):
    keys = set()
    for index in sorted((root / 'ffxiv').glob('0400*.win32.index')):
        with index.open('rb') as f:
            f.seek(12); header, = struct.unpack('<I', f.read(4))
            f.seek(header + 8); offset, length = struct.unpack('<II', f.read(8))
            assert length <= 64 * 1024 * 1024
            f.seek(offset); keys.update(v[0] for v in struct.iter_unpack('<QII', f.read(length)))
    def exists(path):
        directory, name = path.rsplit('/', 1)
        crc = lambda text: ~zlib.crc32(text.encode('ascii')) & 0xffffffff
        return (crc(directory) << 32 | crc(name)) in keys
    reports = []
    for race, domain, path in paths:
        b = path.read_bytes(); _, size, count = struct.unpack_from('<3H', b)
        pointers = struct.unpack_from(f'<{count}H', b, 6)
        hist, examples = Counter(), {}
        for identity in range(size * count):
            p = pointers[identity // size]
            if p == 65535: continue
            value, = struct.unpack_from('<H', b, 6 + count * 2 + (p + identity % size) * 2)
            suffixes=['met','top','glv','dwn','sho']if domain=='equipment'else['ear','nek','wrs','rir','ril']
            for slot, suffix in enumerate(suffixes):
                pair = value >> (slot * 2) & 3
                if pair not in [1,2]: continue
                marker='e'if domain=='equipment'else'a'
                mdl = f'chara/{domain}/{marker}{identity:04}/model/c{race:04}{marker}{identity:04}_{suffix}.mdl'
                materials = [f'chara/{domain}/{marker}{identity:04}/material/v{v:04}/mt_c{race:04}{marker}{identity:04}_{suffix}_a.mtrl' for v in [1,2,3]]
                for kind, hit in [('mdl',exists(mdl)),('mtrl_v1_to_v3',any(map(exists, materials)))]:
                    key = f'{kind}/pair{pair}/exists{int(hit)}'
                    hist[key] += 1; examples.setdefault(key, mdl if kind == 'mdl' else materials)
        reports.append({'race':race,'domain':domain,'observations':dict(hist),'example_candidates':examples})
    return reports


def main():
    root, output = Path(sys.argv[1]), Path(sys.argv[2])
    assert not output.resolve().is_relative_to(root.parent.resolve())
    output.mkdir(parents=True,exist_ok=False)
    manifest_path = Path(sys.argv[3])if len(sys.argv)>3 else Path(__file__).resolve().parent.parent / 'tests/fixtures/eqdp-observations.json'
    manifest=json.loads(manifest_path.read_text());pinned=manifest['samples']
    version=(root.parent/'ffxivgame.ver').read_text().strip();assert version==manifest['version']
    pins = {s['path']:s for s in pinned}
    reports, paths = [], []
    for sample in pinned:
        path=sample['path'];race=int(Path(path).stem[1:]);domain='accessory'if '/accessorydeformerparameter/'in path else'equipment'
        target = output / f'{domain}-c{race:04}.eqdp'
        paths.append((race,domain,target))
        subprocess.run(['target/debug/examples/local_sqpack',str(root),path,str(target)],check=True,capture_output=True)
        dat,entry=locate(root,path)
        with dat.open('rb')as f:independent=standard(f,entry)
        assert independent==target.read_bytes()
        expected, report = observe(target.read_bytes())
        actual = subprocess.check_output(['target/debug/examples/validate_eqdp',str(target)],text=True)
        assert expected == actual, 'full EQDP table differs'
        (output / f'{domain}-c{race:04}.tsv').write_text(actual)
        expected_slots,_=observe(target.read_bytes(),True)
        actual_slots=subprocess.check_output(['target/debug/examples/validate_eqdp',f'--{domain}-slots',str(target)],text=True);assert expected_slots==actual_slots
        (output/f'{domain}-c{race:04}.slots.tsv').write_text(actual_slots)
        report['slot_rows_sha256']=hashlib.sha256(actual_slots.encode()).hexdigest()
        report['path'] = path
        if 'sha256'in pins[path]:
            assert all(report[k] == pins[path][k] for k in ['sha256','rows_sha256'])
            if'slot_rows_sha256'in pins[path]:assert report['slot_rows_sha256']==pins[path]['slot_rows_sha256']
            report['pinned_regression'] = 'equal'
        else: report['pinned_regression'] = 'unregistered sample'
        reports.append(report)
    result = {'version':(root.parent / 'ffxivgame.ver').read_text().strip(),'platform':'Win32','python':sys.version,
              'samples':reports,'route_correlations':route_correlations(root,paths),
              'comparison':'all logical entry words and absent-block indicators equal'}
    text = json.dumps(result,indent=2) + '\n'; (output / 'report.json').write_text(text); print(text,end='')


if __name__ == '__main__': main()

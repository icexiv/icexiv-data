"""Reproduce pinned rendering-resource regressions from an authorized installation.

Args: game/sqpack authorized-ffxiv_dx11.exe new-output-directory.
Prerequisites: cargo build --features local --examples, Python with Pillow,
installed objdump. No target execution, server, GPU or game-install writes.
Passing this suite is not completion of docs/requirements-coverage.md.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time


def main():
    root, exe, out = [Path(v).resolve() for v in sys.argv[1:]]
    repo = Path(__file__).resolve().parent.parent
    assert not out.is_relative_to(root.parent) and not out.is_relative_to(repo)
    out.mkdir(parents=True, exist_ok=False)
    inputs = out / 'inputs'
    inputs.mkdir()
    version = (root.parent / 'ffxivgame.ver').read_text().strip()
    fixtures = repo / 'tests/fixtures'
    assert version == json.loads((fixtures / 'bc6h-observations.json').read_text())['version']
    assert exe.stat().st_size <= 128 << 20
    binary_sha256 = hashlib.sha256(exe.read_bytes()).hexdigest()
    assert binary_sha256 == json.loads((fixtures / 'bc6h-observations.json').read_text())['binary_sha256']
    source_digest = hashlib.sha256()
    source_files = []
    for pattern in ('src/**/*.rs', 'tests/**/*.rs', 'tests/fixtures/*.json', 'examples/*.rs', 'tools/*.py', 'tools/bc6h_oracle/*'):
        source_files.extend(repo.glob(pattern))
    source_files.extend(repo / p for p in ('Cargo.toml', 'Cargo.lock', 'docs/requirements-coverage.md'))
    for path in sorted(set(source_files)):
        data = path.read_bytes()
        source_digest.update(str(path.relative_to(repo)).encode()+b'\0'+hashlib.sha256(data).digest())
    state = {'version': version, 'platform': 'Win32', 'binary_sha256': binary_sha256,
        'source_set_sha256': source_digest.hexdigest(), 'steps': [],
        'scope': 'Pinned extraction/metadata/pixel/math regressions, not whole-goal acceptance. Known-gap observations do not count as successful decoding.',
        'coverage_sha256': hashlib.sha256((repo / 'docs/requirements-coverage.md').read_bytes()).hexdigest()}

    def save():
        (out / 'report.json').write_text(json.dumps(state, indent=2)+'\n')
        lines = ['# Rendering-resource regression results', '',
            state['scope'], '', f'Installation: `{version}`; platform: Win32.', '',
            '| Check | Kind | Result | Report |', '| --- | --- | --- | --- |']
        for s in state['steps']:
            if s['kind'] == 'preparation':
                continue
            result = 'passed' if s['returncode'] == 0 else str(s['returncode'])
            report = f'[{s["report"]}]({s["report"]})' if 'report' in s else f'[{s["log"]}]({s["log"]})'
            lines.append(f'| {s["label"]} | {s["kind"]} | {result} | {report} |')
        lines += ['', 'Volume output-size gaps remain unresolved. Passing gap/inventory observations reproduce explicit extraction errors; they do not reconstruct missing bytes.',
            '', f'Coverage reference: `{repo / "docs/requirements-coverage.md"}`.',
            '', f'Source-set SHA-256: `{state["source_set_sha256"]}`.',
            '', f'Executable SHA-256: `{binary_sha256}`.', '']
        (out / 'summary.md').write_text('\n'.join(lines))

    def run(label, argv, report=None, kind='verification'):
        argv = list(map(str, argv))
        start = time.monotonic()
        log = out / f'{len(state["steps"]):03d}-{label}.log'
        with log.open('x') as f:
            try:
                done = subprocess.run(argv, cwd=repo, stdout=f, stderr=subprocess.STDOUT, timeout=1200)
                code = done.returncode
            except subprocess.TimeoutExpired:
                code = 'timeout'
        if code == 0 and report is not None:
            if not report.is_file():
                code = 'missing-report'
            else:
                try:
                    result = json.loads(report.read_text())
                except (ValueError, OSError):
                    code = 'invalid-report'
                else:
                    def unregistered(value):
                        if isinstance(value, dict):
                            return any(unregistered(v) for v in value.values())
                        if isinstance(value, list):
                            return any(unregistered(v) for v in value)
                        return isinstance(value, str) and 'unregistered' in value
                    if unregistered(result):
                        code = 'unregistered-sample'
        step = {'label': label, 'kind': kind, 'command': argv, 'returncode': code,
            'seconds': round(time.monotonic()-start, 3), 'log': log.name}
        if report is not None and report.is_file():
            step['report'] = str(report.relative_to(out))
            step['report_sha256'] = hashlib.sha256(report.read_bytes()).hexdigest()
        state['steps'].append(step)
        save()
        print(f'{label}: {code}', flush=True)
        if code != 0:
            raise SystemExit(f'{label} failed; see {log}')

    def verify(label, tool, *args, kind='verification'):
        dest = out / label
        run(label, [sys.executable, repo / 'tools' / tool, root, dest, *args], dest / 'report.json', kind)
        return dest

    def extract(label, source, file=None, max_bytes=None):
        path = inputs / (file or label)
        if isinstance(source, str):
            command = [repo / 'target/debug/examples/local_sqpack', root, source, path]
        elif 'dat' in source:
            command = [repo / 'target/debug/examples/extract_entry', root, source['dat'], source['offset'], path]
        else:
            command = [repo / 'target/debug/examples/local_sqpack', root, source['path'], path]
        if max_bytes is not None:
            assert command[0].name == 'local_sqpack'
            command[1:1] = ['--max-bytes', max_bytes]
        run('extract-'+label, command, kind='preparation')
        return path

    # IMC, EQDP, EQP, CMP, PBD and skeletal/animation families.
    verify('imc', 'verify_imc.py')
    verify('imc-variants', 'verify_imc_variants.py')
    verify('eqdp', 'verify_eqdp.py')
    verify('eqdp-variants', 'verify_eqdp.py', fixtures / 'eqdp-variant-observations.json')
    verify('eqdp-links', 'verify_eqdp_links.py')
    eqp = extract('equipmentparameter.eqp', 'chara/xls/equipmentparameter/equipmentparameter.eqp')
    run('eqp', [sys.executable, repo / 'tools/verify_eqp.py', eqp, out / 'eqp'], out / 'eqp/report.json')
    run('eqp-hair', [sys.executable, repo / 'tools/verify_eqp_hair.py', eqp, out / 'eqp-hair'], out / 'eqp-hair/report.json')
    verify('eqp-semantics', 'verify_eqp_semantics.py')
    verify('eqp-ear-tail', 'verify_eqp_semantics.py', fixtures / 'eqp-ear-tail-observations.json')
    run('eqp-static', [sys.executable, repo / 'tools/verify_client_eqp.py', exe, out / 'eqp-static'], out / 'eqp-static/report.json')
    cmp = extract('human.cmp', 'chara/xls/charamake/human.cmp')
    run('cmp', [sys.executable, repo / 'tools/verify_cmp.py', cmp, out / 'cmp'], out / 'cmp/report.json')
    run('cmp-static', [sys.executable, repo / 'tools/verify_client_cmp.py', exe, out / 'cmp-static'], out / 'cmp-static/report.json')
    run('cmp-interpolation', [sys.executable, repo / 'tools/verify_cmp_interpolation.py', cmp, out / 'cmp-interpolation'], out / 'cmp-interpolation/report.json')
    pbd = extract('human.pbd', 'chara/xls/boneDeformer/human.pbd')
    run('pbd', [sys.executable, repo / 'tools/verify_pbd.py', pbd, out / 'pbd'], out / 'pbd/report.json')
    run('pbd-static', [sys.executable, repo / 'tools/verify_client_pbd.py', exe, pbd, out / 'pbd-static'], out / 'pbd-static/report.json')
    run('pbd-binding-static', [sys.executable, repo / 'tools/verify_client_pbd_bindings.py', exe,
        out / 'pbd-binding-static'], out / 'pbd-binding-static/report.json')
    run('pbd-composition', [sys.executable, repo / 'tools/verify_pbd_composition.py', pbd, out / 'pbd-composition'], out / 'pbd-composition/report.json')
    run('pbd-inverse', [sys.executable, repo / 'tools/verify_pbd_inverse.py', pbd, out / 'pbd-inverse'], out / 'pbd-inverse/report.json')
    run('pbd-paths', [sys.executable, repo / 'tools/verify_pbd_paths.py', pbd, out / 'pbd-paths'], out / 'pbd-paths/report.json')
    run('epbd-scalars', [sys.executable, repo / 'tools/verify_epbd_scalars.py', pbd, out / 'epbd-scalars'], out / 'epbd-scalars/report.json')
    verify('skeleton', 'verify_skeleton.py')
    run('pbd-reference-hypotheses', [sys.executable, repo / 'tools/observe_pbd_reference_relations.py', pbd,
        *[out / 'skeleton' / f'skl_c{identity:04}b0001.sklb' for identity in (101, 201, 901, 1801)],
        out / 'pbd-reference-hypotheses'], out / 'pbd-reference-hypotheses/report.json', kind='inventory-observation')
    verify('pap', 'verify_animation.py')
    for variant in ('variant', 'empty', 'large'):
        verify('pap-'+variant, 'verify_animation.py', fixtures / f'pap-{variant}-observations.json')

    # Complete baseline type-3 reconstruction and MDL semantic comparison.
    model_paths = ['chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl',
        'chara/equipment/e0001/model/c0101e0001_top.mdl',
        'chara/human/c0101/obj/face/f0001/model/c0101f0001_fac.mdl',
        'chara/equipment/e0600/model/c0101e0600_top.mdl',
        'chara/human/c0101/obj/hair/h0001/model/c0101h0001_hir.mdl',
        '@ffxiv/010000.win32.dat0:31060736', '@ffxiv/010000.win32.dat0:89515904',
        '@ffxiv/010000.win32.dat0:1097222400', '@ffxiv/010000.win32.dat1:403897472',
        '@ffxiv/010000.win32.dat1:59152640']
    models = verify('model-reconstruction', 'verify_model.py', *model_paths)
    run('mdl', [sys.executable, repo / 'tools/verify_mdl.py', out / 'mdl',
        *[models / f'{i}.mdl' for i in range(len(model_paths))]], out / 'mdl/report.json')
    run('mdl-semantic-queries', [sys.executable, repo / 'tools/verify_mdl_semantics.py', models / '0.mdl', out / 'mdl-semantic-queries'], out / 'mdl-semantic-queries/report.json')
    verify('mdl-semantic-variants', 'verify_mdl_semantic_variants.py')
    verify('mdl-extended-inputs', 'verify_mdl_semantic_variants.py', fixtures / 'mdl-extended-input-observations.json')
    verify('mdl-roles', 'verify_mdl_roles.py')
    run('mdl-glass-inventory', [sys.executable, repo / 'tools/find_mdl_glass.py', root, out / 'mdl-glass-inventory.json'], out / 'mdl-glass-inventory.json', kind='inventory-observation')

    # MTRL/SHPK and dye templates, preserving shader filenames for ID links.
    material_sources = [
        'chara/weapon/w2001/obj/body/b0102/material/v0001/mt_w2001b0102_a.mtrl',
        {'dat':'ffxiv/040000.win32.dat0', 'offset':1569636480},
        'chara/human/c0101/obj/face/f0001/material/mt_c0101f0001_fac_a.mtrl',
        'bgcommon/hou/dyna/opt/sg/0009/material/opt_sg_m0009_3a.mtrl',
        'chara/equipment/e0001/material/v0001/mt_c0101e0001_top_a.mtrl']
    materials = [extract(f'material-{i}.mtrl', s) for i,s in enumerate(material_sources)]
    shaders = [extract(name+'.shpk', f'shader/sm5/shpk/{name}.shpk', max_bytes=128 << 20 if name == 'bg' else None)
        for name in ('character', 'characterlegacy', 'skin', 'bgcrestchange', 'water', 'bg', 'hair')]
    run('materials', [sys.executable, repo / 'tools/verify_materials.py', out / 'materials', *materials, *shaders], out / 'materials/report.json')
    run('client-vertex-inputs', [sys.executable, repo / 'tools/verify_client_vertex_inputs.py', exe,
        out / 'client-vertex-inputs', *shaders], out / 'client-vertex-inputs/report.json')
    run('mtrl-static-dye', [sys.executable, repo / 'tools/verify_client_mtrl.py', exe, materials[1], out / 'mtrl-static-dye'], out / 'mtrl-static-dye/report.json')
    run('dye-lane-profiles', [sys.executable, repo / 'tools/verify_dye_lanes.py', exe,
        materials[0], materials[4], out / 'dye-lane-profiles'], out / 'dye-lane-profiles/report.json')
    verify('mtrl-legacy-inventory', 'find_legacy_dye_materials.py', kind='inventory-observation')
    verify('shpk-variants', 'verify_shpk_variants.py')
    stms = [extract(f'stm-{i}.stm', s) for i,s in enumerate(json.loads((fixtures / 'stm-observations.json').read_text())['samples'])]
    stain = [extract('stain.exh', 'exd/stain.exh'), extract('stain_0_chs.exd', 'exd/stain_0_chs.exd')]
    run('stm', [sys.executable, repo / 'tools/verify_stm.py', out / 'stm', *stms, *stain, *materials], out / 'stm/report.json')
    run('dye-row-overrides', [sys.executable, repo / 'tools/verify_dye_rows.py', exe,
        *stms, materials[0], materials[4], out / 'dye-row-overrides'], out / 'dye-row-overrides/report.json')
    run('color-table-outputs', [sys.executable, repo / 'tools/verify_color_tables.py', exe,
        out, out / 'color-table-outputs'], out / 'color-table-outputs/report.json')

    # Every baseline texture image, real variants and controlled signed BC6H.
    texs = [extract(f'tex-{i}', s, s['file']) for i,s in enumerate(json.loads((fixtures / 'tex-observations.json').read_text())['samples'])]
    run('tex', [sys.executable, repo / 'tools/verify_tex.py', root, out / 'tex', *texs], out / 'tex/report.json')
    run('cube-face-bytes', [sys.executable, repo / 'tools/verify_cube_faces.py',
        inputs / 'cube-rgba4.tex', inputs / 'cube-bc7-mips.tex', out / 'cube-face-bytes'],
        out / 'cube-face-bytes/report.json')
    verify('tex-variants', 'verify_texture_variants.py')
    verify('bc45-textures', 'verify_texture_variants.py', fixtures / 'bc45-texture-observations.json')
    run('bc45-controls', [sys.executable, repo / 'tools/verify_bc45_controls.py', out / 'bc45-controls'], out / 'bc45-controls/report.json')
    run('bc6h', [sys.executable, repo / 'tools/verify_bc6h.py', root, exe, out / 'bc6h'], out / 'bc6h/report.json')
    run('bc6h-modes', [sys.executable, repo / 'tools/verify_bc6h_modes.py', out / 'bc6h-modes'], out / 'bc6h-modes/report.json')
    run('float-controls', [sys.executable, repo / 'tools/verify_float_texture_controls.py', exe, out / 'float-controls'], out / 'float-controls/report.json')
    verify('texture-known-gaps', 'observe_texture_group_sizes.py', kind='known-gap-observation')
    verify('volume-inventory', 'verify_volume_inventory.py', '--all-records', kind='inventory-observation')
    run('texture-full-inventory', [sys.executable, repo / 'tools/inventory_texture_headers.py', root,
        out / 'texture-full-inventory.json', '--all-records'],
        out / 'texture-full-inventory.json', kind='inventory-observation')
    run('standard-texture-full-inventory', [sys.executable, repo / 'tools/inventory_standard_textures.py', root,
        out / 'standard-texture-full-inventory.json', '--all-indexes'],
        out / 'standard-texture-full-inventory.json', kind='inventory-observation')
    run('volume-counterparts', [sys.executable, repo / 'tools/observe_volume_counterparts.py',
        root, out / 'volume-inventory/report.json', out / 'volume-counterparts'],
        out / 'volume-counterparts/report.json', kind='inventory-observation')
    verify('index2-collision-inventory', 'observe_index2_collisions.py', kind='inventory-observation')
    run('index2-collision-lookup', ['python', 'tools/verify_index2_lookup.py', root,
        out / 'index2-collision-inventory/report.json', out / 'index2-collision-lookup'],
        out / 'index2-collision-lookup/report.json')
    run('texture-group-static', [sys.executable, repo / 'tools/verify_client_texture_groups.py', exe, out / 'texture-group-static'], out / 'texture-group-static/report.json')

    verify('avfx', 'verify_avfx.py')
    verify('avfx-assets', 'verify_avfx_assets.py')
    verify('avfx-variants', 'verify_avfx_assets.py', fixtures / 'avfx-variant-observations.json')
    verify('atex', 'verify_atex.py')
    verify('weapon-binder', 'verify_weapon_binder.py')
    verify('sgb', 'verify_sgb.py')
    verify('sgb-inventory', 'verify_sgb_inventory.py')
    verify('atex-pixels', 'verify_atex_pixels.py')
    verify('excel', 'verify_excel.py')
    run('mutation-safety', [sys.executable, repo / 'tools/verify_mutation_safety.py', root,
        out, out / 'mutation-safety'], out / 'mutation-safety/report.json')
    state['completed_regressions'] = sum(s['kind'] == 'verification' for s in state['steps'])
    state['known_gap_observations'] = sum(s['kind'] == 'known-gap-observation' for s in state['steps'])
    state['inventory_observations'] = sum(s['kind'] == 'inventory-observation' for s in state['steps'])
    state['verification_suite_passed'] = True
    save()


if __name__ == '__main__':
    main()

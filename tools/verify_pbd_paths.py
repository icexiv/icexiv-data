"""All PBD paths: independent child/sibling BFS versus Rust parent traversal.

Args: extracted-human.pbd new-output-directory. Uses existing independent wire
observations, never client code, runtime route policy or a skeleton assembly.
"""
from collections import deque
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from verify_pbd import observe


def main():
    source, out = map(Path, sys.argv[1:])
    assert source.stat().st_size <= 64 << 20
    out.mkdir(parents=True, exist_ok=False)
    fixtures = Path(__file__).resolve().parent.parent / 'tests/fixtures'
    pin = json.loads((fixtures / 'pbd-observations.json').read_text())
    data = source.read_bytes()
    assert hashlib.sha256(data).hexdigest() == pin['sha256']
    rows, tables = [], []
    stats = dict.fromkeys(['pbd_tables', 'deformers', 'bones', 'float_coefficients',
        'nonidentity_matrices', 'translated_matrices', 'offdiagonal_matrices', 'extended_entries'], 0)
    observe(data, 'root', rows, stats)
    assert hashlib.sha256(('\n'.join(rows)+'\n').encode()).hexdigest() == pin['rows_sha256']
    for row in rows:
        f = row.split('\t')
        if f[0] == 'pbd': tables.append((f[1], int(f[2]), {}, {}))
        elif f[0] == 'deformer': tables[-1][2][int(f[1])] = int(f[3])
        elif f[0] == 'connection': tables[-1][3][int(f[1])] = tuple(map(int, f[2:]))
    expected = []
    counts = dict.fromkeys(('ordered_pairs', 'empty_paths', 'unrelated_pairs', 'nonempty_paths', 'path_nodes', 'maximum_path_nodes'), 0)
    for path, n, deformers, links in tables:
        expected.append(f'table\t{path}\t{n}')
        paths = {}
        for ancestor in range(n):
            queue = deque([(deformers[ancestor], [])])
            seen = set()
            while queue:
                connection, chain = queue.popleft()
                assert connection not in seen
                seen.add(connection)
                parent, child, sibling, deformer = links[connection]
                paths[(deformer, ancestor)] = list(reversed(chain))
                siblings = set()
                while child != -1:
                    assert child not in siblings
                    siblings.add(child)
                    queue.append((child, chain+[links[child][3]]))
                    child = links[child][2]
        for descendant in range(n):
            for ancestor in range(n):
                route = paths.get((descendant, ancestor))
                counts['ordered_pairs'] += 1
                if route is None:
                    value = 'unrelated'
                    counts['unrelated_pairs'] += 1
                elif not route:
                    value = 'empty'
                    counts['empty_paths'] += 1
                else:
                    value = ','.join(map(str, route))
                    counts['nonempty_paths'] += 1
                    counts['path_nodes'] += len(route)
                    counts['maximum_path_nodes'] = max(counts['maximum_path_nodes'], len(route))
                expected.append(f'path\t{descendant}\t{ancestor}\t{value}')
    expected = '\n'.join(expected)+'\n'
    rust = subprocess.check_output(['target/debug/examples/validate_pbd_paths', str(source)], text=True, timeout=120)
    assert rust == expected
    (out / 'rust.tsv').write_text(rust)
    (out / 'python.tsv').write_text(expected)
    result = {'resource_sha256': pin['sha256'], 'tables': len(tables), 'statistics': counts,
        'rows_sha256': hashlib.sha256(expected.encode()).hexdigest(),
        'limitations': 'File forest paths only; no race fallback, EPBD selection, bone-name matching, matrix composition or source/target-space claim.'}
    extra = fixtures / 'pbd-path-observations.json'
    if extra.exists():
        saved = json.loads(extra.read_text())
        assert all(result[k] == saved[k] for k in ('resource_sha256', 'tables', 'statistics', 'rows_sha256'))
    text = json.dumps(result, indent=2)+'\n'
    (out / 'report.json').write_text(text)
    print(text, end='')


if __name__ == '__main__':
    main()

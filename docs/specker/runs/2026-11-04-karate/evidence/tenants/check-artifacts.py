"""Run from repository root; validate this checkpoint's contracts and evidence."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib
from urllib.parse import unquote

base = '4225dc9b1852c2b6b896848088a4fd1461ab6287'
root = Path.cwd()
evidence = root / 'docs/specker/runs/2026-11-04-karate/evidence/tenants'
files = sorted(set(subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], text=True).splitlines()))
snapshot = [{'path': path, 'sha256': hashlib.sha256((root/path).read_bytes()).hexdigest()}
            for path in files if not path.startswith('docs/specker/runs/2026-11-04-karate/evidence/')]
(evidence / 'snapshot.json').write_text(json.dumps({'base_commit': base, 'files': snapshot}, indent=2) + '\n')
print('Snapshot files hashed:', len(snapshot))

links = 0
for path in files:
    file = root / path
    if file.suffix == '.md':
        for href in re.findall(r'\]\(([^)]+)\)', file.read_text()):
            href = href.split('#')[0]
            if not href or re.match(r'[a-zA-Z]+:', href):
                continue
            assert (file.parent / unquote(href)).exists(), (path, href)
            links += 1
print('Relative documentation links valid:', links)
report = json.loads((evidence / 'coverage-summary.json').read_text())
for item in report['files']:
    assert hashlib.sha256((root / item['path']).read_bytes()).hexdigest() == item['sha256'], item['path']
assert hashlib.sha256((root / 'Cargo.lock').read_bytes()).hexdigest() == report['lock_sha256']
for component in report['components']:
    for metric in ['lines', 'branches']:
        counts = component[metric]
        assert counts['covered'] * 100 > counts['count'] * 90
print('Coverage matches current product sources/lock; strict component thresholds pass')
contract = json.loads((root / 'docs/contracts/projects-v1.openapi.json').read_text())
operations = set()
def validate(value):
    if isinstance(value, dict):
        if '$ref' in value:
            current = contract
            for name in value['$ref'][2:].split('/'):
                current = current[name]
        if 'operationId' in value:
            assert value['operationId'] not in operations
            operations.add(value['operationId'])
            for security in value.get('security', []):
                assert set(security) <= set(contract['components']['securitySchemes'])
        for item in value.values():
            validate(item)
    elif isinstance(value, list):
        for item in value:
            validate(item)
validate(contract)
print('OpenAPI references, security schemes and unique operations valid:', len(operations))
old = tomllib.loads(subprocess.check_output(['git', 'show', base + ':Cargo.lock'], text=True))
new = tomllib.loads((root / 'Cargo.lock').read_text())
before = {(p['name'], p['version']) for p in old['package']}
after = {(p['name'], p['version']) for p in new['package']}
assert not before - after
print('Existing dependency versions preserved; added packages:', len(after - before))
subprocess.run(['git', 'diff', '--exit-code', base, '--', 'LICENSE'], check=True)
subprocess.run(['git', 'diff', '--check'], check=True)
subprocess.run(['git', 'diff', '--cached', '--check'], check=True)
assert json.loads((evidence / 'gitleaks.json').read_text()) == []
assert not (root / 'target').exists()
print('Product MIT license unchanged; whitespace and secrets clean; build artifacts removed')

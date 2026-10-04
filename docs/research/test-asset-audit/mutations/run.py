"""Execute one manually designed audit mutant, with exact source restoration."""
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[4]
plan_path = Path(sys.argv[1]).resolve()
plan = json.loads(plan_path.read_text())
out = root / 'target/test-asset-audit' / f'm{plan["sample"]:02}'
out.mkdir(parents=True, exist_ok=True)
results = []
def run(phase, argv, env=None):
    started = time.time()
    with (out / f'{phase}.log').open('w') as log:
        result = subprocess.run(argv, cwd=root, stdout=log, stderr=subprocess.STDOUT, env=env)
    text = (out / f'{phase}.log').read_text()
    entry = dict(phase=phase, command=argv, exit=result.returncode,
                 seconds=round(time.time()-started, 3), log=str(out / f'{phase}.log'))
    results.append(entry)
    print(json.dumps(entry), flush=True)
    return result.returncode

def check(phase):
    argv = ['python3', 'scripts/verification.py', 'step', f'audit-m{plan["sample"]:02}-{phase}', '--']
    return run(phase, argv + plan['command'])

source = root / plan['source']
original = source.read_bytes()
assert hashlib.sha256(original).hexdigest() == plan['source_sha256']
text = original.decode()
assert text.count(plan['old']) == 1
mutated = text.replace(plan['old'], plan['new'])
patch = ''.join(difflib.unified_diff(text.splitlines(True), mutated.splitlines(True),
    fromfile='a/'+plan['source'], tofile='b/'+plan['source']))
(out / 'mutant.patch').write_text(patch)
(out / 'source.original').write_bytes(original)
binaries = {}
status = 'blocked'
try:
    baseline = check('baseline')
    if baseline == 0:
        source.write_text(mutated)
        ready = True
        if plan.get('release'):
            for name in ['storyos-server', 'storyos-worker', 'storyos-storage']:
                file = root / 'target/release-package' / name
                backup = out / (name + '.original')
                shutil.copy2(file, backup)
                binaries[file] = backup
            env = dict(os.environ)
            env['STORYOS_WEB_MANIFEST_SHA256'] = 'sha256:' + hashlib.sha256((root/'target/release-package/web/manifest.json').read_bytes()).hexdigest()
            ready = run('build', ['python3', 'scripts/verification.py', 'step',
                f'audit-m{plan["sample"]:02}-build', '--', 'cargo', 'build', '--locked', '--release',
                '--target-dir', 'target/web-release', '-p', 'storyos-server', '-p', 'storyos-worker-bin',
                '-p', 'storyos-adapter-postgres'], env) == 0
            if ready:
                for file in binaries: shutil.copy2(root/'target/web-release/release'/file.name, file)
        if ready:
            mutant = check('mutant')
            status = 'candidate-kill' if mutant else 'candidate-miss'
            if plan.get('removed_command'):
                run('removed-mutant', ['python3', 'scripts/verification.py', 'step', f'audit-m{plan["sample"]:02}-removed-mutant', '--'] + plan['removed_command'])
finally:
    source.write_bytes(original)
    for file, backup in binaries.items(): shutil.copy2(backup, file)
    assert source.read_bytes() == original
    restored = check('restored')
    if restored: status = 'blocked-restoration-check'
    record = dict(sample=plan['sample'], id=plan['id'], status=status,
        source=plan['source'], source_restored=True, results=results)
    (out/'result.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps({'status':status,'result':str(out/'result.json')}),flush=True)

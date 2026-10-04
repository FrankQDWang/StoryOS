#!/usr/bin/env python3
"""Replay the fixed matrix and commit evidence after each completed case."""
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
round_name = sys.argv[1]
cases = sys.argv[2:] or [
    'durable', 'lost-ack', 'cut-admission-server', 'cut-admission-database',
    'cut-core-server', 'cut-core-database', 'cut-commit-server', 'cut-commit-database',
    'concurrent-rename', 'concurrent-retry', 'concurrent-rename-restart',
    'concurrent-author', 'session-replay', 'takeover-server', 'takeover-database', 'takeover-concurrent',
]
target = root / 'target/durability-verification' / round_name
evidence = root / 'docs/research/durability-verification/evidence' / round_name
target.mkdir(parents=True, exist_ok=True)
evidence.mkdir(parents=True, exist_ok=True)
for case in cases:
    output = target / f'{case}.json'
    command = ['scripts/dev-postgres.sh', 'run', 'node', 'prototypes/durability-verification/driver.mjs', case, str(output)]
    print(f'Start {round_name}/{case}', flush=True)
    with (target / f'{case}.log').open('w') as log:
        result = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT)
    data = json.loads(output.read_text()) if output.exists() else {'scenario': case, 'events': []}
    data['exit_code'] = result.returncode
    data['driver_commit'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    data['command'] = ['scripts/dev-postgres.sh', 'run', 'node', 'prototypes/durability-verification/driver.mjs', case]
    (evidence / f'{case}.json').write_text(json.dumps(data, separators=(',', ':')) + '\n')
    errors = [e['data'] for e in data['events'] if e['name'] == 'blocked_or_probe_error']
    verdict = 'invariant failure' if result.returncode == 1 else 'completed' if result.returncode == 0 else 'blocked'
    with (root / 'docs/research/durability-verification/PROGRESS.md').open('a') as progress:
        progress.write(f'- Matrix {round_name}, `{case}`: {verdict} (exit {result.returncode}). '
                       f'Evidence: `evidence/{round_name}/{case}.json`.\n')
        if errors:
            progress.write('  Reason: ' + errors[0]['message'].replace('\n', ' ') + '\n')
    subprocess.run(['git', 'add', 'docs/research/durability-verification'], cwd=root, check=True)
    subprocess.run(['git', 'commit', '-m', f'Record {round_name} {case} evidence'], cwd=root, check=True, stdout=subprocess.DEVNULL)
    print(f'End {round_name}/{case}: {verdict}', flush=True)

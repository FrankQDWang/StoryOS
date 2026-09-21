"""Read current results and execute policy-registered targeted checks."""

from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

import verification_cache
import verification_graph


def targeted_plan(root, check):
    import verification as runner
    policy = (root / 'docs/agents/verification-policy.json').read_bytes()
    files = runner.inventory(root)['files']
    registered = json.loads(policy).get('targeted', {})
    if check not in registered:
        raise ValueError('Unknown targeted check; inspect verification-policy.json')
    entry = registered[check]
    plan = {'version': 1, 'check': check, 'source': runner.source_identity(root),
            'policy_sha256': hashlib.sha256(policy).hexdigest(), 'command': entry['command'],
            'test_files': sorted(f['path'] for f in files if f['kind'].endswith('-test') and (root / f['path']).is_file()),
            'checks': [{'group': check, 'status': 'pending' if entry['clean'] and
                       runner.source_identity(root)['dirty'] else 'ready'}],
            'workers': 'existing-targeted-profile'}
    import verification_candidate
    plan['execution_inputs_sha256'] = verification_cache.digest({k: v for k, v in verification_candidate.environment().items()
        if k not in {'_', 'SHLVL', 'STORYOS_VERIFICATION_RUN', 'STORYOS_VERIFICATION_PARENT', 'PYTHONDONTWRITEBYTECODE'}})
    verification_graph.attach(root, plan, json.loads(policy), files)
    plan['digest'] = verification_cache.digest(plan)
    return plan


def execute(root, check, context):
    import verification as runner
    plan = targeted_plan(root, check)
    if os.environ.get('STORYOS_VERIFICATION_RUN'):
        return runner.step(root, check, plan['command'], node_id="targeted:" + check)
    if plan['checks'][0]['status'] == 'pending':
        import verification_candidate
        verification_candidate.observe(root, 'refused', issue=context.get('issue'),
                                       reason='Release package requires clean sources', check=check)
        return 2
    command = [sys.executable, str(Path(runner.__file__).resolve()), 'step', check, '--', *plan['command']]
    return runner.run(root, command, plan=plan, context={**context, 'profile': 'targeted'})


def process_state(process):
    result = subprocess.run(['ps', '-o', 'lstart=', '-p', str(process.get('pid', 0))],
                            capture_output=True, text=True)
    return 'active' if result.returncode == 0 and result.stdout.strip() == process.get('birth') else 'lost'


def status(root, plan):
    next_command = (f"python3 scripts/verification.py targeted --check {shlex.quote(plan['check'])}"
                    if 'check' in plan else f"make verify-changed BASE={plan['base']}")
    if plan.get('changes') == []:
        next_command = 'make verify-targeted CHECK=verify-policy'
    result = {'status': 'pending', 'plan': plan, 'next_command': next_command}
    reports = []
    for path in (root / 'target/verification').glob('*/report.json'):
        try:
            report = json.loads(path.read_text())
            previous = report.get('plan', {})
            if (previous.get('check') == plan.get('check') and report.get('profile') ==
                    ('targeted' if 'check' in plan else 'daily')):
                reports.append((datetime.fromisoformat(report['started_at']).timestamp(), path, report))
        except (OSError, ValueError, TypeError, KeyError):
            continue
    if reports:
        _, path, report = max(reports)
        previous = report.get('plan', {})
        comparable = lambda p: {k: v for k, v in p.items() if k not in {'digest', 'historical_estimate_seconds'}}
        current = comparable(previous) == comparable(plan)
        result.update(status=report['status'] if current else 'stale', report=str(path), run_id=report.get('run_id'))
        if current and report['status'] == 'running':
            result['execution'] = process_state(report.get('process', {}))
            result['heartbeat_at'] = report.get('heartbeat_at')
        if result['status'] == 'passed' and (report.get('source_end') != plan['source'] or
                not report.get('steps') or any(s['status'] not in {'passed', 'cached'} for s in report['steps'])):
            result['status'] = 'stale'
    pending = [c for c in plan['checks'] if c['status'] == 'pending']
    if pending:
        result['status'] = 'unmet-prerequisites'
        result['prerequisites'] = pending
        result['next_command'] = 'git status --short' if plan['source']['dirty'] else 'make verify-plan'
    return result


def display(value, as_json):
    if as_json:
        print(json.dumps(value, indent=2))
    else:
        print(f"Status: {value['status']}")
        for check in value.get('plan', {}).get('checks', []):
            print(f"  {check['group']}: {check['status']}; {'; '.join(check.get('reasons', []))}")
        print(f"Next: {value['next_command']}")


def attempt_status(root, attempt):
    import verification_candidate
    if Path(attempt).name != attempt:
        raise ValueError('Use one retained attempt identity')
    report = json.loads((root / 'target/verification' / attempt / 'report.json').read_text())
    current = report.get('candidate') == verification_candidate.identity(root, report['command'], report['base'])
    result = {'status': report['status'] if current else 'stale', 'run_id': attempt,
              'failed_stages': [s['stage'] for s in report.get('steps', []) if s['status'] == 'failed'],
              'next_command': f"python3 scripts/verification.py recover --attempt {shlex.quote(attempt)} --reason 'Check failed stage'"}
    if report['status'] == 'running':
        result.update(execution=process_state(report['process']), heartbeat_at=report.get('heartbeat_at'))
    if not current or report['status'] in {'passed', 'source-changed', 'incomplete'}:
        result['next_command'] = 'make verify-status'
    return result

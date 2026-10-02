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
import verification_rust_cache


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
            'workers': 'existing-targeted-profile', 'rust_cache': verification_rust_cache.identity(root)}
    import verification_candidate
    plan['execution_inputs_sha256'] = verification_cache.digest(
        verification_candidate.execution_inputs(plan['rust_cache']))
    verification_graph.attach(root, plan, json.loads(policy), files)
    plan['digest'] = verification_cache.digest(plan)
    return plan


def execute(root, check, context):
    import verification as runner
    if os.environ.get('STORYOS_VERIFICATION_RUN'):
        plan = targeted_plan(root, check)
        return runner.step(root, check, plan['command'], node_id="targeted:" + check)
    with verification_cache.budget(root):
        verification_rust_cache.prepare(root)
        plan = targeted_plan(root, check)
        if plan['checks'][0]['status'] == 'pending':
            import verification_candidate
            verification_candidate.observe(root, 'refused', issue=context.get('issue'),
                                           reason='Release package requires clean sources', check=check)
            return 2
        command = [sys.executable, str(Path(runner.__file__).resolve()), 'step', check, '--', *plan['command']]
        return runner.run(root, command, plan=plan, context={**context, 'profile': 'targeted'}, locked=True)


def process_state(process):
    result = subprocess.run(['ps', '-o', 'lstart=', '-p', str(process.get('pid', 0))],
                            capture_output=True, text=True)
    return 'active' if result.returncode == 0 and result.stdout.strip() == process.get('birth') else 'lost'


def status(root, plan):
    next_command = (f"python3 scripts/verification.py targeted --check {shlex.quote(plan['check'])}"
                    if 'check' in plan else f"make verify-changed BASE={shlex.quote(plan['base'])}")
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
        result['retainedStatus'] = report['status']
        result['retainedResultCurrent'] = current
        result['failed_stages'] = [s['stage'] for s in report.get('steps', [])
                                   if current and s['status'] == 'failed']
        result['changedInputs'] = sorted(k for k in comparable(previous).keys() | comparable(plan).keys()
                                         if comparable(previous).get(k) != comparable(plan).get(k))
        if report['status'] == 'running':
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
    observe = (['python3', 'scripts/verification.py', 'status', '--check', plan['check'], '--json']
               if 'check' in plan else ['python3', 'scripts/verification_plan.py', 'status', '--base', plan['base'],
                                        '--workers', str(plan['workers'])])
    return guidance(result, observe, complete=False)


def guidance(result, observe, *, complete):
    action = shlex.split(result['next_command'])
    state = result['status']
    if result.get('execution') == 'active':
        decision, reason, action = 'observe', 'process-active', observe
        hint = 'Observe this run. Wait for it to finish before execution or recovery.'
    elif result.get('execution') == 'lost':
        recoverable = complete and not result.get('changedInputs')
        decision, reason = ('recover' if recoverable else 'blocked'), 'process-lost'
        if not recoverable:
            action = None
        hint = 'Confirm child-process cleanup before recovery or another run. Recovery rechecks admission.'
    elif result.get('changedInputs') or state in {'stale', 'source-changed', 'incomplete'}:
        decision, reason = 'replan', 'identity-changed' if result.get('changedInputs') else 'invalid-evidence'
        plan = result.get('plan', {})
        action = ['make', 'verify-plan', 'BASE=' + plan.get('base', result.get('base', 'origin/main'))]
        hint = 'Inspect a fresh plan and run applicable targeted checks. Refresh candidate reviews after source edits.'
    elif state == 'unmet-prerequisites':
        dirty = result['plan']['source']['dirty']
        decision, reason = 'blocked', 'pending-obligations'
        ready = sum(c['status'] == 'ready' for c in result['plan']['checks'])
        if 'check' in result['plan'] and dirty:
            reason = 'dirty-package-inputs'
        action = (['make', 'verify-changed', 'BASE=' + result['plan']['base']]
                  if ready and 'base' in result['plan'] else None)
        hint = f'{ready} ready checks can run; pending checks remain unsatisfied. Inspect blocking reasons and check details.'
    elif state == 'passed':
        decision, reason, action = 'satisfied', 'current-pass', None
        hint = 'This verification scope passed. This status does not grant merge approval.'
    elif complete:
        decision, reason = 'recover', 'candidate-' + state
        hint = 'Inspect the retained failure and supply its recovery reason. Recovery rechecks admission.'
    else:
        decision, reason = 'run', ('missing-evidence' if state == 'pending' else
                                   ('targeted-' if 'check' in result['plan'] else 'daily-') + state)
        hint = 'Run the selected checks.' if state == 'pending' else 'Correct the failure, then run the selected checks.'
    workers = result.get('plan', {}).get('workers')
    if action and action[:2] in (['make', 'verify-plan'], ['make', 'verify-changed']) and type(workers) is int:
        action.append(f'VERIFY_ARGS=--workers {workers}')
    result.update(version=2, decision=decision, reasonCode=reason,
                  nextAction={'argv': action} if action else None, agentHint=hint,
                  next_command=shlex.join(action) if action else None)
    return result


def display(value, as_json, details=False, *, page=1, selection='all'):
    import verification_summary
    import verification_records
    value['supervision'] = verification_records.supervision(value.get('report'))
    if details:
        print(json.dumps(value, indent=2))
    else:
        verification_summary.display(verification_summary.summary(value, page, selection), as_json)


def attempt_status(root, attempt):
    import verification_candidate
    if Path(attempt).name != attempt:
        raise ValueError('Use one retained attempt identity')
    path = root / 'target/verification' / attempt / 'report.json'
    report = json.loads(path.read_text())
    if report.get('profile') != 'complete':
        raise ValueError('Use --check for targeted status or verification_plan.py status for daily status')
    previous = report.get('candidate', {})
    candidate = verification_candidate.identity(root, report['command'], report['base'])
    changed = sorted(key for key in previous.keys() | candidate.keys() if previous.get(key) != candidate.get(key))
    result = {'status': report['status'] if not changed else 'stale', 'run_id': attempt, 'report': str(path),
              'changedInputs': changed, 'base': report['base'],
              'failed_stages': [s['stage'] for s in report.get('steps', []) if s['status'] == 'failed'],
              'next_command': f"python3 scripts/verification.py recover --attempt {shlex.quote(attempt)} --reason 'Check failed stage'"}
    if report['status'] == 'running':
        result.update(execution=process_state(report['process']), heartbeat_at=report.get('heartbeat_at'))
    return guidance(result, ['python3', 'scripts/verification.py', 'status', '--attempt', attempt, '--json'], complete=True)

"""Admit complete attempts and retain local request and recovery observations."""

from contextlib import ExitStack
from datetime import datetime, timezone
import fcntl
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time
import uuid

import verification_cache


def environment():
    return {key: value for key, value in os.environ.items() if key not in {
        'MAKEFLAGS', 'MFLAGS', 'MAKELEVEL', 'MAKEOVERRIDES', 'MAKE_TERMOUT', 'MAKE_TERMERR',
        'BASE', 'VERIFY_ARGS', 'PR', 'REPORT', 'MANPATH', 'PWD', 'OLDPWD'}}


def identity(root, command, base):
    import verification as runner
    source = runner.source_identity(root)
    tools = []
    chrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
    chrome = chrome if os.access(chrome, os.X_OK) else (shutil.which('google-chrome-stable') or shutil.which('google-chrome'))
    for name in ('git', 'make', 'cargo', 'rustc', 'node', 'pnpm', 'docker', 'chrome', 'python'):
        executable = chrome if name == 'chrome' else (sys.executable if name == 'python' else shutil.which(name))
        tools.append([name, executable, hashlib.sha256(Path(executable).read_bytes()).hexdigest() if executable else None, subprocess.check_output(
            [executable, '--version'], cwd=root, stderr=subprocess.STDOUT, text=True).strip() if executable else None])
    inputs = {key: value for key, value in environment().items()
                   if key not in {'_', 'SHLVL', 'STORYOS_VERIFICATION_RUN', 'STORYOS_VERIFICATION_PARENT', 'PYTHONDONTWRITEBYTECODE'}}
    return {'version': 1, 'source': {key: value for key, value in source.items() if key != 'write_stamps_sha256'},
            'plan': runner.complete_plan(root, base=base), 'command': command, 'tools': tools,
            'inputs': verification_cache.digest(inputs),
            'runners': verification_cache.digest([(p.name, hashlib.sha256(p.read_bytes()).hexdigest())
                                                  for p in sorted(Path(__file__).parent.glob('verification*.py'))]),
            'host': [platform.node(), platform.platform(), sys.version, sys.executable,
                     subprocess.check_output(['ps', '-o', 'lstart=', '-p', '1'], text=True).strip()]}


def observe(root, outcome, *, emit=True, **fields):
    import verification as runner
    directory = root / 'target/verification/requests'
    directory.mkdir(parents=True, exist_ok=True)
    event = {'version': 1, 'id': uuid.uuid4().hex, 'outcome': outcome,
             'utc': datetime.now(timezone.utc).isoformat(), 'monotonic': time.monotonic(), **fields}
    runner.write_json(directory / f"{event['id']}.json", event)
    if emit:
        print(json.dumps(event), flush=True)


def process_identity():
    return {'pid': os.getpid(), 'birth': subprocess.check_output(
        ['ps', '-o', 'lstart=', '-p', str(os.getpid())], text=True).strip(), 'nonce': uuid.uuid4().hex}


def readiness(root, candidate, context):
    """Validate current source, targeted checks, and independent review records."""
    import verification as runner
    if candidate['source']['dirty']:
        raise ValueError('Complete verification requires a clean tracked and untracked worktree')
    runner.inventory(root)
    import verification_reviews
    return verification_reviews.admission(root, context)


def validate_success(root, report):
    import verification_evidence
    source, plan = report['source_start'], report['plan']
    head = report['admission']['request']['candidate']['head'] if report.get('admission') else source['commit']
    packet = {'head': head, 'pr': report.get('pr'), 'base': plan['base'], 'baseline': plan['base'],
              'report': base64.b64encode(gzip.compress(json.dumps(report).encode())).decode()}
    if report.get('admission'):
        packet['admission_version'] = 1
    verification_evidence.check(root, packet, source['commit'], plan['base'], head, plan['base'],
                                policy_review_required=False)


def require_cleanup(active_path):
    if active_path.exists():
        active = json.loads(active_path.read_text())
        if active.get('status') == 'running' and Path(active['report']).exists():
            group = json.loads(Path(active['report']).read_text())['process'].get('child_group')
            if group:
                try:
                    os.killpg(group, 0)
                except ProcessLookupError:
                    return
                raise ValueError('Lost attempt children require process cleanup before admission')


def run(root, command, context):
    import verification as runner
    directory = root / 'target/verification'
    directory.mkdir(parents=True, exist_ok=True)
    try:
        if os.environ.get('STORYOS_VERIFICATION_RUN'):
            parent = json.loads((Path(os.environ['STORYOS_VERIFICATION_RUN']) / 'report.json').read_text())
            observe(root, 'refused', issue=parent.get('issue'), requested_scope=parent.get('profile'),
                    complete_dispatch_attempt=True, reason='A complete verification run cannot be nested')
            return 1
        with ExitStack() as resources:
            with (directory / 'admission.lock').open('a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                candidate = identity(root, command, context['base'])
                active_path = directory / 'active.json'
                try:
                    resources.enter_context(verification_cache.budget(root))
                except ValueError:
                    active = json.loads(active_path.read_text()) if active_path.exists() else {}
                    if (active.get('candidate') == candidate and active.get('status') == 'running'
                            and subprocess.check_output(['ps', '-o', 'lstart=', '-p', str(active['process']['pid'])],
                                                        text=True).strip() == active['process']['birth']):
                        observe(root, 'active', run_id=active['run_id'], report=active['report'])
                        return 0
                    raise
                require_cleanup(active_path)
                admission = readiness(root, candidate, context)
                previous = []
                for path in directory.glob('*/report.json'):
                    report = json.loads(path.read_text())
                    if report.get('candidate') == candidate:
                        previous.append((report['started_monotonic'], path, report))
                retry = None
                if previous:
                    _, path, report = max(previous)
                    receipt_path = path.parent / 'completion.json'
                    receipt = json.loads(receipt_path.read_text()) if receipt_path.exists() else {}
                    valid = receipt.get('sha256') == hashlib.sha256(path.read_bytes()).hexdigest()
                    if valid and report['status'] == 'passed':
                        validate_success(root, report)
                        observe(root, 'reused', report=str(path), run_id=report['run_id'])
                        return 0
                    recovery_path = path.parent / 'recovery.json'
                    retry = json.loads(recovery_path.read_text()) if recovery_path.exists() else None
                    if (not retry or retry['report_sha256'] != hashlib.sha256(path.read_bytes()).hexdigest()
                            or retry['candidate'] != candidate or retry['status'] != 'passed'
                            or retry['source'] != runner.source_identity(root)):
                        failures = [s['stage'] for s in report.get('steps', []) if s['status'] != 'passed']
                        raise ValueError(f"Attempt {report['run_id']} is {report['status']}; targeted boundaries: {failures}; "
                                         f"use verification.py recover --attempt {report['run_id']} --reason <reason>")
                run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-' + uuid.uuid4().hex[:8]
                process = process_identity()
                context = {**context, 'candidate': candidate, 'run_id': run_id, 'record_version': 1,
                           'repository': str(root.resolve()), 'parent': None, 'executor_context': context.get('executor_context') or process['nonce'], 'admission': admission,
                           'requested_scope': 'complete', 'effective_scope': candidate['plan'],
                           'retry_reason': retry['reason'] if retry else None, 'process': process,
                           'started_monotonic': time.monotonic(), 'attempt_started': False}
                runner.write_json(active_path, {'candidate': candidate, 'run_id': run_id, 'status': 'running',
                                               'process': process, 'report': str(directory / run_id / 'report.json')})
                observe(root, 'admitted', run_id=run_id, issue=context.get('issue'), pr=context.get('pr'))
            code = runner.record_run(root, command, context=context)
            path = directory / run_id / 'report.json'
            report = json.loads(path.read_text())
            if code == 0:
                try:
                    if candidate != identity(root, command, context['base']):
                        raise ValueError('Candidate execution inputs changed')
                    validate_success(root, report)
                except (ValueError, KeyError, TypeError) as error:
                    report.update(status='incomplete', error=str(error))
                    runner.write_json(path, report)
                    code = 1
            observe(root, 'run-end', run_id=run_id, status=report['status'])
            runner.write_json(path.parent / 'completion.json', {'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
            runner.write_json(active_path, {'status': 'settled', 'run_id': run_id})
            return code
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        observe(root, 'refused', reason=str(error), issue=context.get('issue'), pr=context.get('pr'))
        print(str(error), file=sys.stderr)
        return 1


def recover(root, attempt, reason):
    import verification as runner
    if not reason.strip() or Path(attempt).name != attempt:
        raise ValueError('Recovery needs an attempt identity and a reason')
    with verification_cache.budget(root):
        path = root / 'target/verification' / attempt / 'report.json'
        report = json.loads(path.read_text())
        candidate = identity(root, report['command'], report['base'])
        if candidate != report['candidate'] or report['status'] in {'passed', 'source-changed', 'incomplete'}:
            raise ValueError('Recovery requires the unchanged failed candidate; correct invalid evidence at its owner')
        admission = readiness(root, candidate, report)
        active_path = root / 'target/verification/active.json'
        require_cleanup(active_path)
        failures = [s for s in report.get('steps', []) if s['status'] == 'failed'
                    and s.get('parent') is None]
        if report['status'] == 'failed' and not failures:
            raise ValueError('No registered targeted boundary; correct the failed source before retry')
        if report['status'] == 'running':
            observe(root, 'lost-process', run_id=attempt, process=report['process'])
        started = runner.source_identity(root)
        recovery = {'version': 1, 'candidate': candidate, 'admission': admission, 'reason': reason, 'status': 'running', 'process': process_identity(),
                    'report_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'source': started,
                    'utc': datetime.now(timezone.utc).isoformat(), 'boundaries': [s['stage'] for s in failures]}
        recovery_path = path.parent / 'recovery.json'
        run_id = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-' + uuid.uuid4().hex[:8]
        command = [sys.executable, str(Path(runner.__file__).resolve()), 'step', 'recovery-check', '--',
                   sys.executable, str(Path(runner.__file__).resolve()), 'recover-steps', '--attempt', attempt]
        runner.write_json(active_path, {'status': 'running', 'report': str(path.parent.parent / run_id / 'report.json')})
        observe(root, 'requested', profile='recovery', run_id=run_id, recovery_of=attempt, issue=report.get('issue'))
        code = runner.record_run(root, command, context={'run_id': run_id, 'profile': 'recovery',
                                 'issue': report.get('issue'), 'pr': report.get('pr'), 'recovery_of': attempt, 'admission': admission,
                                 'retry_reason': reason, 'effective_scope': recovery['boundaries']})
        recovery['run_id'] = run_id
        passed = code == 0 and started == runner.source_identity(root)
        recovery['status'] = 'passed' if passed else 'failed'
        runner.write_json(recovery_path, recovery)
        runner.write_json(active_path, {'status': 'settled'})
        observe(root, 'recovery', recovery_of=attempt, **recovery)
        return 0 if passed else 1


def recovery_steps(root, attempt):
    import verification as runner
    if not os.environ.get('STORYOS_VERIFICATION_RUN') or Path(attempt).name != attempt:
        raise ValueError('Recovery steps require the admitted recovery run')
    report = json.loads((root / 'target/verification' / attempt / 'report.json').read_text())
    commands = json.loads((root / 'docs/agents/verification-policy.json').read_text())['complete'].get('recovery', {})
    for failure in report.get('steps', []):
        if failure['status'] == 'failed' and failure.get('parent') is None:
            code = runner.step(root, failure['stage'], commands.get(failure['stage'], failure['command']))
            if code:
                return code
    if report['status'] == 'infrastructure-failed' and not shutil.which(report['command'][0]):
        return 1
    return 0

"""Bind independent review imports and targeted results to one candidate."""

import argparse
import json
from pathlib import Path
import subprocess
import time

import verification
import verification_cache
import verification_evidence
import verification_status


POLICY = 'docs/agents/verification-policy.json'


def binding(root, revision, base, head, pr, purpose):
    plan = verification.complete_plan(root, revision, base)
    return {'pr': pr, 'head': head, 'base': plan['base'], 'tree': plan['tree'],
            'policy_sha256': plan['policy_sha256'], 'membership': plan['test_files'], 'purpose': purpose,
            'scope': verification.git(root, 'diff', '--name-status', base, revision, '--')}


def sentinel(route, head, base, tree):
    checks = verification_evidence.api(f'{route}/commits/{head}/check-runs?per_page=100')['check_runs']
    checks = [c for c in checks if c['name'] == 'verify' and c['head_sha'] == head]
    check = max(checks, key=lambda c: c['id']) if checks else {}
    if check.get('status') != 'completed' or check.get('conclusion') != 'success':
        raise ValueError('Current synthetic-merge verify success is required')
    logs = subprocess.check_output(['gh', 'api', f"{route}/actions/jobs/{check['id']}/logs", "--allow-escape-sequences"], text=True)
    for label, value in [('Pull request base', base), ('Pull request head', head), ('Synthetic merge tree', tree)]:
        if not any(line.endswith(f'{label}: {value}') for line in logs.splitlines()):
            raise ValueError('Verify sentinel does not cover the current synthetic merge')
    return {'head': head, 'base': base, 'tree': tree, 'check_id': check['id'], 'result': 'PASS'}


def current(root, pr, purpose):
    if not pr or purpose not in {'candidate', 'post-merge-different-tree', 'manual-linux'}:
        raise ValueError('Use a PR and an explicit candidate, post-merge-different-tree, or manual-linux purpose')
    repository = json.loads(subprocess.check_output(['gh', 'repo', 'view', '--json', 'nameWithOwner'], cwd=root))['nameWithOwner']
    route = f'repos/{repository}'
    pull = verification_evidence.api(f'{route}/pulls/{pr}')
    ref = 'refs/storyos/review-candidate'
    verification.git(root, 'fetch', '--no-tags', 'origin', f'+{pull["merge_commit_sha"] if pull.get("merged") else f"refs/pull/{pr}/merge"}:{ref}')
    head, base = pull['head']['sha'], pull['base']['sha']
    if verification.git(root, 'rev-list', '--parents', '-n', '1', ref).split()[1:] != [base, head]:
        raise ValueError('Review candidate must match the PR synthetic merge parents')
    verify = sentinel(route, head, base, verification.git(root, 'rev-parse', f'{ref}^{{tree}}'))
    if purpose == 'candidate' and pull['state'] != 'open':
        raise ValueError('Candidate admission requires an open PR')
    revision = ref
    if purpose == 'post-merge-different-tree':
        if not pull.get('merged'):
            raise ValueError('Post-merge verification requires a merged PR')
        revision = 'HEAD'
        head = verification.git(root, 'rev-parse', revision)
    source = verification.source_identity(root)
    candidate = binding(root, revision, base, head, pr, purpose)
    if source['dirty'] or source['tree'] != candidate['tree']:
        raise ValueError('Review requires a clean source matching the candidate tree')
    return candidate, verify


def validate(request, reviews, expected):
    content = {k: v for k, v in request.items() if k != 'digest'}
    if (request.get('version') != 1 or request['digest'] != verification_cache.digest(content)
            or request['candidate'] != expected or not request.get('executor_context')
            or request['verify']['result'] != 'PASS'
            or (expected['purpose'] == 'candidate' and any(request['verify'][k] != expected[k] for k in ('head', 'base', 'tree')))):
        raise ValueError('Review request is missing, stale, or mismatched')
    if set(reviews) != {'standards', 'spec'}:
        raise ValueError('Current independent Standards and Spec reviews are required')
    contexts = {request['executor_context']}
    for axis, record in reviews.items():
        context = record.get('reviewer_context')
        if (record.get('request_sha256') != request['digest'] or record.get('axis') != axis
                or record.get('result') != 'PASS' or not isinstance(context, str) or not context.strip()
                or context in contexts or not isinstance(record.get('evidence'), str) or not record['evidence'].strip()):
            raise ValueError('Review is failed, stale, incomplete, or not independent')
        contexts.add(context)


def admission(root, context):
    policy = json.loads((root / POLICY).read_text()).get('complete', {}).get('admission')
    protected = json.loads(verification.git(root, 'show', f"{context['base']}:{POLICY}")).get('complete', {}).get('admission')
    if not policy and protected:
        raise ValueError('Protected candidate admission cannot be removed')
    if not policy:
        return None
    if policy.get('version') != 1 or not context.get('review_request'):
        raise ValueError('Current independent candidate reviews are required; use verification_reviews.py request')
    request = json.loads(Path(context['review_request']).read_text())
    expected, verify = current(root, context.get('pr'), context.get('purpose', 'candidate'))
    if (context.get('executor_context') != request['executor_context']
            or verification.git(root, 'rev-parse', context['base']) != expected['base']
            or request['source'] != verification.source_identity(root) or request['verify'] != verify):
        raise ValueError('Review source, executor, base, or verify result changed')
    directory = root / 'target/verification/reviews' / request['digest']
    reviews = {}
    for axis in ('standards', 'spec'):
        records = sorted(directory.glob(f'{axis}-*.json'))
        if records:
            reviews[axis] = json.loads(records[-1].read_text())
    validate(request, reviews, expected)
    targeted = {}
    for check in policy['targeted']:
        state = verification_status.status(root, verification_status.targeted_plan(root, check))
        if state['status'] != 'passed':
            raise ValueError(f"Required targeted result {check} is {state['status']}; {state['next_command']}")
        targeted[check] = json.loads(Path(state['report']).read_text())
    return {'version': 1, 'request': request, 'reviews': reviews, 'targeted': targeted}


def check_report(root, report, revision, base, head, pr):
    candidate_policy = json.loads(verification.git(root, 'show', f'{revision}:{POLICY}'))
    baseline_policy = json.loads(verification.git(root, 'show', f'{base}:{POLICY}'))
    policy = candidate_policy['complete'].get('admission')
    enforced = baseline_policy.get('complete', {}).get('admission')
    record = report.get('admission')
    if not (policy or enforced or record):
        return
    if not policy or not record or policy.get('version') != 1 or record.get('version') != 1:
        raise ValueError('Candidate admission records are required by the protected contract')
    request = record['request']
    expected = binding(root, revision, base, head, pr, report['purpose'])
    validate(request, record['reviews'], expected)
    candidate = report['candidate']
    if (request['source'] != report['source_start'] or request['executor_context'] != report['executor_context']
            or report['pr'] != pr or candidate['plan'] != report['plan']
            or candidate['source'] != {k: v for k, v in report['source_start'].items() if k != 'write_stamps_sha256'}
            or candidate['command'] != report['command'] or set(record['targeted']) != set(policy['targeted'])):
        raise ValueError('Admission and complete report are inconsistent')
    for check, result in record['targeted'].items():
        plan = result['plan']
        if (result['status'] != 'passed' or result['profile'] != 'targeted' or result['exit_code'] != 0
                or result['source_start'] != request['source'] or result['source_end'] != request['source']
                or plan['source'] != request['source'] or plan['check'] != check
                or plan['policy_sha256'] != expected['policy_sha256']
                or plan['command'] != candidate_policy['targeted'][check]['command']
                or plan['execution_inputs_sha256'] != candidate['inputs']
                or plan['test_files'] != sorted(f['path'] for f in report['inventory']['files'] if f['kind'].endswith('-test'))
                or not result['steps'] or any(s['status'] != 'passed' or s['exit_code'] != 0 for s in result['steps'])
                or result['started_monotonic'] + result['duration_seconds'] > report['started_monotonic']):
            raise ValueError('Required targeted evidence is stale, failed, or inconsistent')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    request = sub.add_parser('request')
    request.add_argument('--pr', type=int, required=True)
    request.add_argument('--executor-context', required=True)
    request.add_argument('--purpose', default='candidate')
    entry = sub.add_parser('import')
    entry.add_argument('--request', type=Path, required=True)
    entry.add_argument('--record', type=Path, required=True)
    args = parser.parse_args()
    try:
        root = Path(verification.git(Path.cwd(), 'rev-parse', '--show-toplevel'))
        if args.action == 'request':
            candidate, verify = current(root, args.pr, args.purpose)
            value = {'version': 1, 'candidate': candidate, 'verify': verify,
                     'source': verification.source_identity(root), 'executor_context': args.executor_context}
            value['digest'] = verification_cache.digest(value)
            path = root / 'target/verification/reviews' / value['digest'] / 'request.json'
        else:
            request = json.loads(args.request.read_text())
            value = json.loads(args.record.read_text())
            if (value.get('axis') not in {'standards', 'spec'} or value.get('request_sha256') != request['digest']
                    or value.get('result') not in {'PASS', 'FAIL'}):
                raise ValueError('Import must name this request, review axis, and PASS or FAIL result')
            path = root / 'target/verification/reviews' / request['digest'] / f"{value['axis']}-{time.time_ns()}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        verification.write_json(path, value)
        print(path)
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Review refused: {error}\n')


if __name__ == '__main__':
    main()

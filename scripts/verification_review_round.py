"""Run one complete Standards and Spec review round on a pull request."""

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys

import verification
import verification_github

SCRIPTS = Path(__file__).resolve().parent
TEMPLATE = SCRIPTS.parent / 'docs/agents/review-prompt.md'
VERDICT = re.compile(r'## (Standards|Spec) review, round (\d+): (PASS|FAIL)')
TREE = re.compile(r'^Candidate: head `[0-9a-f]+`, base `[0-9a-f]+`, tree `([0-9a-f]{40})`\.$', re.M)
ROUNDS = 3
BROKER_COMMAND = 'openai-codex/codex/.*/scripts/app-server-broker.mjs'
WAIT_MS = 600000
WAITS = 6
gh = verification_github.gh


def codex_scripts():
    """The scripts directory of the newest installed Codex plugin version."""
    versions = sorted(Path.home().glob('.claude/plugins/cache/openai-codex/codex/*/scripts/codex-companion.mjs'),
                      key=lambda p: [int(n) if n.isdecimal() else 0 for n in re.split(r'[.-]', p.parents[1].name)])
    if not versions:
        raise ValueError('The Codex plugin is not installed; run /codex:setup')
    return versions[-1].parent


def codex(*args):
    """Run one command of the newest installed Codex plugin version and return its JSON."""
    return json.loads(subprocess.check_output(['node', str(codex_scripts() / 'codex-companion.mjs'), *args, '--json'],
                                              text=True))


def stop_codex_broker():
    """Stop the brokers of this workspace and their app servers after the review jobs.

    The plugin teardown of a Claude session end stops the broker that the plugin state records. Two
    review jobs can start two brokers at the same time, and the state keeps only one, so the other
    brokers of this workspace stop by their working directory."""
    stopped = subprocess.run(['node', str(codex_scripts() / 'session-lifecycle-hook.mjs'), 'SessionEnd'],
                             input=json.dumps({'cwd': os.getcwd()}), text=True, capture_output=True)
    if stopped.returncode != 0:
        print(f'Warning: the Codex broker did not stop: exit {stopped.returncode}', file=sys.stderr)
    workspace = os.path.realpath(os.getcwd())
    found = subprocess.run(['pgrep', '-f', BROKER_COMMAND], capture_output=True, text=True).stdout.split()
    parents = [line.split() for line in subprocess.check_output(['ps', '-eo', 'pid=,ppid='], text=True).splitlines()]
    for broker in (int(pid) for pid in found if process_directory(int(pid)) == workspace):
        for pid in [int(child) for child, parent in parents if int(parent) == broker] + [broker]:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass


def process_directory(pid):
    """The working directory of a process, or None when it is not readable."""
    try:
        return os.path.realpath(os.readlink(f'/proc/{pid}/cwd'))
    except OSError:
        listed = subprocess.run(['lsof', '-a', '-p', str(pid), '-d', 'cwd', '-Fn'], capture_output=True, text=True)
        return next((os.path.realpath(line[1:]) for line in listed.stdout.splitlines() if line.startswith('n')), None)


def verify_ready(route, head):
    checks, page = [], 1
    while len(checks) == 100 * (page - 1):
        checks += json.loads(gh('api', f'{route}/commits/{head}/check-runs?filter=all&per_page=100&page={page}'))['check_runs']
        page += 1
    runs = [c for c in checks if c['name'] == 'verify' and c['head_sha'] == head]
    if not any(c.get('status') == 'completed' and c.get('conclusion') == 'success' for c in runs):
        links = ', '.join(c.get('html_url') or str(c['id']) for c in runs) or 'no verify run yet'
        raise ValueError(f'Wait for a successful verify run on the head {head}: {links}')


def contract(pull, directory):
    parts = [f"# Pull request\n\n{pull['body']}"]
    for issue in pull['closingIssuesReferences']:
        value = json.loads(gh('issue', 'view', str(issue['number']), '--json', 'number,title,body'))
        parts.append(f"# Issue #{value['number']}: {value['title']}\n\n{value['body']}")
    (directory / 'contract.md').write_text('\n\n'.join(parts) + '\n')


def review(job):
    """Wait through the plugin status command, then parse the final JSON block of the result."""
    for _ in range(WAITS):
        snapshot = codex('status', job, '--wait', '--timeout-ms', str(WAIT_MS))
        if not snapshot['waitTimedOut']:
            break
    else:
        raise ValueError(f'Codex job {job} did not finish; read it with the plugin status command')
    if snapshot['job']['status'] != 'completed':
        raise ValueError(f"Codex job {job} is {snapshot['job']['status']}")
    stored = codex('result', job)
    output = (stored.get('storedJob') or {}).get('result', {}).get('rawOutput', '')
    blocks = re.findall(r'```json\s*\n(.*?)\n```', output, re.S)
    value = json.loads(blocks[-1] if blocks else output)
    if not all(isinstance(value.get(k), list) and all(isinstance(i, str) for i in value[k])
               for k in ('blocking', 'non_blocking', 'evidence')):
        raise ValueError(f'Codex job {job} did not return the blocking, non_blocking, and evidence lists')
    value['thread'] = stored['storedJob'].get('threadId') or stored['job'].get('threadId')
    return value


def listed(items, numbered=True):
    return '\n'.join(f'{n}. {item}' if numbered else f'- {item}' for n, item in enumerate(items, 1)) or 'None.'


def comment(axis, number, verdict, context, request):
    candidate = request['candidate']
    guards = ', '.join(f'`{k}` {v}' for k, v in request['guards'].items())
    return (f"## {axis.capitalize()} review, round {number}: {verdict['result']}\n\n"
            f"Reviewer: Codex plugin, new read-only thread `{verdict['thread']}` (reviewer context `{context}`). "
            f"Executor context: `{request['executor_context']}`.\n"
            f"Candidate: head `{request['verify']['head']}`, base `{candidate['base']}`, tree `{candidate['tree']}`.\n"
            f"Request digest: `{request['digest']}`. Request `guards`: {guards}.\n\n"
            f"### Blocking\n\n{listed(verdict['blocking'])}\n\n"
            f"### Non-blocking\n\n{listed(verdict['non_blocking'])}\n\n"
            f"### Evidence\n\n{listed(verdict['evidence'], numbered=False)}\n")


def run(root, pr, executor):
    route = 'repos/' + json.loads(gh('repo', 'view', '--json', 'nameWithOwner'))['nameWithOwner']
    pull = json.loads(gh('pr', 'view', str(pr), '--json', 'body,closingIssuesReferences,comments,headRefOid'))
    rounds, trees = {}, {}
    for body in (c['body'] for c in pull['comments']):
        if match := VERDICT.match(body):
            rounds.setdefault(int(match[2]), {})[match[1]] = match[3]
            trees.setdefault(int(match[2]), set()).update(TREE.findall(body))
    # A round counts only when it has a verdict comment for each axis; a retry resumes an incomplete round.
    number = 1 + max((n for n, verdicts in rounds.items() if len(verdicts) == 2), default=0)
    if rounds.get(number - 1) == {'Standards': 'PASS', 'Spec': 'PASS'}:
        # A passed round without a recorded tree, or with the tree of the current synthetic merge, stays final.
        ref = 'refs/storyos/review-candidate'
        verification.git(root, 'fetch', '--no-tags', 'origin', f'+refs/pull/{pr}/merge:{ref}')
        tree = verification.git(root, 'rev-parse', f'{ref}^{{tree}}')
        if not trees[number - 1] or tree in trees[number - 1]:
            raise ValueError(f'Round {number - 1} passed on the two axes and the candidate tree {tree} did not change. '
                             'Only a blocking finding or a changed tree starts a new round; '
                             'send the PR link and the verdict comment links to the coordinator session')
    missing = [axis for axis in ('Standards', 'Spec') if rounds.get(number) and axis not in rounds[number]]
    if missing:
        # The records are already imported; post only the retained comment and do not review again.
        stored = [b for b in root.glob(f'target/verification/reviews/*/{missing[0].lower()}-comment.md')
                  if b.read_text().startswith(f'## {missing[0]} review, round {number}:')]
        if not stored:
            raise ValueError(f'Round {number} has no {missing[0]} comment and no retained verdict; ask the coordinator session')
        print(f'Round {number}: posted the retained {missing[0]} verdict:',
              verification_github.comment(pr, max(stored, key=lambda b: b.stat().st_mtime)).strip())
        return
    if number > ROUNDS:
        raise ValueError(f'PR {pr} had {ROUNDS} review rounds. Send the open findings to the coordinator session; '
                         'it decides the next step')
    verify_ready(route, pull['headRefOid'])
    reviews = SCRIPTS / 'verification_reviews.py'
    path = Path(subprocess.check_output([sys.executable, str(reviews), 'request', '--pr', str(pr),
                                         '--executor-context', executor], text=True, cwd=root).strip())
    request = json.loads(path.read_text())
    contract(pull, path.parent)
    jobs = {}
    try:
        for axis in ('standards', 'spec'):
            prompt = path.parent / f'{axis}-prompt.md'
            prompt.write_text(TEMPLATE.read_text().replace('{{axis}}', axis).replace('{{request}}', str(path)))
            jobs[axis] = codex('task', '--background', '--fresh', '--prompt-file', str(prompt))['jobId']
        verdicts, results = {axis: review(job) for axis, job in jobs.items()}, {}
    finally:
        stop_codex_broker()
    for axis, verdict in verdicts.items():
        verdict['result'], verdict['context'] = 'FAIL' if verdict['blocking'] else 'PASS', f'codex-{axis}-pr{pr}'
        # The admission glob reads <axis>-*.json in this directory, so the local record uses another name.
        record = path.parent / f'record-{axis}.json'
        record.write_text(json.dumps({'request_sha256': request['digest'], 'axis': axis, 'reviewer_context': verdict['context'],
                                      'result': verdict['result'], 'evidence': '\n'.join([f"Codex thread {verdict['thread']}", *verdict['evidence']])}))
        subprocess.check_output([sys.executable, str(reviews), 'import', '--request', str(path), '--record', str(record)], cwd=root)
    for axis, verdict in verdicts.items():
        body = path.parent / f'{axis}-comment.md'
        body.write_text(comment(axis, number, verdict, verdict['context'], request))
        results[axis] = (verdict['result'], verification_github.comment(pr, body).strip())
    print(f'Round {number} of {ROUNDS} for PR {pr}. Request: {path}')
    for axis, (result, url) in results.items():
        print(f'{axis.capitalize()}: {result} {url}')
    if all(result == 'PASS' for result, _ in results.values()):
        print('Next: send the PR link and the two verdict comment links to the coordinator.')
    elif number < ROUNDS:
        print(f'Next: fix the blocking findings, commit, push, wait for verify, and run make review-round PR={pr} again.')
    else:
        print('Next: this was the last round. Send the open findings to the coordinator.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pr', type=int, required=True)
    parser.add_argument('--executor-context')
    args = parser.parse_args()
    try:
        root = Path(verification.git(Path.cwd(), 'rev-parse', '--show-toplevel'))
        run(root, args.pr, args.executor_context or f'claude-pr{args.pr}')
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Review round refused: {error}\n')


if __name__ == '__main__':
    main()

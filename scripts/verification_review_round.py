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
import verification_reviews

SCRIPTS = Path(__file__).resolve().parent
TEMPLATE = SCRIPTS.parent / 'docs/agents/review-prompt.md'
VERDICT = re.compile(r'## (Standards|Spec) review, round (\d+): (PASS|FAIL)')
# A sync or coordinator fix record is not a review round. It adds its tree to the round that it follows.
RECORD = re.compile(r'## (Sync|Coordinator fix) after round (\d+), tree `[0-9a-f]+`: PASS')
CANDIDATE = re.compile(r'^Candidate: head `([0-9a-f]+)`, base `([0-9a-f]+)`, tree `([0-9a-f]{40})`\.$', re.M)
ROUNDS = 3
PASSED = {'Standards': 'PASS', 'Spec': 'PASS'}
LAST_ROUND = ('PR {pr} had {rounds} review rounds. Send the open findings to the coordinator session; it decides the next step. '
              'After a coordinator review of a small fix, record it with KIND=coordinator-fix COMMENT=<comment URL>')
BROKER_COMMAND = 'openai-codex/codex/.*/scripts/app-server-broker.mjs'
WAIT_MS = 600000
WAITS = 6


def gh(*args):
    return verification_github.gh(*args, wait=verification_github.ADMISSION_WAIT)


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
    brokers of this workspace stop by their working directory. A failure writes one warning and never
    stops the round, so the completed verdicts are still imported and posted."""
    try:
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
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f'Warning: the Codex broker did not stop: {type(error).__name__}', file=sys.stderr)


def process_directory(pid):
    """The working directory of a process, or None when it is not readable."""
    try:
        return os.path.realpath(os.readlink(f'/proc/{pid}/cwd'))
    except OSError:
        listed = subprocess.run(['lsof', '-a', '-p', str(pid), '-d', 'cwd', '-Fn'], capture_output=True, text=True)
        return next((os.path.realpath(line[1:]) for line in listed.stdout.splitlines() if line.startswith('n')), None)


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


def candidate_tree(root, pr):
    """Fetch the synthetic merge of the PR and return its parents and tree."""
    ref = 'refs/storyos/review-candidate'
    verification_github.fetch(root, '--no-tags', 'origin', f'+refs/pull/{pr}/merge:{ref}')
    return verification.git(root, 'rev-list', '--parents', '-n', '1', ref).split()[1:], verification.git(root, 'rev-parse', f'{ref}^{{tree}}')


def file_patches(root, old, new):
    """The patch of each file from `old` to `new` without the `index` lines and the hunk line numbers.

    Two equal patches apply the same change. The context lines of each hunk stay, so a change near a hunk makes it different."""
    output = verification.git(root, 'diff', '--no-renames', '--no-color', '--no-ext-diff', '--binary', old, new, '--')
    patches = {}
    for patch in re.split(r'^(?=diff --git )', output, flags=re.M)[1:]:
        lines = [re.sub(r'^@@ .*', '@@', line) for line in patch.splitlines() if not line.startswith('index ')]
        patches[lines[0]] = lines
    return patches


def post_record(root, pr, route, pull, wait, kind, last, trees, heading, lines):
    """Check the synthetic merge and its verify run, then post one record comment for the current tree."""
    head, base = pull['headRefOid'], pull['baseRefOid']
    parents, tree = candidate_tree(root, pr)
    if parents != [base, head]:
        raise ValueError('The synthetic merge of the PR does not have the current base and head as parents; run the command again')
    if tree in trees.get(last, ()):
        raise ValueError(f'Round {last} already records the candidate tree {tree}; send the record links to the coordinator session')
    verification_reviews.wait_verify(route, head, wait)
    verify = verification_reviews.sentinel(route, head, base, tree)
    body = root / 'target/verification/reviews' / f'{kind}-{tree}' / 'comment.md'
    body.parent.mkdir(parents=True, exist_ok=True)
    body.write_text(f'## {heading} after round {last}, tree `{tree[:12]}`: PASS\n\n' + '\n'.join([
        *lines, f'Candidate: head `{head}`, base `{base}`, tree `{tree}`.', f"Verify check run: `{verify['check_id']}`."]) + '\n')
    return verification_github.comment(pr, body).strip()


def sync(root, pr, route, pull, wait, last, trees, anchor):
    """Record a moved main after a passed round when each file has the same change on the branch and on main."""
    if not anchor:
        raise ValueError('Sync needs a passed round or a coordinator fix record with a candidate head and base')
    (old_head, old_base), _ = anchor[0], candidate_tree(root, pr)
    branch, main = file_patches(root, old_head, pull['headRefOid']), file_patches(root, old_base, pull['baseRefOid'])
    if changed := sorted({*branch, *main} - {k for k in branch if branch[k] == main.get(k)}):
        raise ValueError('The change from the round head to the new head is not the same as the change from the old base '
                         f"to the new base in: {', '.join(name.split(' b/', 1)[-1] for name in changed)}. Run a new review round")
    url = post_record(root, pr, route, pull, wait, 'sync', last, trees, 'Sync', [
        f'Round head `{old_head}`, base `{old_base}`.',
        f'The change from the round head to the new head is the same as the change from the old base to the new base. '
        f'Compared files: {len(main)}. The comparison ignores the `index` lines and the hunk line numbers.'])
    print(f'Sync after round {last} for PR {pr}: PASS {url}')
    print(f'Next: run make verify-changed BASE={old_head} and make verify-targeted CHECK=project-scope.')


def coordinator_fix(root, pr, route, pull, wait, last, trees, review_comment, end, candidate):
    """Record a coordinator review comment that names the head as the verdict for a small fix after the last round."""
    if last < ROUNDS:
        raise ValueError(f'A coordinator fix record is only for a fix after {ROUNDS} complete rounds; run a review round')
    found = [n for n, c in enumerate(pull['comments']) if c.get('url') == review_comment]
    if not found:
        raise ValueError(f'{review_comment} is not a comment of PR {pr}')
    if found[0] < end:
        raise ValueError(f'The coordinator review {review_comment} is older than the round {last} verdicts')
    if not any(pull['headRefOid'].startswith(s) for s in re.findall(r'[0-9a-f]{7,40}', pull['comments'][found[0]]['body'])):
        raise ValueError(f"The coordinator review {review_comment} does not name the head {pull['headRefOid']}")
    url = post_record(root, pr, route, pull, wait, 'coordinator-fix', last, trees, 'Coordinator fix',
                      [f'Coordinator review: {review_comment}.'])
    print(f'Coordinator fix after round {last} for PR {pr}: PASS {url}')
    print(f"Next: run make verify-changed BASE={candidate[0] if candidate else f'<round {last} head>'}, "
          'make verify-targeted CHECK=project-scope, and make verify-policy.')


def run(root, pr, executor, wait, kind='review', review_comment=None):
    route = 'repos/' + json.loads(gh('repo', 'view', '--json', 'nameWithOwner'))['nameWithOwner']
    pull = json.loads(gh('pr', 'view', str(pr), '--json', 'body,closingIssuesReferences,comments,headRefOid,baseRefOid'))
    rounds, trees, candidates, records, ends = {}, {}, {}, {}, {}
    for position, body in enumerate(c['body'] for c in pull['comments']):
        if match := VERDICT.match(body):
            rounds.setdefault(int(match[2]), {})[match[1]] = match[3]
            trees.setdefault(int(match[2]), set()).update(c[2] for c in CANDIDATE.findall(body))
            candidates.update({int(match[2]): c[:2] for c in CANDIDATE.findall(body)})
            ends[int(match[2])] = position
        elif match := RECORD.match(body):
            trees.setdefault(int(match[2]), set()).update(c[2] for c in CANDIDATE.findall(body))
            records.setdefault(int(match[2]), []).extend(c[:2] for c in CANDIDATE.findall(body))
    # A round counts only when it has a verdict comment for each axis; a retry resumes an incomplete round.
    number = 1 + max((n for n, verdicts in rounds.items() if len(verdicts) == 2), default=0)
    last = number - 1
    if kind == 'sync':
        # The newest passed record of the last round is the anchor: a sync or coordinator fix, else the passed round.
        passed = rounds.get(last) == PASSED and last in candidates
        return sync(root, pr, route, pull, wait, last, trees, (records.get(last) or [candidates.get(last)] * passed)[-1:])
    if kind == 'coordinator-fix':
        return coordinator_fix(root, pr, route, pull, wait, last, trees, review_comment, ends.get(last, 0), candidates.get(last))
    if rounds.get(last) == PASSED:
        # A passed round without a recorded tree, or with the tree of the current synthetic merge, stays final.
        _, tree = candidate_tree(root, pr)
        if not trees[last] or tree in trees[last]:
            raise ValueError(f'Round {last} passed on the two axes and the candidate tree {tree} did not change. '
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
        raise ValueError(LAST_ROUND.format(pr=pr, rounds=ROUNDS))
    verification_reviews.wait_verify(route, pull['headRefOid'], wait)
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
    parser.add_argument('--wait', type=int, default=0, help='the seconds to wait for the verify run of the head')
    parser.add_argument('--kind', choices=('review', 'sync', 'coordinator-fix'), default='review',
                        help='sync records a moved main after a passed round; coordinator-fix records a coordinator review comment')
    parser.add_argument('--comment', help='the URL of the coordinator review comment for coordinator-fix')
    args = parser.parse_args()
    if (args.kind == 'coordinator-fix') != bool(args.comment):
        parser.error('--comment goes with --kind coordinator-fix only, and coordinator-fix needs it')
    try:
        root = Path(verification.git(Path.cwd(), 'rev-parse', '--show-toplevel'))
        run(root, args.pr, args.executor_context or f'claude-pr{args.pr}', args.wait, args.kind, args.comment)
    except (OSError, ValueError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Review round refused: {error}\n')


if __name__ == '__main__':
    main()

"""Check the bounded network retry of verify-tracker, the tracker query, and git-retry.sh with fake gh and git commands."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import verification_github

SCRIPTS = Path(__file__).parent.resolve()
ISSUES = 'https://github.com/FrankQDWang/StoryOS/issues/'
TLS = 'Get "https://api.github.com/repos/FrankQDWang/StoryOS/issues/100": net/http: TLS handshake timeout'
SSL = "fatal: unable to access 'https://github.com/FrankQDWang/StoryOS/': LibreSSL SSL_connect: SSL_ERROR_SYSCALL"
FAKE = f'''#!{sys.executable}
import json, os, sys
from pathlib import Path
root, args = Path(os.environ["FAKE_ROOT"]), sys.argv[1:]
log = root / "calls.json"
calls = json.loads(log.read_text()) if log.exists() else []
log.write_text(json.dumps(calls + [args]))
fault = json.loads((root / "fault.json").read_text())
if calls.count(args) < fault["failures"]:
    sys.exit(fault["error"])
answers = json.loads((root / "answers.json").read_text())
print(answers.get(args[-1], "ok"))
'''


def issue(number, title, body, parent):
    return {'html_url': ISSUES + str(number), 'title': title, 'body': body,
            'parent_issue_url': 'https://api.github.com/repos/FrankQDWang/StoryOS/issues/' + str(parent)}


def expected(number, title, body, parent):
    return {'issue': ISSUES + str(number), 'parent': ISSUES + str(parent), 'title': title,
            'issue_body_sha256': hashlib.sha256(body.encode()).hexdigest(), 'blocked_by': []}


class NetworkRetryTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        tools = self.root / 'tools'
        tools.mkdir()
        for name in ('gh', 'git'):
            (tools / name).write_text(FAKE)
            (tools / name).chmod(0o755)
        self.environment = {**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH'], 'FAKE_ROOT': str(self.root),
                            'PYTHONDONTWRITEBYTECODE': '1', 'STORYOS_GH_RETRY_SECONDS': '0'}
        self.answer({})

    def answer(self, answers, failures=0, error=''):
        (self.root / 'answers.json').write_text(json.dumps({k: json.dumps(v) for k, v in answers.items()}))
        (self.root / 'fault.json').write_text(json.dumps({'failures': failures, 'error': error}))
        (self.root / 'calls.json').unlink(missing_ok=True)

    def calls(self):
        return json.loads((self.root / 'calls.json').read_text())

    def run_command(self, command, cwd=None):
        return subprocess.run(command, cwd=cwd or self.root, env=self.environment, capture_output=True, text=True)

    def tracker(self, failures, error):
        """Run verify-tracker on a copy with a fixture crosswalk: parent #100, live #101, historical #102."""
        copy = self.root / 'repository'
        (copy / 'scripts').mkdir(parents=True)
        for name in ('verify-stage1-ticket-bindings.py', 'verification_github.py'):
            shutil.copy(SCRIPTS / name, copy / 'scripts' / name)
        crosswalk = copy / 'generated/evidence/stage1-contract-crosswalk.json'
        crosswalk.parent.mkdir(parents=True)
        tickets = [{'responsibility_id': 'LIVE', **expected(101, 'Live', 'live body', 100)},
                   {'responsibility_id': 'HISTORICAL', **expected(102, 'Historical', 'old body', 100)}]
        crosswalk.write_text(json.dumps({'declarations': {
            'delivery_contract': {'parent_specification': expected(100, 'Parent', 'parent body', 1), 'tickets': tickets},
            'tracker_verification': {'historical_reusable_owner_responsibility_ids': ['HISTORICAL']}}}))
        route = 'repos/FrankQDWang/StoryOS/issues/'
        self.answer({route + '100': issue(100, 'Parent', 'parent body', 1), route + '101': issue(101, 'Live', 'live body', 100),
                     route + '100/sub_issues': [{'html_url': ISSUES + '101'}],
                     route + '100/dependencies/blocked_by': [], route + '101/dependencies/blocked_by': []}, failures, error)
        return self.run_command([sys.executable, str(copy / 'scripts/verify-stage1-ticket-bindings.py')])

    def test_verify_tracker_completes_when_each_gh_call_fails_two_times_on_a_tls_timeout(self):
        result = self.tracker(2, TLS)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, 'verified parent #100, 1 live Stage 1 ticket bindings, and 1 historical reusable-owner binding\n')
        calls = self.calls()
        self.assertEqual((len({json.dumps(c) for c in calls}), min(calls.count(c) for c in calls)), (5, 3))
        self.assertIn(f'gh api repos/FrankQDWang/StoryOS/issues/100: transient failure, retry 2 of 6 in 0 s: {TLS}\n', result.stderr)

    def test_verify_tracker_stops_with_the_original_error_of_a_permanent_failure(self):
        for error, attempts in (('HTTP 404: Not Found (https://api.github.com/repos/FrankQDWang/StoryOS/issues/100)', 1),
                                ('HTTP 401: Bad credentials (https://api.github.com/repos/FrankQDWang/StoryOS/issues/100)', 1),
                                (TLS, 7)):
            shutil.rmtree(self.root / 'repository', ignore_errors=True)
            result = self.tracker(99, error)
            self.assertEqual(result.returncode, 1)
            self.assertIn(f'gh api issues/100 failed; stderr:\n{error}\n', result.stderr)
            self.assertEqual(self.calls(), [['api', 'repos/FrankQDWang/StoryOS/issues/100']] * attempts)

    def test_tracker_query_completes_when_each_gh_call_fails_two_times_on_a_tls_timeout(self):
        route = 'repos/FrankQDWang/StoryOS/issues/860'
        answers = {route: {'number': 860, 'title': 'Query ticket', 'state': 'open'},
                   route + '/dependencies/blocked_by?per_page=100&page=1': [{'number': 861, 'title': 'Blocker', 'state': 'closed'}],
                   route + '/dependencies/blocking?per_page=100&page=1': []}
        command = [sys.executable, str(SCRIPTS / 'tracker_query.py'), '860', '--format', 'json']
        self.answer(answers, 2, TLS)
        result = self.run_command(command)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((json.loads(result.stdout)['dependencyState'], min(self.calls().count(c) for c in self.calls())),
                         ('unblocked', 3))
        self.answer(answers, 99, 'HTTP 404: Not Found')
        failed = self.run_command(command)
        self.assertEqual((failed.returncode, json.loads(failed.stdout)['dependencyState'], len(self.calls())), (1, 'unknown', 1))

    def test_git_retry_completes_a_fetch_and_a_pull_after_two_transient_failures(self):
        for error in (SSL, 'fatal: the remote end hung up unexpectedly'):
            for command in (['fetch', '--no-tags', 'origin', 'main'], ['pull', '--ff-only', 'origin', 'main']):
                self.answer({}, 2, error)
                result = self.run_command([str(SCRIPTS / 'git-retry.sh'), *command])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual((result.stdout, self.calls()), ('ok\n', [command] * 3))
                self.assertIn(f'git {" ".join(command[:2])}: transient failure, retry 2 of 6 in 0 s: {error}\n', result.stderr)

    def test_git_retry_stops_a_rejected_push_at_once_and_a_transient_one_after_the_total_wait(self):
        rejected = ' ! [rejected]        main -> main (non-fast-forward)\nerror: failed to push some refs'
        for error, attempts in ((rejected, 1), (SSL, 7)):
            self.answer({}, 99, error)
            result = self.run_command([str(SCRIPTS / 'git-retry.sh'), 'push', 'origin', 'HEAD:main'])
            self.assertEqual(result.returncode, 1)
            self.assertTrue(result.stderr.endswith(error + '\n'), result.stderr)
            self.assertEqual(self.calls(), [['push', 'origin', 'HEAD:main']] * attempts)

    def test_git_retry_refuses_other_git_commands_without_a_call(self):
        for command in (['status'], ['pull', 'origin', 'main'], []):
            result = self.run_command([str(SCRIPTS / 'git-retry.sh'), *command])
            self.assertEqual(result.returncode, 2)
            self.assertIn('git-retry.sh: use ', result.stderr)
            self.assertFalse((self.root / 'calls.json').exists())

    def test_admission_wait_is_sixty_seconds_in_doubling_pauses(self):
        self.assertEqual(verification_github.delays(verification_github.ADMISSION_WAIT), [1, 2, 4, 8, 16, 29])
        self.assertEqual(verification_github.delays(verification_github.WAIT), [1, 2, 4])


if __name__ == '__main__':
    unittest.main()

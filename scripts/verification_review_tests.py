"""Observe review admission before any complete child starts."""

import json
import unittest

import verification_candidate_tests


class ReviewAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.fixture = verification_candidate_tests.CandidateCommandTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.repo, self.root = self.fixture.repo, self.fixture.root
        path = self.root / 'docs/agents/verification-policy.json'
        policy = json.loads(path.read_text())
        policy['complete']['admission'] = {'version': 1, 'targeted': ['cheap']}
        policy['targeted'] = {'cheap': {'command': ['python3', '-c', 'print("checked")'], 'clean': False}}
        path.write_text(json.dumps(policy))
        self.fixture.install_child("from pathlib import Path; Path('target/heavy').touch()")

    def test_missing_reviews_refuse_before_complete_child(self):
        result = self.fixture.run_complete('--pr', '745')
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertFalse((self.root / 'target/heavy').exists())
        self.assertEqual(list(self.root.glob('target/verification/*/report.json')), [])

    def review_cli(self, *args):
        import subprocess
        import sys
        return subprocess.run([sys.executable, str(verification_candidate_tests.verification_tests.COMMAND.with_name('verification_reviews.py')), *args],
                              cwd=self.root, env=self.repo.environment, capture_output=True, text=True)

    def prepare_reviews(self, purpose="candidate"):
        import os
        import sys
        self.base = self.repo.git('rev-parse', 'origin/main')
        self.head = self.repo.git('rev-parse', 'HEAD')
        self.tree = self.repo.git('rev-parse', 'HEAD^{tree}')
        self.repo.git('config', 'user.name', 'Fixture')
        self.repo.git('config', 'user.email', 'fixture@example.invalid')
        self.merge = self.repo.git('commit-tree', self.tree, '-p', self.base, '-p', self.head, '-m', 'Synthetic candidate')
        self.repo.git('update-ref', 'refs/pull/745/merge', self.merge)
        self.repo.git('remote', 'add', 'origin', str(self.root))
        tools = self.root / 'target/tools'
        tools.mkdir(parents=True, exist_ok=True)
        self.live = self.root / 'target/live.json'
        self.live.write_text(json.dumps({'head': {'sha': self.head}, 'base': {'sha': self.base},
                                        'number': 745, 'state': 'open', 'merged': False, 'merge_commit_sha': self.merge}))
        gh = tools / 'gh'
        gh.write_text(f'#!{sys.executable}\nimport json,sys\nfrom pathlib import Path\np=json.loads(Path("target/live.json").read_text())\n'
                      f'print("Pull request base: {self.base}\\nPull request head: {self.head}\\nSynthetic merge tree: {self.tree}") if sys.argv[2].endswith("/logs") else '
                      'print(json.dumps({"nameWithOwner":"fixture/repo"}) if sys.argv[1]=="repo" else '
                      'json.dumps({"check_runs":[{"name":"verify","head_sha":p["head"]["sha"],"id":p.get("check_id",1),"conclusion":"success","status":"completed"}]}) '
                      'if "check-runs" in sys.argv[2] else json.dumps(p))\n')
        gh.chmod(0o755)
        self.repo.environment['PATH'] = str(tools) + os.pathsep + self.repo.environment['PATH']
        result = self.review_cli('request', '--pr', '745', '--executor-context', 'executor', '--purpose', purpose)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.request_path = result.stdout.strip()
        self.request = json.loads(__import__('pathlib').Path(self.request_path).read_text())
        self.assertEqual(self.repo.cli('targeted', '--check', 'cheap').returncode, 0)
        for axis in ('standards', 'spec'):
            record = self.root / f'target/{axis}.json'
            record.write_text(json.dumps({'request_sha256': self.request['digest'], 'axis': axis,
                                          'reviewer_context': axis, 'result': 'PASS', 'evidence': 'Reviewed candidate diff and policy.'}))
            imported = self.review_cli('import', '--request', self.request_path, '--record', str(record))
            self.assertEqual(imported.returncode, 0, imported.stderr)

    def complete(self, purpose="candidate"):
        return self.fixture.run_complete('--pr', '745', '--executor-context', 'executor', '--review-request', self.request_path, '--purpose', purpose)

    def test_current_reviews_admit_and_policy_drift_refuses_before_second_child(self):
        self.prepare_reviews()
        result = self.complete()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / 'target/heavy').exists())
        (self.root / 'target/heavy').unlink()
        policy = self.root / 'docs/agents/verification-policy.json'
        policy.write_text(policy.read_text() + '\n')
        self.fixture.install_child("from pathlib import Path; Path('target/heavy').touch()")
        result = self.complete()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / 'target/heavy').exists())

    def test_latest_failed_or_dependent_review_and_failed_targeted_result_refuse(self):
        self.prepare_reviews()
        record = self.root / 'target/spec.json'
        original = json.loads(record.read_text())
        for change in ({'result': 'FAIL'}, {'reviewer_context': 'executor'}, {'reviewer_context': 'standards'},
                       {'request_sha256': 'stale'}, {'evidence': ''}):
            record.write_text(json.dumps({**original, **change}))
            result = self.review_cli('import', '--request', self.request_path, '--record', str(record))
            if change.get('request_sha256') == 'stale':
                self.assertNotEqual(result.returncode, 0)
            else:
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertNotEqual(self.complete().returncode, 0)
            self.assertFalse((self.root / 'target/heavy').exists())
        record.write_text(json.dumps(original))
        self.assertEqual(self.review_cli('import', '--request', self.request_path, '--record', str(record)).returncode, 0)
        path = next(self.root.glob('target/verification/*/report.json'))
        report = json.loads(path.read_text())
        report['status'] = 'failed'
        path.write_text(json.dumps(report))
        self.assertNotEqual(self.complete().returncode, 0)
        self.assertFalse((self.root / 'target/heavy').exists())

    def test_recovery_requires_current_reviews_and_retains_failed_attempt(self):
        self.fixture.install_child("raise SystemExit(7)")
        self.prepare_reviews()
        self.assertNotEqual(self.complete().returncode, 0)
        path = next(p for p in self.root.glob('target/verification/*/report.json') if json.loads(p.read_text())['profile'] == 'complete')
        original = path.read_bytes()
        record = self.root / 'target/spec.json'
        value = json.loads(record.read_text())
        value['result'] = 'FAIL'
        record.write_text(json.dumps(value))
        self.assertEqual(self.review_cli('import', '--request', self.request_path, '--record', str(record)).returncode, 0)
        result = self.repo.cli('recover', '--attempt', json.loads(original)['run_id'], '--reason', 'Check failed boundary')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Review', result.stderr)
        self.assertEqual(path.read_bytes(), original)
        self.assertFalse(any(json.loads(p.read_text())['profile'] == 'recovery' for p in self.root.glob('target/verification/*/report.json')))
        live = json.loads(self.live.read_text())
        self.live.write_text(json.dumps({**live, 'check_id': 2}))
        value['result'] = 'PASS'
        record.write_text(json.dumps(value))
        self.assertEqual(self.review_cli('import', '--request', self.request_path, '--record', str(record)).returncode, 0)
        self.repo.cli('recover', '--attempt', json.loads(original)['run_id'], '--reason', 'Recheck failed boundary')
        self.assertTrue(any(json.loads(p.read_text())['profile'] == 'recovery' for p in self.root.glob('target/verification/*/report.json')))
        self.assertEqual(path.read_bytes(), original)

    def test_different_merge_tree_gets_a_new_post_merge_request(self):
        self.prepare_reviews()
        (self.root / 'AGENTS.md').write_text('Merged source differs from the verified candidate.\n')
        self.fixture.install_child("print('new candidate')")
        tree = self.repo.git('rev-parse', 'HEAD^{tree}')
        merged = self.repo.git('commit-tree', tree, '-p', self.base, '-p', self.head, '-m', 'Changed merge tree')
        self.live.write_text(json.dumps({'head': {'sha': self.head}, 'base': {'sha': self.base},
                                        'state': 'closed', 'merged': True, 'merge_commit_sha': merged}))
        result = self.review_cli('request', '--pr', '745', '--executor-context', 'executor', '--purpose', 'post-merge-different-tree')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_manual_linux_reviews_survive_checkout_stamps_with_fresh_targeted_results(self):
        self.prepare_reviews('manual-linux')
        (self.root / 'AGENTS.md').touch()
        self.assertEqual(self.repo.cli('targeted', '--check', 'cheap').returncode, 0)
        result = self.complete('manual-linux')
        self.assertEqual(result.returncode, 0, result.stderr)

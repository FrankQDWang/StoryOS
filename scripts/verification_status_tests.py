"""Observe targeted results and read-only status through public commands."""

import json
import sys
import unittest

import verification_tests
import verification_plan_tests


class TargetedStatusTests(unittest.TestCase):
    def setUp(self):
        self.repo = verification_tests.VerificationCommandTests()
        self.repo.setUp()
        self.addCleanup(self.repo.doCleanups)
        self.root = self.repo.root
        policy = self.root / 'docs/agents/verification-policy.json'
        data = json.loads(policy.read_text())
        child = "from pathlib import Path; p=Path('target/launches'); p.write_text(p.read_text()+'x' if p.exists() else 'x'); raise SystemExit(7 if Path('target/fail').exists() else 0)"
        data['targeted'] = {'sample': {'command': [sys.executable, '-c', child], 'clean': False},
                            'package': {'command': [sys.executable, '-c', child], 'clean': True}}
        policy.write_text(json.dumps(data))
        self.repo.git('add', '.')
        self.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                      'commit', '--quiet', '-m', 'Register targeted checks.')

    def status(self, check='sample'):
        result = self.repo.cli('status', '--check', check, '--json')
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_status_is_read_only_and_rejects_stale_or_failed_results(self):
        self.assertEqual(self.status()['status'], 'pending')
        self.assertFalse((self.root / 'target').exists())
        result = self.repo.cli('targeted', '--check', 'sample', '--issue', '744')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.status()['status'], 'passed')
        report = self.repo.report()
        self.assertEqual((report['issue'], report['profile'], report['parent']), (744, 'targeted', None))
        self.assertTrue(report['plan']['policy_sha256'])
        before = {str(p): p.read_bytes() for p in self.root.glob('target/verification/**/*.json')}
        self.assertEqual(self.status()['status'], 'passed')
        self.assertEqual(before, {str(p): p.read_bytes() for p in self.root.glob('target/verification/**/*.json')})
        (self.root / 'AGENTS.md').write_text('Changed source.\n')
        self.assertEqual(self.status()['status'], 'stale')
        (self.root / 'target/fail').touch()
        self.assertNotEqual(self.repo.cli('targeted', '--check', 'sample').returncode, 0)
        self.assertEqual(self.status()['status'], 'failed')
        self.assertEqual((self.root / 'target/launches').read_text(), 'xx')

    def test_dirty_package_refuses_without_start_and_nested_steps_share_one_root(self):
        (self.root / 'AGENTS.md').write_text('Dirty package input.\n')
        self.assertEqual(self.status('package')['status'], 'unmet-prerequisites')
        result = self.repo.cli('targeted', '--check', 'package')
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse((self.root / 'target/launches').exists())
        result = self.repo.cli('step', 'outer', '--', sys.executable, str(verification_tests.COMMAND),
                               'step', 'inner', '--', sys.executable, '-c', 'print("nested")')
        self.assertEqual(result.returncode, 0, result.stderr)
        report = self.repo.report()
        outer, inner = report['steps']
        self.assertEqual((outer['parent'], inner['parent'], report['issue']), (None, outer['id'], None))
        self.assertTrue(report['actual_started_at'])
        self.assertTrue(report['heartbeat_at'])
        self.assertTrue(all(step['started_at'] and step['ended_at'] for step in report['steps']))

    def test_daily_status_and_readable_plan_do_not_launch_children(self):
        fixture = verification_plan_tests.FilePlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        empty = fixture.cli('status')
        self.assertEqual(empty.returncode, 0, empty.stderr)
        self.assertEqual(json.loads(empty.stdout)['status'], 'pending')
        fixture.install_runner_fixture()
        fixture.add_test()
        readable = fixture.cli('plan', '--format', 'text')
        self.assertEqual(readable.returncode, 0, readable.stderr)
        self.assertIn('Next:', readable.stdout)
        self.assertFalse((fixture.root / 'target/verification').exists())
        result = fixture.cli('run', '--issue', '744')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(fixture.cli('status').stdout)['status'], 'passed')
        self.assertEqual(fixture.repo.report()['issue'], 744)


if __name__ == '__main__':
    unittest.main()

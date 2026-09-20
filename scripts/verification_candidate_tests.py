"""Observe complete admission through the public command and real child processes."""

import json
from pathlib import Path
import shutil
import shlex
import os
import signal
import subprocess
import sys
import unittest

import verification_tests


class CandidateCommandTests(unittest.TestCase):
    def setUp(self):
        self.repo = verification_tests.VerificationCommandTests()
        self.repo.setUp()
        self.addCleanup(self.repo.doCleanups)
        self.root = self.repo.root
        policy = self.root / 'docs/agents/verification-policy.json'
        data = json.loads(policy.read_text())
        data['complete'] = {'stages': ['sample'], 'groups': {}}
        data['rules'].append({'pattern': 'Makefile', 'kind': 'verification', 'group': 'complete'})
        policy.write_text(json.dumps(data))
        child = "from pathlib import Path; p=Path('target/launches'); p.write_text(p.read_text()+'x' if p.exists() else 'x')"
        command = [sys.executable, str(verification_tests.COMMAND), 'step', 'sample', '--', sys.executable, '-c', child]
        (self.root / 'Makefile').write_text('verify-local-steps:\n\t@' + shlex.join(command) + '\n')
        self.repo.git('add', '.')
        self.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                      'commit', '--quiet', '-m', 'Add complete fixture.')
        self.repo.git('update-ref', 'refs/remotes/origin/main', 'HEAD')

    def run_complete(self, *args):
        return self.repo.cli('run', *args, '--', 'make', 'verify-local-steps')

    def test_matching_success_reuses_report_without_launch(self):
        first = self.run_complete()
        self.assertEqual(first.returncode, 0, first.stderr)
        second = self.run_complete()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual((self.root / 'target/launches').read_text(), 'x')
        self.assertIn('reused', second.stdout)
        self.assertEqual(self.repo.report()['status'], 'passed')

    def test_running_duplicate_returns_active_attempt_and_interruption_requires_recovery(self):
        child = "import signal; print('ready', flush=True); signal.pause()"
        command = [sys.executable, str(verification_tests.COMMAND), 'step', 'sample', '--', sys.executable, '-c', child]
        (self.root / 'Makefile').write_text('verify-local-steps:\n\t@' + shlex.join(command) + '\n')
        self.repo.git('add', '.')
        self.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                      'commit', '--quiet', '-m', 'Wait for interruption.')
        with subprocess.Popen([sys.executable, str(verification_tests.COMMAND), 'run', '--', 'make', 'verify-local-steps'],
                              cwd=self.root, env=self.repo.environment, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True) as process:
            try:
                while process.stdout.readline().strip() != 'ready':
                    self.assertIsNone(process.poll())
                duplicate = self.run_complete()
                self.assertEqual(duplicate.returncode, 0, duplicate.stderr)
                self.assertIn('active', duplicate.stdout)
            finally:
                process.send_signal(signal.SIGTERM)
                process.communicate(timeout=10)
        report = self.repo.report()
        self.assertEqual(report['status'], 'interrupted')
        self.assertNotEqual(self.run_complete().returncode, 0)
        recovered = self.repo.cli('recover', '--attempt', report['run_id'], '--reason', 'Child cleanup confirmed')
        self.assertEqual(recovered.returncode, 0, recovered.stderr)
        with self.assertRaises(ProcessLookupError):
            os.killpg(report['process']['child_group'], 0)

    def test_failure_requires_successful_targeted_recovery_and_retains_attempts(self):
        child = "from pathlib import Path; p=Path('target/launches'); p.write_text(p.read_text()+'x' if p.exists() else 'x'); raise SystemExit(0 if Path('target/repaired').exists() else 7)"
        command = [sys.executable, str(verification_tests.COMMAND), 'step', 'sample', '--', sys.executable, '-c', child]
        (self.root / 'Makefile').write_text('verify-local-steps:\n\t@' + shlex.join(command) + '\n')
        self.repo.git('add', '.')
        self.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                      'commit', '--quiet', '-m', 'Fail at a targeted boundary.')
        self.assertNotEqual(self.run_complete('--issue', '746', '--pr', '123').returncode, 0)
        report = self.repo.report()
        self.assertEqual((report['issue'], report['pr'], report['purpose']), (746, 123, 'candidate'))
        self.assertNotEqual(self.run_complete().returncode, 0)
        self.assertEqual((self.root / 'target/launches').read_text(), 'x')
        self.assertNotEqual(self.repo.cli('recover', '--attempt', report['run_id'], '--reason', 'Check failure').returncode, 0)
        (self.root / 'target/repaired').touch()
        self.assertEqual(self.repo.cli('recover', '--attempt', report['run_id'], '--reason', 'Fixture restored').returncode, 0)
        retry = self.run_complete()
        self.assertEqual(retry.returncode, 0, retry.stderr)
        reports = [json.loads(p.read_text()) for p in self.root.glob('target/verification/*/report.json')]
        self.assertEqual(sorted(r['status'] for r in reports), ['failed', 'passed'])
        self.assertEqual(next(r for r in reports if r['status'] == 'passed')['retry_reason'], 'Fixture restored')
        self.assertEqual((self.root / 'target/launches').read_text(), 'xxxx')

    def test_dirty_preflight_is_not_an_attempt_and_changed_base_cannot_reuse(self):
        (self.root / 'AGENTS.md').write_text('Changed input')
        self.assertNotEqual(self.run_complete().returncode, 0)
        self.assertEqual(list(self.root.glob('target/verification/*/report.json')), [])
        self.repo.git('checkout', '--', 'AGENTS.md')
        self.assertEqual(self.run_complete().returncode, 0)
        self.repo.git('update-ref', 'refs/remotes/origin/main', 'HEAD~1')
        self.assertEqual(self.run_complete().returncode, 0)
        self.assertEqual((self.root / 'target/launches').read_text(), 'xx')

    def test_corrupt_success_cannot_be_reused_or_trigger_blind_retry(self):
        self.assertEqual(self.run_complete().returncode, 0)
        path = next(self.root.glob('target/verification/*/report.json'))
        report = json.loads(path.read_text())
        report['steps'] = []
        path.write_text(json.dumps(report))
        self.assertNotEqual(self.run_complete().returncode, 0)
        self.assertEqual((self.root / 'target/launches').read_text(), 'x')

    def test_changed_tool_bytes_with_equal_version_invalidate_success(self):
        tools = self.root / 'target/tools'
        tools.mkdir(parents=True)
        tool = tools / 'pnpm'
        tool.write_text('#!/bin/sh\nprintf "fixture-version\\n"\n')
        tool.chmod(0o755)
        self.repo.environment['PATH'] = str(tools) + os.pathsep + self.repo.environment['PATH']
        self.assertEqual(self.run_complete().returncode, 0)
        tool.write_text(tool.read_text() + '# Changed implementation\n')
        self.assertEqual(self.run_complete().returncode, 0)
        self.assertEqual((self.root / 'target/launches').read_text(), 'xx')

    def test_legacy_success_is_retained_without_inventing_candidate_proof(self):
        self.assertEqual(self.run_complete().returncode, 0)
        path = next(self.root.glob('target/verification/*/report.json'))
        report = json.loads(path.read_text())
        del report['candidate']
        path.write_text(json.dumps(report))
        self.assertEqual(self.run_complete().returncode, 0)
        self.assertEqual((self.root / 'target/launches').read_text(), 'xx')
        self.assertEqual(json.loads(path.read_text()), report)

    def test_launch_fault_requires_a_recorded_infrastructure_recovery(self):
        tools = self.root / 'target/tools'
        tools.mkdir(parents=True)
        tool = tools / 'make'
        real_make = shutil.which('make')
        tool.write_text(f"#!{sys.executable}\n" +
                        "import os, pathlib, sys\n" +
                        "if '--version' in sys.argv:\n" +
                        "    print('fixture make')\n" +
                        "    if pathlib.Path('target/disable-launch').exists(): pathlib.Path(sys.argv[0]).chmod(0o644)\n" +
                        f"else: os.execv({real_make!r}, [{real_make!r}, *sys.argv[1:]])\n")
        tool.chmod(0o755)
        marker = self.root / 'target/disable-launch'
        marker.touch()
        self.repo.environment['PATH'] = str(tools) + os.pathsep + self.repo.environment['PATH']
        self.assertNotEqual(self.run_complete().returncode, 0)
        report = self.repo.report()
        self.assertEqual(report['status'], 'infrastructure-failed')
        marker.unlink()
        tool.chmod(0o755)
        self.assertNotEqual(self.run_complete().returncode, 0)
        recovery = self.repo.cli('recover', '--attempt', report['run_id'], '--reason', 'Launch permission restored')
        self.assertEqual(recovery.returncode, 0, recovery.stderr)
        retry = self.run_complete()
        self.assertEqual(retry.returncode, 0, retry.stderr)
        self.assertEqual((self.root / 'target/launches').read_text(), 'x')

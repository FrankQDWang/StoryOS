"""Observe targeted results and read-only status through public commands."""

import json
import signal
import subprocess
import sys
import unittest

import verification_candidate_tests
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
        self.repo.environment['PYTHONDONTWRITEBYTECODE'] = '1'
        result = self.repo.cli('targeted', '--check', 'sample', '--issue', '744')
        self.repo.environment.pop('PYTHONDONTWRITEBYTECODE')
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

    def test_summary_is_bounded_and_details_preserve_large_plan(self):
        fixture = verification_plan_tests.FilePlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        for index in range(180):
            fixture.add_test(f'case_{index}.test.ts')
        before = fixture.repo.git('status', '--porcelain')
        plan = json.loads(fixture.cli('plan').stdout)
        summary = fixture.cli('status')
        value = json.loads(summary.stdout)
        self.assertNotIn('plan', value)
        self.assertLess(len(summary.stdout), 4096)
        text = fixture.cli('plan', '--format', 'text', '--details').stdout
        for check in plan['checks']:
            for reason in check.get('reasons', []):
                self.assertIn(reason, text)
        details = fixture.cli('status', '--details')
        self.assertEqual(details.returncode, 0, details.stderr)
        self.assertEqual(json.loads(details.stdout)['plan'], plan)
        self.assertEqual(fixture.repo.git('status', '--porcelain'), before)
        self.assertFalse((fixture.root / 'target').exists())

    def test_guidance_distinguishes_execution_drift_failure_and_package_block(self):
        self.assertEqual(self.status()['decision'], 'run')
        self.assertEqual(self.repo.cli('targeted', '--check', 'sample').returncode, 0)
        self.assertEqual((self.status()['decision'], self.status()['nextAction']), ('satisfied', None))
        before = {str(p): p.read_bytes() for p in self.root.glob('target/verification/**/*.json')}
        self.repo.environment['STATUS_EXECUTION_INPUT'] = 'private-value'
        value = self.status()
        self.assertEqual((value['decision'], value['reasonCode']), ('replan', 'identity-changed'))
        self.assertIn('execution_inputs_sha256', value['changedInputs'])
        self.assertNotIn('private-value', json.dumps(value))
        text = self.repo.cli('status', '--check', 'sample').stdout
        self.assertIn('execution_inputs_sha256', text)
        self.assertNotIn('private-value', text)
        self.repo.environment.pop('STATUS_EXECUTION_INPUT')
        (self.root / 'AGENTS.md').write_text('Changed input.\n')
        self.assertIn('source', self.status()['changedInputs'])
        self.assertEqual(self.status('package')['reasonCode'], 'dirty-package-inputs')
        self.assertEqual(before, {str(p): p.read_bytes() for p in self.root.glob('target/verification/**/*.json')})
        (self.root / 'target/fail').touch()
        self.assertNotEqual(self.repo.cli('targeted', '--check', 'sample').returncode, 0)
        value = self.status()
        self.assertEqual((value['decision'], value['reasonCode']), ('run', 'targeted-failed'))
        self.assertEqual(value['nextAction']['argv'][-2:], ['--check', 'sample'])
        self.assertEqual((self.root / 'target/launches').read_text(), 'xx')

    def test_active_targeted_run_only_offers_observation_even_after_source_change(self):
        policy = self.root / 'docs/agents/verification-policy.json'
        data = json.loads(policy.read_text())
        data['targeted']['sample']['command'] = [sys.executable, '-c',
            "import signal; print('ready', flush=True); signal.pause()"]
        policy.write_text(json.dumps(data))
        with subprocess.Popen([sys.executable, str(verification_tests.COMMAND), 'targeted', '--check', 'sample'],
                              cwd=self.root, env=self.repo.environment, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True) as process:
            try:
                while process.stdout.readline().strip() != 'ready':
                    self.assertIsNone(process.poll())
                for source_changed in (False, True):
                    if source_changed:
                        (self.root / 'AGENTS.md').write_text('Changed during execution.\n')
                    value = self.status()
                    self.assertEqual((value['execution'], value['decision']), ('active', 'observe'))
                    self.assertEqual(value['nextAction']['argv'],
                                     ['python3', 'scripts/verification.py', 'status', '--check', 'sample', '--json'])
                    self.assertEqual(len(list(self.root.glob('target/verification/*/report.json'))), 1)
            finally:
                process.send_signal(signal.SIGTERM)
                process.communicate()

    def test_make_selector_does_not_invalidate_public_python_status(self):
        runner = verification_tests.COMMAND.resolve()
        (self.root / 'scripts/verification.py').write_text(
            f'import runpy,sys\nsys.path.insert(0, {str(runner.parent)!r})\n'
            f'runpy.run_path({str(runner)!r}, run_name="__main__")\n')
        self.repo.git('add', '.')
        self.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                      'commit', '--quiet', '-m', 'Use the public Make entry.')
        result = subprocess.run(['make', '-f', str(runner.parent.parent / 'Makefile'),
                                 'verify-targeted', 'CHECK=sample'], cwd=self.root,
                                env=self.repo.environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.status()['status'], 'passed')

    def test_dirty_package_refuses_without_start_and_nested_steps_share_one_root(self):
        (self.root / 'AGENTS.md').write_text('Dirty package input.\n')
        self.assertEqual(self.status('package')['status'], 'unmet-prerequisites')
        result = self.repo.cli('targeted', '--check', 'package')
        self.assertEqual((result.returncode, result.stderr), (2,
            'Release packaging requires a clean tracked and untracked worktree. '
            'Commit or remove the dirty paths: AGENTS.md\n'))
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

    def test_new_failure_after_reboot_overrides_old_success(self):
        self.assertEqual(self.repo.cli('targeted', '--check', 'sample').returncode, 0)
        first = next(self.root.glob('target/verification/*/report.json'))
        (self.root / 'target/fail').touch()
        self.assertNotEqual(self.repo.cli('targeted', '--check', 'sample').returncode, 0)
        for path in self.root.glob('target/verification/*/report.json'):
            report = json.loads(path.read_text())
            report.update(started_monotonic=90000 if path == first else 100,
                          started_at='2026-09-19T10:00:00+00:00' if path == first else '2026-09-20T10:00:00+00:00')
            path.write_text(json.dumps(report))
        self.assertEqual(self.status()['status'], 'failed')

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

    def test_daily_prerequisites_name_the_dirty_tree_and_the_stale_policy_result(self):
        fixture = verification_plan_tests.FilePlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.install_runner_fixture()
        policy = json.loads(fixture.policy_path.read_text())
        policy['targeted'] = {'verify-policy': {'command': [sys.executable, '-c', 'pass'], 'clean': False}}
        policy['complete'] = {'stages': ['sample'], 'groups': {'node-contract': ['sample']},
                              'admission': {'version': 1, 'targeted': ['verify-policy']}}
        fixture.policy_path.write_text(json.dumps(policy))
        fixture.repo.git('add', '.')
        fixture.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                         'commit', '--quiet', '-m', 'Require a fresh policy result.')
        fixture.base = fixture.repo.git('rev-parse', 'HEAD')
        path = fixture.add_test()
        path.write_text("import {test} from 'vitest'; test('needs a package',()=>{});\n")
        value = json.loads(fixture.cli('status').stdout)
        stale = {'status': 'unmet', 'reason': 'The verify-policy result is pending. '
                 'Refresh it with make verify-targeted CHECK=verify-policy'}
        self.assertEqual({key: value[key] for key in ('decision', 'reasonCode', 'nextAction', 'prerequisites')}, {
            'decision': 'blocked', 'reasonCode': 'dirty-package-inputs',
            'nextAction': {'argv': ['git', 'status', '--short', '--untracked-files=all'], 'prerequisite': 'cleanTree'},
            'prerequisites': {'cleanTree': {'status': 'unmet', 'reason': 'Release packaging requires a clean tracked '
                                            'and untracked worktree. Commit or remove the dirty paths',
                                            'paths': ['apps/web/test/node-contract/new.test.ts'], 'omittedPaths': 0},
                              'policyFresh': stale}})
        path.write_text("// Verification: repository-inputs-only.\nimport {test} from 'vitest';\ntest('new',()=>{});\n")
        prerequisites = {'cleanTree': {'status': 'not-required', 'reason': 'No selected check requires a clean worktree'},
                         'policyFresh': stale}
        for status in ('pending', 'passed'):
            if status == 'passed':
                self.assertEqual(fixture.cli('run').returncode, 0)
            value = json.loads(fixture.cli('status', '--details').stdout)
            daily = ({'decision': 'run', 'reasonCode': 'missing-evidence', 'nextAction': {'argv': [
                'make', 'verify-changed', 'BASE=' + fixture.base, f"VERIFY_ARGS=--workers {value['plan']['workers']}"]}}
                     if status == 'pending' else {'decision': 'satisfied', 'reasonCode': 'current-pass', 'nextAction': None})
            self.assertEqual({key: value[key] for key in ('status', *daily, 'prerequisites')},
                             {'status': status, **daily, 'prerequisites': prerequisites})
        self.assertEqual(fixture.repo.cli('targeted', '--check', 'verify-policy').returncode, 0)
        value = json.loads(fixture.cli('status').stdout)
        self.assertEqual((value['decision'], value['nextAction'], value['prerequisites']['policyFresh']),
                         ('satisfied', None, {'status': 'met', 'reason': 'The verify-policy result is current and passed'}))

    def test_complete_status_names_the_stale_policy_result(self):
        fixture = verification_candidate_tests.CandidateCommandTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.install_child('raise SystemExit(1)')
        self.assertNotEqual(fixture.run_complete().returncode, 0)
        policy = json.loads((fixture.root / 'docs/agents/verification-policy.json').read_text())
        policy['targeted'] = {'verify-policy': {'command': [sys.executable, '-c', 'pass'], 'clean': False}}
        policy['complete']['admission'] = {'version': 1, 'targeted': ['verify-policy']}
        (fixture.root / 'docs/agents/verification-policy.json').write_text(json.dumps(policy))
        fixture.install_child('raise SystemExit(1)')
        attempt = fixture.repo.report()['run_id']
        value = json.loads(fixture.repo.cli('status', '--attempt', attempt, '--json').stdout)
        self.assertEqual({key: value[key] for key in ('decision', 'reasonCode', 'nextAction', 'prerequisites')}, {
            'decision': 'run', 'reasonCode': 'stale-policy-result',
            'nextAction': {'argv': ['make', 'verify-targeted', 'CHECK=verify-policy'], 'prerequisite': 'policyFresh'},
            'prerequisites': {'cleanTree': {'status': 'not-required',
                                            'reason': 'No selected check requires a clean worktree'},
                              'policyFresh': {'status': 'unmet', 'reason': 'The verify-policy result is pending. '
                                              'Refresh it with make verify-targeted CHECK=verify-policy'}}})
        self.assertEqual(fixture.repo.cli('targeted', '--check', 'verify-policy').returncode, 0)
        value = json.loads(fixture.repo.cli('status', '--attempt', attempt, '--json').stdout)
        self.assertEqual((value['decision'], value['prerequisites']['policyFresh']['status']), ('replan', 'met'))

    def test_size_advisory_reports_the_change_and_large_rust_modules_without_new_guidance(self):
        fixture = verification_plan_tests.FilePlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        commit = ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--quiet', '-m']
        policy = json.loads(fixture.policy_path.read_text())
        policy['rules'].insert(0, {'pattern': 'scripts/*_tests.rs', 'kind': 'rust-test', 'group': 'verification-tools'})
        fixture.policy_path.write_text(json.dumps(policy))
        (fixture.root / 'docs/old.md').write_text('Old line.\n' * 10)
        fixture.repo.git('add', '.')
        fixture.repo.git(*commit, 'Add a file to delete.')
        fixture.base = fixture.repo.git('rev-parse', 'HEAD')
        rust = lambda count: ''.join(f'fn item_{index}() {{}}\n' for index in range(count))
        (fixture.root / 'docs/old.md').unlink()
        (fixture.root / 'AGENTS.md').write_text('Changed source.\nSecond line.\n')
        (fixture.root / 'scripts/large.rs').write_text(rust(400))
        (fixture.root / 'scripts/tested.rs').write_text(
            rust(400) + '#[cfg(test)]\nmod tests {\n' + '    fn case() {}\n' * 148 + '}\n')
        (fixture.root / 'scripts/tested_tests.rs').write_text(rust(600))
        (fixture.root / 'scripts/huge.rs').write_text(rust(801))
        fixture.repo.git('add', '.')
        fixture.repo.git(*commit, 'Change the sources.')
        (fixture.root / 'docs/new.md').write_text('New line.\n' * 3)
        guidance = ('decision', 'reasonCode', 'nextAction')
        small = json.loads(fixture.cli('status').stdout)
        (fixture.root / 'scripts/large.rs').write_text(rust(600))
        fixture.repo.git('add', '.')
        fixture.repo.git(*commit, 'Grow one module.')
        large = json.loads(fixture.cli('status').stdout)
        self.assertEqual({key: small[key] for key in guidance}, {key: large[key] for key in guidance})
        self.assertEqual((small['changeSize'], small['moduleSize']), (
            {'addedAndChanged': 2357, 'deletedFiles': 1, 'deletedFileLines': 10, 'limits': [500, 800],
             'above': [500, 800]},
            [{'path': 'scripts/huge.rs', 'lines': 801, 'above': [500, 800]}]))
        self.assertEqual(large['moduleSize'], [{'path': 'scripts/huge.rs', 'lines': 801, 'above': [500, 800]},
                                               {'path': 'scripts/large.rs', 'lines': 600, 'above': [500]}])
        text = fixture.cli('plan', '--format', 'text')
        self.assertEqual(text.returncode, 0, text.stderr)
        rows = text.stdout.splitlines()
        self.assertEqual([row.split(':')[0] for row in rows[2:6]],
                         ['nextAction', 'changeSize', 'moduleSize', 'omittedModules'])

if __name__ == '__main__':
    unittest.main()

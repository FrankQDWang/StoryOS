"""Check bounded workflow queries through public commands and disposable inputs."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import verification_plan_tests

SCRIPTS = Path(__file__).parent.resolve()


class WorkflowQueryTests(unittest.TestCase):
    def test_large_plan_pages_details_and_saved_execution_contract(self):
        fixture = verification_plan_tests.FilePlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.install_runner_fixture()
        policy = json.loads(fixture.policy_path.read_text())
        policy['daily_consumers'] = [{'pattern': 'docs/fixture.md', 'groups': [
            'contracts', 'node-contract', 'browser-source', 'database', 'node-postgresql',
            'node-process-cut', 'exact-dist', 'recovery', 'pending']}]
        fixture.policy_path.write_text(json.dumps(policy))
        for index in range(250):
            fixture.add_test(f'case_{index}.test.ts')
        (fixture.root / 'docs/fixture.md').write_text('Changed shared input.\n')
        # The production Make recipe reaches the real command from this fixture.
        runner = SCRIPTS / 'verification_plan.py'
        (fixture.root / 'scripts/verification_plan.py').write_text(
            f'import runpy,sys\nsys.path.insert(0,{str(SCRIPTS)!r})\n'
            f'runpy.run_path({str(runner)!r},run_name="__main__")\n')
        before = {str(p.relative_to(fixture.root)): p.read_bytes() for p in fixture.root.rglob('*') if p.is_file()}
        full = fixture.cli('plan')
        self.assertEqual(full.returncode, 0, full.stderr)
        plan = json.loads(full.stdout)
        self.assertGreater(len(full.stdout), 50000)
        for action in ('summary', 'status'):
            for form in ('json', 'text'):
                result = fixture.cli(action, '--format', form)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertLessEqual(len(result.stdout.encode()), 16384)
                self.assertLessEqual(len(result.stdout.splitlines()), 80)
        result = subprocess.run(['make', '-f', str(SCRIPTS.parent / 'Makefile'), 'verify-plan',
                                 'BASE=' + fixture.base], cwd=fixture.root, env=fixture.repo.environment,
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLessEqual(len(result.stdout.encode()), 16384)
        value = json.loads(fixture.cli('summary').stdout)
        self.assertEqual(value['checks'][0]['status'], 'pending')
        self.assertEqual(value['counts'], {'pending': 7, 'ready': 4})
        self.assertEqual(sum(r['checks'] for r in value['blockedReasons']), 7)
        self.assertEqual(value['nextAction']['argv'], ['make', 'verify-changed', 'BASE=' + fixture.base])
        page2 = json.loads(fixture.cli('summary', '--page', '2').stdout)
        self.assertEqual(len(value['checks']) + len(page2['checks']), len(plan['checks']))
        for check in value['checks'] + page2['checks']:
            detail = fixture.cli('summary', '--index', str(check['index']), '--details')
            self.assertEqual(json.loads(detail.stdout), plan['checks'][check['index']])
        blocked = json.loads(fixture.cli('summary', '--select', 'blocked').stdout)
        self.assertEqual(blocked['matchingChecks'], 7)
        self.assertNotEqual(fixture.cli('summary', '--page', '3').returncode, 0)
        self.assertEqual(before, {str(p.relative_to(fixture.root)): p.read_bytes()
                                 for p in fixture.root.rglob('*') if p.is_file()})
        target = fixture.root / 'target'
        target.mkdir()
        records = target / 'verification/retained'
        records.mkdir(parents=True)
        report = {'plan': plan, 'profile': 'daily', 'status': 'failed', 'run_id': 'retained',
                  'started_at': '2026-09-30T00:00:00+00:00',
                  'steps': [{'stage': 'node-contract', 'status': 'failed'}] +
                           [{'stage': '界' * 1000 + str(i), 'status': 'failed'} for i in range(20)]}
        report_path = records / 'report.json'
        report_path.write_text(json.dumps(report))
        report_bytes = report_path.read_bytes()
        failure = fixture.cli('status')
        self.assertLessEqual(len(failure.stdout.encode()), 16384)
        failure_value = json.loads(failure.stdout)
        self.assertEqual(failure_value['checks'][0]['group'], 'node-contract')
        self.assertEqual(failure_value['omittedFailedStages'], 13)
        self.assertEqual(report_bytes, report_path.read_bytes())
        self.assertEqual(len(json.loads(fixture.cli('status', '--details').stdout)['failed_stages']), 21)
        saved = target / 'plan.json'
        saved.write_text(full.stdout)
        fixture.add_test('later.test.ts')
        rejected = fixture.cli('run', '--plan', str(saved))
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn('stale', rejected.stderr)
        self.assertEqual(list(target.glob('verification/*/report.json')), [report_path])

    def test_native_dependencies_cross_pages_fail_closed_and_keep_contract_separate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            gh = root / 'gh'
            gh.write_text(f'#!{sys.executable}\n' + '''import json,pathlib,sys
root=pathlib.Path(__file__).parent
path=sys.argv[-1]
with (root/'calls').open('a') as out: out.write(path+'\\n')
if (root/'fail').exists(): sys.exit(1)
if '/dependencies/' not in path:
 print(json.dumps({'number':860,'title':'Query ticket','state':'open','body':'SECRET BODY'}))
elif '/blocking?' in path: print('[]')
else:
 page=int(path.split('page=')[-1])
 values=[{'number':i+1,'title':'界'*1000,'state':'open' if i==100 else 'closed','body':'SECRET BODY'} for i in range(101)]
 if (root/'unknown').exists(): values[100]['state']='unexpected'
 print(json.dumps(values[(page-1)*100:page*100]))
''')
            gh.chmod(0o755)
            environment = {**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH'],
                           'PYTHONDONTWRITEBYTECODE': '1'}
            command = [sys.executable, str(SCRIPTS / 'tracker_query.py'), '860', '--format', 'json']
            result = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            value = json.loads(result.stdout)
            self.assertEqual((value['dependencyState'], value['openBlockers'], value['counts']['blocked_by']),
                             ('blocked', 1, 101))
            self.assertEqual(value['items'][0]['number'], 101)
            self.assertEqual((value['omitted'], value['pages']), (93, 13))
            self.assertLessEqual(len(result.stdout.encode()), 16384)
            self.assertNotIn('SECRET BODY', result.stdout)
            self.assertIn('title,body,labels,assignees,comments', value['contract'])
            last = subprocess.run(command + ['--page', '13'], cwd=root, env=environment, capture_output=True, text=True)
            self.assertEqual(len(json.loads(last.stdout)['items']), 5)
            self.assertTrue(all('/issues/860' in line for line in (root / 'calls').read_text().splitlines()))
            (root / 'unknown').touch()
            unknown = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
            self.assertEqual(json.loads(unknown.stdout)['dependencyState'], 'unknown')
            (root / 'fail').touch()
            failed = subprocess.run(command, cwd=root, env=environment, capture_output=True, text=True)
            self.assertNotEqual(failed.returncode, 0)
            self.assertEqual(json.loads(failed.stdout)['dependencyState'], 'unknown')

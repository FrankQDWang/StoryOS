"""Exercise the bounded Web stage pair through its public command."""

import json
import os
from pathlib import Path
import select
import shlex
import signal
import subprocess
import sys
import tempfile
import unittest

import verification
import verification_candidate_tests
import verification_tests
import verification_web_overlap


STAGES = ('foundation-tests', 'project-scope')


class WebOverlapTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / 'scripts').mkdir()
        (self.root / 'docs/agents').mkdir(parents=True)
        self.run = self.root / 'target/verification/sample'
        (self.run / 'steps').mkdir(parents=True)
        for stage in STAGES:
            os.mkfifo(self.root / (stage + '.ready'))
            os.mkfifo(self.root / (stage + '.release'))
        (self.root / 'scripts/verification.py').write_text('''
import json, os, signal, sys, time
from pathlib import Path
stage = sys.argv[2]
root = Path.cwd()
run = Path(os.environ['STORYOS_VERIFICATION_RUN'])
started = time.monotonic()
def finish(status, code):
    ended = time.monotonic()
    (run / 'steps' / (stage + '.json')).write_text(json.dumps({
        'stage': stage, 'status': status, 'started_monotonic': started,
        'ended_monotonic': ended, 'duration_seconds': ended - started,
        'attempt_started': True}))
    sys.exit(code)
signal.signal(signal.SIGTERM, lambda *_: finish('interrupted', 143))
with (root / (stage + '.ready')).open('w') as ready:
    ready.write('r')
with (root / (stage + '.release')).open('r') as release:
    release.read(1)
finish('failed' if os.environ.get('FAIL_STAGE') == stage else 'passed',
       7 if os.environ.get('FAIL_STAGE') == stage else 0)
''')
        (self.root / 'Makefile').write_text('''
.PHONY: web-foundation project-scope release-package
web-foundation: release-package
\t@python3 scripts/verification.py step foundation-tests -- true
project-scope: release-package
\t@python3 scripts/verification.py step project-scope -- true
release-package:
\t@exit 91
''')
        self.policy = {'workflow': {'complete_overlap': {
            'prerequisite': 'release-package', 'stages': list(STAGES), 'max_workers': 2,
            'cpu_budget': 2, 'memory_budget_bytes': 2097152,
            'resources': {stage: {'reads': ['release-package'], 'writes': [stage],
                                  'cpu': 1, 'memory_bytes': 1048576} for stage in STAGES}}}}
        self.graph = {'nodes': [{'id': 'check:' + stage, 'selected': True}
                                for stage in (*STAGES, 'release-package')],
                      'dependencies': [{'from': 'check:release-package', 'to': 'check:' + stage}
                                       for stage in STAGES]}
        (self.run / 'steps/release.json').write_text(json.dumps({'stage': 'release-package', 'status': 'passed'}))

    def launch(self, profile='complete', fail=None):
        (self.root / 'docs/agents/verification-policy.json').write_text(json.dumps(self.policy))
        (self.run / 'report.json').write_text(json.dumps({'profile': profile, 'graph': self.graph}))
        environment = {**os.environ, 'STORYOS_VERIFICATION_RUN': str(self.run)}
        if fail:
            environment['FAIL_STAGE'] = fail
        return subprocess.Popen([sys.executable, str(Path(verification_web_overlap.__file__))],
                                cwd=self.root, env=environment, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, text=True)

    def ready(self, stage):
        descriptor = os.open(self.root / (stage + '.ready'), os.O_RDWR | os.O_NONBLOCK)
        self.addCleanup(os.close, descriptor)
        self.assertTrue(select.select([descriptor], [], [], 5)[0], stage)
        self.assertEqual(os.read(descriptor, 1), b'r')

    def release(self, stage):
        descriptor = os.open(self.root / (stage + '.release'), os.O_WRONLY)
        os.write(descriptor, b'r')
        os.close(descriptor)

    def steps(self):
        return {stage: json.loads((self.run / 'steps' / (stage + '.json')).read_text()) for stage in STAGES}

    def test_same_scope_serial_and_overlap_intervals(self):
        serial = self.launch(profile='targeted')
        self.ready('foundation-tests')
        self.release('foundation-tests')
        self.ready('project-scope')
        self.release('project-scope')
        self.assertEqual(serial.communicate(timeout=5)[0].strip(), 'Web stage mode: serial')
        self.assertEqual(serial.returncode, 0)
        before = self.steps()
        self.assertLessEqual(before[STAGES[0]]['ended_monotonic'], before[STAGES[1]]['started_monotonic'])
        for stage in STAGES:
            (self.run / 'steps' / (stage + '.json')).unlink()
        concurrent = self.launch()
        self.ready('foundation-tests')
        self.ready('project-scope')
        self.release('foundation-tests')
        self.release('project-scope')
        self.assertEqual(concurrent.communicate(timeout=5)[0].strip(), 'Web stage mode: bounded overlap')
        self.assertEqual(concurrent.returncode, 0)
        after = self.steps()
        self.assertLess(max(item['started_monotonic'] for item in after.values()),
                        min(item['ended_monotonic'] for item in after.values()))
        self.assertEqual(set(before), set(after))

    def test_failure_records_independent_stage_and_cancellation_stops_both(self):
        failed = self.launch(fail='foundation-tests')
        for stage in STAGES:
            self.ready(stage)
        for stage in STAGES:
            self.release(stage)
        failed.communicate(timeout=5)
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual({stage: item['status'] for stage, item in self.steps().items()},
                         {'foundation-tests': 'failed', 'project-scope': 'passed'})
        for stage in STAGES:
            (self.run / 'steps' / (stage + '.json')).unlink()
        cancelled = self.launch()
        for stage in STAGES:
            self.ready(stage)
        cancelled.send_signal(signal.SIGTERM)
        cancelled.communicate(timeout=5)
        self.assertEqual(cancelled.returncode, 143)
        self.assertEqual({item['status'] for item in self.steps().values()}, {'interrupted'})

    def test_conflicting_resources_or_missing_budget_fall_back_to_serial(self):
        self.policy['workflow']['complete_overlap']['resources']['project-scope']['writes'] = ['release-package']
        stages, concurrent = verification_web_overlap.admission(self.policy, self.graph, 'complete')
        self.assertEqual(stages, list(STAGES))
        self.assertFalse(concurrent)
        self.policy['workflow']['complete_overlap']['resources']['project-scope']['writes'] = ['project-scope']
        self.policy['workflow']['complete_overlap']['cpu_budget'] = 10**6
        self.assertFalse(verification_web_overlap.admission(self.policy, self.graph, 'complete')[1])

    def test_current_complete_graph_keeps_every_mandatory_stage_and_file(self):
        root = Path(__file__).resolve().parent.parent
        plan = verification.complete_plan(root, base='HEAD^', with_graph=True)
        policy = json.loads((root / 'docs/agents/verification-policy.json').read_text())
        selected = {node['id'] for node in plan['graph']['nodes'] if node['selected']}
        self.assertLessEqual({'check:' + stage for stage in policy['complete']['stages']}, selected)
        self.assertEqual({'file:' + item['group'] + ':' + item['path']
                          for item in verification.inventory(root)['files']
                          if item['kind'].endswith('-test')
                          and item['kind'] not in {'historical-test', 'prototype-test'}},
                         {name for name in selected if name.startswith('file:')})
        fixture = verification_candidate_tests.CandidateCommandTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        policy_path = fixture.root / 'docs/agents/verification-policy.json'
        controlled = json.loads(policy_path.read_text())
        controlled['complete']['stages'] = plan['stages']
        policy_path.write_text(json.dumps(controlled))
        runner = [sys.executable, str(verification_tests.COMMAND), 'step']
        commands = [shlex.join([*runner, stage, '--', sys.executable, '-c', 'pass'])
                    for stage in plan['stages']]
        (fixture.root / 'Makefile').write_text('verify-local-steps:\n' + ''.join(
            '\t@' + command + '\n' for command in commands))
        fixture.repo.git('add', '.')
        fixture.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                         'commit', '--quiet', '-m', 'Control every complete stage.')
        fixture.repo.git('update-ref', 'refs/remotes/origin/main', 'HEAD')
        result = fixture.run_complete()
        self.assertEqual(result.returncode, 0, result.stderr)
        report = fixture.repo.report()
        self.assertEqual({step['stage'] for step in report['steps']}, set(plan['stages']))
        self.assertEqual(len(report['steps']), len(plan['stages']))
        self.assertTrue(all(step['status'] == 'passed' and step['attempt_started']
                            and step['started_monotonic'] <= step['ended_monotonic']
                            and step['duration_seconds'] >= 0 for step in report['steps']))
        self.assertLessEqual(max(step['ended_monotonic'] for step in report['steps'])
                             - min(step['started_monotonic'] for step in report['steps']),
                             report['duration_seconds'])


if __name__ == '__main__':
    unittest.main()

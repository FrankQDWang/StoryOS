"""Exercise node recording and replay through managed commands and SQLite."""

import json
import signal
import sqlite3
import subprocess
import sys
import unittest
from pathlib import Path

import verification_graph_tests

COMMAND = Path(__file__).with_name('verification_observation.py')
RUNNER = Path(__file__).with_name('verification.py')


class NodeObservationTests(unittest.TestCase):
    def test_node_attempts_follow_actual_commands_and_replay(self):
        import verification_graph_tests
        fixture = verification_graph_tests.GraphPlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        result = fixture.fixture.cli('run')
        self.assertEqual(result.returncode, 0, result.stderr)
        report = fixture.fixture.repo.report()
        attempts = [s for s in report['steps'] if s.get('node_id')]
        self.assertTrue(attempts)
        for attempt in attempts:
            self.assertEqual(attempt['run_id'], report['run_id'])
            self.assertTrue(attempt['graph_sha256'])
            self.assertTrue(attempt['execution_scope'])
            self.assertTrue(attempt['selection_reason'])
            self.assertTrue(attempt['attempt_started'])
        records = fixture.fixture.root / 'target/verification'
        database = fixture.fixture.root / 'target/read.sqlite'
        command = [sys.executable, str(COMMAND), 'collect', '--records', str(records), '--database', str(database)]
        for _ in range(2):
            self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
        with sqlite3.connect(database) as connection:
            self.assertEqual(connection.execute('SELECT COUNT(*) FROM node_attempts').fetchone(), (len(attempts),))
            self.assertEqual(connection.execute("SELECT state, duration_seconds FROM node_states WHERE path LIKE '%other.test.ts'").fetchone(), ('not-selected', None))
            self.assertEqual(connection.execute("SELECT state, duration_seconds FROM node_states WHERE path LIKE '%first.test.ts'").fetchone(), ('unknown', None))
            before = connection.execute('SELECT * FROM node_attempts ORDER BY attempt_id').fetchall()
        command[2] = 'rebuild'
        self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
        with sqlite3.connect(database) as connection:
            self.assertEqual(connection.execute('SELECT * FROM node_attempts ORDER BY attempt_id').fetchall(), before)

    def test_cargo_build_and_execution_keep_member_timing_unknown(self):
        import verification_plan_tests
        fixture = verification_plan_tests.FilePlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.install_cargo_fixture()
        policy = json.loads(fixture.policy_path.read_text())
        policy['rules'].insert(0, {'pattern': 'crates/*/src/*_tests.rs', 'kind': 'rust-test', 'group': 'cargo'})
        policy['complete'] = {'stages': ['rust-tests'], 'groups': {'cargo': ['rust-tests']}}
        policy['targeted'] = {'rust-tests': {'command': [sys.executable, str(RUNNER), 'rust-tests'], 'clean': False}}
        policy['workflow'] = {'version': 1, 'stage_types': {},
            'operations': {'rust-test-build': {'type': 'build', 'requires': []}},
            'profiles': {'cargo': {'requires': ['rust-test-build']}}, 'targeted': {'rust-tests': ['check:rust-tests']}}
        fixture.policy_path.write_text(json.dumps(policy))
        (fixture.root / 'crates/core/src/lib.rs').write_text('#[cfg(test)] mod value_tests;')
        (fixture.root / 'crates/core/src/value_tests.rs').write_text('#[test] fn value() { assert_eq!(2 + 2, 4); }')
        result = fixture.repo.cli('targeted', '--check', 'rust-tests')
        self.assertEqual(result.returncode, 0, result.stderr)
        report = fixture.repo.report()
        records = list((fixture.root / 'target/verification' / report['run_id'] / 'nodes').glob('*.json'))
        self.assertEqual({json.loads(path.read_text())['node_id'] for path in records}, {'check:rust-test-build', 'check:cargo'})
        self.assertEqual([step['stage'] for step in report['steps']], ['rust-tests'])
        self.assertTrue(any(node.get('path') == 'crates/core/src/value_tests.rs' and node['execution'] == 'member-only'
                            for node in report['plan']['graph']['nodes']))

    def test_failed_prerequisite_blocks_selected_transitive_and_ordered_nodes(self):
        fixture = verification_graph_tests.GraphPlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.policy['workflow']['profiles']['web-typecheck'] = {'requires': ['node-contract']}
        fixture.policy['workflow']['profiles']['contracts'] = {'after': ['web-typecheck']}
        fixture.policy['targeted'] = {'sample': {'command': [sys.executable, str(RUNNER), 'step', 'node-install', '--',
                                                           sys.executable, '-c', 'exit(7)'], 'clean': False}}
        fixture.policy['workflow']['targeted'] = {'sample': ['check:web-typecheck', 'check:contracts']}
        fixture.save()
        self.assertEqual(fixture.fixture.repo.cli('targeted', '--check', 'sample').returncode, 7)
        root = fixture.fixture.root
        database = root / 'target/read.sqlite'
        result = subprocess.run([sys.executable, str(COMMAND), 'collect', '--records', str(root / 'target/verification'),
                                 '--database', str(database)], capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        with sqlite3.connect(database) as connection:
            self.assertEqual(connection.execute("SELECT node_id,state FROM node_states WHERE node_id IN ('check:contracts','check:node-contract','check:web-typecheck') ORDER BY node_id").fetchall(),
                             [('check:contracts', 'blocked'), ('check:node-contract', 'blocked'), ('check:web-typecheck', 'blocked')])

    def test_cached_graph_links_producer_without_node_attempts(self):
        import verification_cache_tests
        fixture = verification_cache_tests.DailyCacheTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        policy = json.loads(fixture.fixture.policy_path.read_text())
        policy['workflow'] = {'version': 1, 'operations': {}, 'targeted': {}, 'stage_types': {},
                              'profiles': {'policy': {}, 'web-typecheck': {}, 'node-contract': {}}}
        fixture.fixture.policy_path.write_text(json.dumps(policy))
        fixture.fixture.repo.git('add', '.')
        fixture.fixture.repo.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
                                'commit', '--quiet', '-m', 'Declare workflow.')
        fixture.fixture.base = fixture.fixture.repo.git('rev-parse', 'HEAD')
        fixture.test.write_text(fixture.test.read_text() + '// changed\n')
        first, producer = fixture.run_daily()
        second, reused = fixture.run_daily()
        self.assertEqual((first.returncode, second.returncode, reused['cache']['status']), (0, 0, 'hit'), second.stderr)
        database = fixture.root / 'target/read.sqlite'
        command = [sys.executable, str(COMMAND), 'collect', '--records', str(fixture.root / 'target/verification'), '--database', str(database)]
        self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
        with sqlite3.connect(database) as connection:
            self.assertEqual(connection.execute('SELECT COUNT(*) FROM node_attempts WHERE run_id=?', (reused['run_id'],)).fetchone(), (0,))
            self.assertEqual(connection.execute('SELECT DISTINCT state,producer FROM node_states WHERE run_id=? AND selected=1', (reused['run_id'],)).fetchall(),
                             [('cached', 'target/verification/' + producer['run_id'] + '/report.json')])

    def test_live_interruption_missing_finish_and_malformed_replay(self):
        fixture = verification_graph_tests.GraphPlanTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        child = [sys.executable, '-c', "import signal; print('ready', flush=True); signal.pause()"]
        fixture.policy['targeted'] = {'sample': {'command': child, 'clean': False}}
        fixture.policy['workflow']['targeted'] = {'sample': ['check:node-contract']}
        fixture.save()
        root = fixture.fixture.root
        database = root / 'target/read.sqlite'
        records = root / 'target/verification'
        collect = [sys.executable, str(COMMAND), 'collect', '--records', str(records), '--database', str(database)]
        with subprocess.Popen([sys.executable, str(RUNNER), 'targeted', '--check', 'sample'],
                cwd=root, env=fixture.fixture.repo.environment, text=True,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE) as process:
            try:
                self.assertEqual(process.stdout.readline(), 'ready\n')
                self.assertEqual(subprocess.run(collect, capture_output=True).returncode, 0)
                with sqlite3.connect(database) as connection:
                    self.assertEqual(connection.execute('SELECT node_id,result,ended_at FROM node_attempts').fetchall(),
                                     [('targeted:sample', 'running', None)])
                process.send_signal(signal.SIGTERM)
                process.communicate(timeout=15)
                self.assertNotEqual(process.returncode, 0)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
        report = fixture.fixture.repo.report()
        step = report['steps'][0]
        self.assertEqual(step['status'], 'interrupted')
        self.assertTrue(step['ended_at'])
        self.assertEqual(subprocess.run(collect, capture_output=True).returncode, 0)
        with sqlite3.connect(database) as connection:
            self.assertEqual(connection.execute('SELECT state FROM node_states WHERE node_id=?', ('targeted:sample',)).fetchone(), ('interrupted',))
            self.assertEqual(connection.execute('SELECT COUNT(*) FROM node_attempts').fetchone(), (1,))
        path = records / report['run_id'] / 'steps' / (step['id'] + '.json')
        step.update(status='running')
        for key in ('ended_at', 'ended_monotonic', 'duration_seconds'):
            step.pop(key)
        path.write_text(json.dumps(step))
        (path.parent / 'duplicate.json').write_text(path.read_text())
        self.assertEqual(subprocess.run(collect, capture_output=True).returncode, 0)
        with sqlite3.connect(database) as connection:
            self.assertEqual(connection.execute('SELECT state,duration_seconds FROM node_states WHERE node_id=?', ('targeted:sample',)).fetchone(), ('unknown', None))
            self.assertEqual(connection.execute("SELECT quality FROM records WHERE path LIKE '%duplicate.json'").fetchone(), ('malformed',))
        for fields in ({'duration_seconds': float('nan')}, {'started_at': []}, {'ended_at': 'not-a-time'}):
            path.write_text(json.dumps({**step, **fields}))
            self.assertEqual(subprocess.run(collect, capture_output=True).returncode, 0)
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute('SELECT COUNT(*) FROM node_attempts').fetchone(), (0,))


if __name__ == '__main__':
    unittest.main()

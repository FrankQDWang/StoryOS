"""Test collection through the public command and its SQLite read interface."""

import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest

import verification_tests


COMMAND = Path(__file__).with_name('verification_observation.py')


class ObservationTests(unittest.TestCase):
    def test_replay_restart_rebuild_and_bad_history_preserve_originals(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            records, database = root / 'records', root / 'read-model.sqlite'
            (records / 'run/steps').mkdir(parents=True)
            report = records / 'run/report.json'
            report.write_text(json.dumps({'record_version': 1, 'run_id': 'run', 'issue': 749,
                'status': 'running', 'profile': 'targeted', 'trigger': 'explicit-request',
                'started_at': '2026-09-21T00:00:00+00:00', 'heartbeat_at': '2026-09-21T00:00:05+00:00',
                'plan': {'checks': [{'group': 'sample', 'reasons': ['changed input']}]}}))
            step = records / 'run/steps/one.json'
            step.write_text(json.dumps({'id': 'one', 'stage': 'sample', 'status': 'running',
                                       'parent': None, 'started_at': '2026-09-21T00:00:01+00:00'}))
            original = {str(p): p.read_bytes() for p in records.rglob('*.json')}
            def collect(action='collect'):
                result = subprocess.run([sys.executable, str(COMMAND), action, '--records', str(records),
                    '--database', str(database)], capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                return json.loads(result.stdout)
            def query(sql):
                with sqlite3.connect(f'file:{database}?mode=ro', uri=True) as connection:
                    return connection.execute(sql).fetchall()
            self.assertEqual(collect()['updated'], 2)
            self.assertEqual(collect()['updated'], 0)
            self.assertEqual(query('SELECT issue, stage, status FROM current_execution'), [(749, 'sample', 'running')])
            self.assertEqual(original, {str(p): p.read_bytes() for p in records.rglob('*.json')})
            report.write_text(json.dumps({'record_version': 1, 'run_id': 'run', 'status': 'failed', 'duration_seconds': 7}))
            self.assertEqual(collect()['updated'], 1)
            self.assertEqual(query('SELECT * FROM current_execution'), [])
            (records / 'legacy').mkdir()
            (records / 'legacy/report.json').write_text('{"status":"passed"}')
            (records / 'broken').mkdir()
            (records / 'broken/report.json').write_text('{')
            collect()
            self.assertEqual(collect()['updated'], 0)
            self.assertEqual(query('SELECT quality, COUNT(*) FROM records GROUP BY quality ORDER BY quality'),
                             [('legacy', 1), ('malformed', 1), ('valid', 2)])
            before = query('SELECT * FROM records ORDER BY path')
            collect('rebuild')
            self.assertEqual(before, query('SELECT * FROM records ORDER BY path'))
            with sqlite3.connect(f'file:{database}?mode=ro', uri=True) as connection:
                with self.assertRaises(sqlite3.OperationalError):
                    connection.execute('DELETE FROM records')


    def test_bounded_backlog_and_complete_scope(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            records = root / 'records'
            (records / 'requests').mkdir(parents=True)
            for index in range(205):
                (records / f'requests/{index}.json').write_text(json.dumps(
                    {'version': 1, 'id': str(index), 'outcome': 'refused', 'reason': 'no review'}))
            (records / 'complete').mkdir()
            (records / 'complete/report.json').write_text(json.dumps({'record_version': 1,
                'run_id': 'complete', 'status': 'running', 'profile': 'complete',
                'plan': {'stages': ['rust-tests'], 'test_files': ['sample.rs']}}))
            (records / 'nonfinite').mkdir()
            (records / 'nonfinite/report.json').write_text(
                '{"record_version":1,"run_id":"nonfinite","status":"running","duration_seconds":NaN}')
            (records / 'linked').symlink_to(root, target_is_directory=True)
            (root / 'report.json').write_text('{"status":"passed"}')
            database = root / 'observation.sqlite'
            result = subprocess.run([sys.executable, str(COMMAND), 'collect', '--records', str(records),
                '--database', str(database)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            batches = [json.loads(line) for line in result.stdout.splitlines()]
            self.assertEqual([batch['updated'] for batch in batches], [200, 8])
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute('SELECT issue, scope FROM current_execution').fetchall(),
                    [(None, '{"stages":["rust-tests"],"test_files":["sample.rs"]}')])
                self.assertEqual(connection.execute("SELECT quality FROM records WHERE path='nonfinite/report.json'").fetchone(),
                                 ('malformed',))


    def test_issue_cost_partitions_overlapping_steps_and_explicit_waits(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            records, database = root / 'records', root / 'read.sqlite'
            (records / 'run/steps').mkdir(parents=True)
            report = {'record_version': 1, 'run_id': 'run', 'issue': 750,
                'profile': 'complete', 'status': 'failed', 'attempt_started': True,
                'started_monotonic': 100, 'duration_seconds': 20,
                'blocked_intervals': [[102, 107], [105, 109]]}
            (records / 'run/report.json').write_text(json.dumps(report))
            for name, parent, start, end in [('outer', None, 101, 119),
                    ('a', 'outer', 103, 110), ('b', 'outer', 108, 115)]:
                (records / f'run/steps/{name}.json').write_text(json.dumps({
                    'id': name, 'parent': parent, 'stage': name, 'status': 'passed',
                    'started_monotonic': start, 'ended_monotonic': end}))
            (records / 'requests').mkdir()
            (records / 'requests/reuse.json').write_text(json.dumps({
                'version': 1, 'id': 'reuse', 'outcome': 'reused', 'run_id': 'run'}))
            result = subprocess.run([sys.executable, str(COMMAND), 'collect', '--records', str(records),
                '--database', str(database)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute('SELECT issue, profile, status, attempts, seconds, blocked_seconds FROM issue_cost').fetchall(),
                                 [(750, 'complete', 'failed', 1, 20.0, 7.0)])
                self.assertEqual(connection.execute('SELECT stage, seconds FROM stage_cost ORDER BY stage').fetchall(),
                                 [('a', 5.0), ('b', 5.0), ('concurrent', 2.0), ('outer', 6.0), ('unclassified', 2.0)])
                self.assertEqual(connection.execute('SELECT issue, outcome, requests FROM request_cost').fetchall(),
                                 [(750, 'reused', 1)])
            report['blocked_clock'] = 'one-boot'
            (records / 'run/report.json').write_text(json.dumps(report))
            (records / 'second').mkdir()
            report.update(run_id='second', blocked_intervals=[[108, 112]])
            (records / 'second/report.json').write_text(json.dumps(report))
            subprocess.run([sys.executable, str(COMMAND), 'collect', '--records', str(records),
                            '--database', str(database)], check=True, capture_output=True)
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute('SELECT attempts,seconds,blocked_seconds FROM issue_cost').fetchall(),
                                 [(2, 40.0, 10.0)])


    def test_rules_distinguish_requests_starts_and_comparable_samples(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            records, database = root / 'records', root / 'read.sqlite'
            (records / 'requests').mkdir(parents=True)
            for index, duration in enumerate([10, 10, 10, 60]):
                run = str(index)
                (records / run).mkdir()
                (records / run / 'report.json').write_text(json.dumps({
                    'record_version': 1, 'run_id': run, 'status': 'passed', 'profile': 'complete',
                    'build_state': 'warm', 'attempt_started': True, 'candidate': {'source': 'same', 'tools': ['locked'], 'inputs': 'inputs', 'runners': 'runners', 'host': ['host'], 'plan': {'policy_sha256': 'policy'}}, 'repository': '/repo',
                    'started_at': f'2026-09-21T00:0{index}:00+00:00', 'started_monotonic': index * 100,
                    'duration_seconds': duration, 'plan': {'checks': []}}))
            (records / 'requests/refused.json').write_text(json.dumps({
                'version': 1, 'id': 'refused', 'outcome': 'refused', 'reason': 'busy'}))
            result = subprocess.run([sys.executable, str(COMMAND), 'collect', '--records', str(records),
                '--database', str(database)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute("SELECT COUNT(*) FROM violations WHERE rule='duplicate-candidate'").fetchone(), (3,))
                self.assertEqual(connection.execute("SELECT COUNT(*) FROM violations WHERE rule='runtime-regression'").fetchone(), (1,))
                self.assertEqual(connection.execute("SELECT samples, state FROM timing_comparison ORDER BY run").fetchall(),
                                 [(0, 'unknown'), (1, 'unknown'), (2, 'unknown'), (3, 'regression')])


    def test_daily_dispatch_resource_overlap_and_stale_runs_survive_downtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            records, database = root / 'records', root / 'read.sqlite'
            for name, scope, start, end in [('daily', 'daily', '00', '10'), ('other', 'targeted', '05', '15')]:
                (records / name / 'steps').mkdir(parents=True)
                (records / name / 'report.json').write_text(json.dumps({'record_version': 1,
                    'run_id': name, 'issue': 750, 'repository': '/same', 'profile': scope,
                    'requested_scope': scope, 'status': 'running', 'attempt_started': True,
                    'actual_started_at': f'2026-09-21T00:00:{start}+00:00',
                    'heartbeat_at': f'2026-09-21T00:00:{end}+00:00'}))
            (records / 'daily/steps/dispatch.json').write_text(json.dumps({'id': 'dispatch',
                'stage': 'complete', 'status': 'failed', 'command': ['make', 'verify-local-steps']}))
            original = {str(p): p.read_bytes() for p in records.rglob('*.json')}
            command = [sys.executable, str(COMMAND), 'collect', '--records', str(records), '--database', str(database)]
            for _ in range(2):
                self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute('SELECT rule, COUNT(*) FROM violations GROUP BY rule ORDER BY rule').fetchall(),
                    [('daily-complete', 1), ('resource-overlap', 1), ('stale-heartbeat', 2)])
                self.assertEqual(connection.execute('SELECT seconds,blocked_seconds FROM issue_cost ORDER BY profile').fetchall(),
                                 [(None, None), (None, None)])
            self.assertEqual(original, {str(p): p.read_bytes() for p in records.rglob('*.json')})


    def test_nested_daily_complete_refusal_is_visible_without_a_child(self):
        repo = verification_tests.VerificationCommandTests()
        repo.setUp()
        self.addCleanup(repo.doCleanups)
        parent = repo.root / 'target/verification/daily'
        parent.mkdir(parents=True)
        (parent / 'report.json').write_text(json.dumps({'profile': 'daily', 'issue': 750}))
        repo.environment['STORYOS_VERIFICATION_RUN'] = str(parent)
        result = repo.cli('run', '--', 'make', 'verify-local-steps')
        self.assertNotEqual(result.returncode, 0)
        events = list(repo.root.glob('target/verification/requests/*.json'))
        self.assertEqual(len(events), 1)
        event = json.loads(events[0].read_text())
        self.assertEqual([event[key] for key in ('outcome', 'issue', 'requested_scope', 'complete_dispatch_attempt')],
                         ['refused', 750, 'daily', True])
        self.assertEqual(list(repo.root.glob('target/verification/*/steps/*.json')), [])


if __name__ == '__main__':
    unittest.main()

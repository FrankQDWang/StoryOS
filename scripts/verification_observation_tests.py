"""Test collection through the public command and its SQLite read interface."""

import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest


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
            (records / 'linked').symlink_to(root, target_is_directory=True)
            (root / 'report.json').write_text('{"status":"passed"}')
            database = root / 'observation.sqlite'
            result = subprocess.run([sys.executable, str(COMMAND), 'collect', '--records', str(records),
                '--database', str(database)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            batches = [json.loads(line) for line in result.stdout.splitlines()]
            self.assertEqual([batch['updated'] for batch in batches], [200, 7])
            with sqlite3.connect(database) as connection:
                self.assertEqual(connection.execute('SELECT issue, scope FROM current_execution').fetchall(),
                    [(None, '{"stages":["rust-tests"],"test_files":["sample.rs"]}')])
                self.assertEqual(connection.execute("SELECT quality FROM records WHERE path='linked/report.json'").fetchone(),
                                 ('malformed',))


if __name__ == '__main__':
    unittest.main()

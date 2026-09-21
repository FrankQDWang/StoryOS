"""Check dashboard queries against retained records through the collector CLI."""

import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class DashboardTests(unittest.TestCase):
    def test_graph_queries_preserve_membership_attempts_and_missing_history(self):
        subprocess.run([sys.executable, str(ROOT / 'scripts/verification_observation_dashboard.py'), '--check'], check=True)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            records = root / 'records'
            (records / 'run/steps').mkdir(parents=True)
            graph = {'version': 1, 'identity': {'source': {'tree': 'sample-tree'}}, 'nodes': [
                {'id': 'setup', 'type': 'setup', 'selected': True},
                {'id': 'group', 'type': 'aggregate', 'selected': True},
                {'id': 'file', 'type': 'test-file', 'path': 'test.py', 'selected': True},
                {'id': 'other', 'type': 'check', 'selected': False}],
                'dependencies': [{'from': 'setup', 'to': 'group'}],
                'relations': [{'from': 'group', 'to': 'file', 'type': 'member'}]}
            (records / 'run/report.json').write_text(json.dumps({'record_version': 1,
                'run_id': 'run', 'status': 'failed', 'profile': 'targeted', 'graph': graph}))
            (records / 'run/steps/attempt.json').write_text(json.dumps({'node_version': 1,
                'run_id': 'run', 'id': 'attempt', 'node_id': 'setup', 'status': 'failed',
                'graph_sha256': hashlib.sha256(json.dumps(graph, sort_keys=True).encode()).hexdigest(),
                'attempt_started': True, 'selection_reason': ['required setup'],
                'execution_scope': {'command': ['false']}, 'duration_seconds': 0,
                'started_at': '2026-09-21T00:00:00+00:00', 'ended_at': '2026-09-21T00:00:00+00:00'}))
            (records / 'legacy').mkdir()
            (records / 'legacy/report.json').write_text('{"status":"passed"}')
            dashboard = json.loads((ROOT / 'scripts/observation/dashboards/run.json').read_text())
            def query(panel, group='__none', run='run'):
                sql = next(p for p in dashboard['panels'] if p['title'] == panel)['targets'][0]['queryText']
                for key, value in {'run': run, 'group': group, 'node': 'setup'}.items():
                    sql = sql.replace('${' + key + ':sqlstring}', "'" + value + "'")
                return connection.execute(sql).fetchall()
            for action in ['collect', 'collect', 'rebuild']:
                subprocess.run([sys.executable, str(ROOT / 'scripts/verification_observation.py'), action,
                    '--records', str(records), '--database', str(root / 'data.sqlite')], check=True, capture_output=True)
                with sqlite3.connect(root / 'data.sqlite') as connection:
                    rows = query('Workflow')
                    self.assertEqual({r[0]: r[2] for r in rows},
                                     {'setup': 'failed', 'group': 'blocked', 'other': 'not-selected'})
                    self.assertEqual({r[0] for r in query('Workflow', group='group')}, {'setup', 'group', 'other', 'file'})
                    self.assertIn('unavailable', str(query('Run', run='legacy')))
                    self.assertIn('sample-tree', str(query('Run')))
                    self.assertIn('required setup', str(query('Node attempts · UTC')))
                    self.assertEqual(query('Workflow', run='legacy'), [])

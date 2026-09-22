"""Check two-run queries through retained-record collection and replay."""

import copy
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.records = self.root / 'records'
        self.graph = {'version': 1, 'nodes': [
            {'id': name, 'type': 'test-file' if name.startswith('file:') else 'check', 'selected': True}
            for name in ['setup', 'tests', 'file:old.py']],
            'dependencies': [{'from': 'setup', 'to': 'tests'}],
            'relations': [{'from': 'tests', 'to': 'file:old.py', 'type': 'member'}]}

    def write_run(self, name, graph, **fields):
        directory = self.records / name
        directory.mkdir(parents=True, exist_ok=True)
        value = {'record_version': 1, 'run_id': name, 'status': 'passed', 'profile': 'daily', **fields}
        if graph is not None:
            value['graph'] = graph
        (directory / 'report.json').write_text(json.dumps(value))

    def query(self, panel, left='left', right='right', action='collect'):
        subprocess.run([sys.executable, str(ROOT / 'scripts/verification_observation.py'), action,
            '--records', str(self.records), '--database', str(self.root / 'data.sqlite')], check=True, capture_output=True)
        dashboard = json.loads((ROOT / 'scripts/observation/dashboards/compare.json').read_text())
        sql = next(p for p in dashboard['panels'] if p['id'] == panel)['targets'][0]['queryText']
        for key, value in {'left': left, 'right': right}.items():
            sql = sql.replace('${' + key + ':sqlstring}', "'" + value + "'")
        with sqlite3.connect(self.root / 'data.sqlite') as connection:
            connection.row_factory = sqlite3.Row
            return [dict(row) for row in connection.execute(sql)]

    def test_snapshot_difference_preserves_removed_files_and_unknown_history(self):
        changed = copy.deepcopy(self.graph)
        changed['nodes'][0]['selected'] = False
        changed['nodes'][2]['id'] = 'file:new.py'
        changed['relations'][0]['to'] = 'file:new.py'
        self.write_run('left', self.graph)
        self.write_run('right', changed, profile='complete')
        self.write_run('legacy', None)
        for action in ['collect', 'collect', 'rebuild']:
            rows = self.query(3, action=action)
            self.assertEqual({r['node_id']: r['difference'] for r in rows},
                {'setup': 'common', 'tests': 'changed-definition', 'file:old.py': 'only-left', 'file:new.py': 'only-right'})
            self.assertEqual({r['node_id']: (r['left_selected'], r['right_selected']) for r in rows if r['node_id']=='setup'},
                {'setup': (1, 0)})
            self.assertEqual({r['difference'] for r in self.query(3, right='legacy')}, {'unavailable'})
            self.assertEqual(self.query(1, right='legacy')[0]['comparison'], 'not comparable')

    def write_attempt(self, run, name, start, end, status='passed', node='setup'):
        directory = self.records / run / 'steps'
        directory.mkdir(exist_ok=True)
        value = {'node_version': 1, 'run_id': run, 'id': name, 'node_id': node,
            'status': status, 'attempt_started': True, 'selection_reason': ['required'],
            'execution_scope': {'group': node}, 'started_at': start, 'ended_at': end,
            'graph_sha256': hashlib.sha256(json.dumps(self.graph, sort_keys=True).encode()).hexdigest()}
        (directory / (name + '.json')).write_text(json.dumps(value))

    def test_actual_intervals_keep_overlap_retries_waits_and_missing_times(self):
        self.write_run('left', self.graph, started_at='2026-09-21T00:00:00+00:00',
            started_monotonic=100, blocked_intervals=[[102, 103]])
        self.write_run('right', self.graph, cache={'status': 'hit', 'producer': 'left/report.json'})
        self.write_attempt('left', 'failed', '2026-09-21T00:00:00+00:00', '2026-09-21T00:00:05+00:00', 'interrupted')
        self.write_attempt('left', 'retry', '2026-09-21T00:00:06+00:00', None, 'running')
        self.write_attempt('left', 'parallel', '2026-09-21T00:00:03+00:00', '2026-09-21T00:00:07+00:00', node='tests')
        rows = self.query(5)
        self.assertEqual({r['attempt_id']: (r['overlaps'], r['result'], r['end_UTC']) for r in rows}, {
            'failed': (1, 'interrupted', '2026-09-21T00:00:05+00:00'),
            'parallel': (1, 'passed', '2026-09-21T00:00:07+00:00'), 'retry': (None, 'running', None)})
        self.assertEqual(self.query(5, right='left')[0]['overlaps'], 1)
        intervals = self.query(4)
        self.assertEqual({r['lane'] for r in intervals}, {'左 · 准备环境 · 1', '左 · 测试 · 1', '左 · 已测量等待 · 0'})
        self.assertEqual([r['end']-r['start'] for r in intervals if '已测量等待' in r['lane']], [1])
        self.assertEqual([(r['selected'], r['executed'], r['reused']) for r in self.query(2)], [(3, 2, 0), (3, 0, 3)])

    def test_duration_comparison_requires_full_evidence(self):
        candidate = {key: {'identity': key} for key in ['tools', 'inputs', 'host', 'runners']}
        candidate['plan'] = {'policy_sha256': 'policy'}
        for name in ['left', 'right']:
            self.write_run(name, self.graph, candidate=candidate, build_state='warm', repository='/sample', duration_seconds=10, attempt_started=True)
            self.write_attempt(name, 'run', '2026-09-21T00:00:00+00:00', '2026-09-21T00:00:05+00:00')
        self.assertEqual(self.query(1)[0]['comparison'], 'comparable evidence')
        for name in ['left', 'right']:
            self.write_run(name, self.graph, candidate=candidate, build_state='warm', duration_seconds=10, attempt_started=True)
        self.assertEqual(self.query(1)[0]['comparison'], 'descriptive only')
        candidate['host'] = {'identity': 'another-host'}
        self.write_run('right', self.graph, candidate=candidate, build_state='warm', duration_seconds=5)
        self.assertEqual((self.query(1)[0]['comparison'], self.query(1)[0]['delta_seconds']), ('descriptive only', -5))
        self.write_run('right', self.graph, candidate=candidate)
        self.assertEqual(self.query(1)[0]['comparison'], 'descriptive only')

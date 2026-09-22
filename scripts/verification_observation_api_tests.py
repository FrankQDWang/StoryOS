"""Verify the read-only HTTP contract against labelled retained fixtures."""

import hashlib
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import selectors
import subprocess
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[1]


class QueryTests(unittest.TestCase):
    def setUp(self):
        output = ROOT / 'target/observation'
        output.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=output)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.records = self.root / 'records'
        self.records.mkdir()
        self.database = self.root / 'runs.sqlite'

    def write(self, name, **fields):
        directory = self.records / name
        directory.mkdir(exist_ok=True)
        (directory / 'report.json').write_text(json.dumps({
            'record_version': 1, 'run_id': name, 'status': 'passed',
            'issue': 773, 'profile': 'targeted', **fields}))

    def start(self, *, missing_database=False, grafana='http://127.0.0.1:1'):
        subprocess.run([sys.executable, str(ROOT / 'scripts/verification_observation.py'),
            'collect', '--records', str(self.records), '--database', str(self.database)],
            check=True, capture_output=True)
        if missing_database:
            self.database.unlink()
        process = subprocess.Popen([sys.executable, '-u', str(ROOT / 'scripts/verification_observation_api.py'),
            '--database', str(self.database), '--port', '0', '--grafana-url', grafana,
            '--collector-health', str(self.root / 'probe-collector.json'), '--health-file', str(self.root / 'probe.json')],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        def stop():
            process.terminate()
            process.communicate(timeout=5)
        self.addCleanup(stop)
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            self.assertTrue(selector.select(5), 'Query service did not become ready')
        line = process.stdout.readline()
        self.assertTrue(line.startswith('http://127.0.0.1:'), line or process.stderr.read())
        self.url = line.strip()

    def get(self, path, **options):
        request = urllib.request.Request(self.url + path, **options)
        try:
            response = urllib.request.urlopen(request, timeout=5)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            return response.status, json.load(response)

    def test_search_pages_all_history_and_preserves_unknown_facts(self):
        for index in range(105):
            self.write(f'fixture-{index:03}', status='running' if index == 0 else 'passed')
        self.start()
        original = {str(p): p.read_bytes() for p in self.records.rglob('*.json')}
        code, page = self.get('/api/v1/runs?limit=100&sort=oldest')
        self.assertEqual((code, page['total'], len(page['items']), page['next_offset']), (200, 105, 100, 100))
        code, last = self.get('/api/v1/runs?limit=100&offset=100&sort=oldest')
        self.assertEqual([r['run'] for r in last['items']], [f'fixture-{i:03}' for i in range(100, 105)])
        code, found = self.get('/api/v1/runs?q=fixture-104')
        self.assertEqual([(r['run'], r['duration_seconds'], r['attempt_started']) for r in found['items']],
                         [('fixture-104', None, None)])
        code, current = self.get('/api/v1/runs?status=running')
        self.assertEqual([r['run'] for r in current['items']], ['fixture-000'])
        self.assertEqual(original, {str(p): p.read_bytes() for p in self.records.rglob('*.json')})

    def test_detail_keeps_group_success_separate_from_file_evidence_and_reuse(self):
        graph = {'version': 1, 'nodes': [
            {'id': 'tests', 'type': 'check', 'selected': True},
            {'id': 'file', 'type': 'test-file', 'path': 'sample.py', 'selected': True}],
            'dependencies': [], 'relations': [{'from': 'tests', 'to': 'file', 'type': 'member'}]}
        self.write('fixture-group', graph=graph)
        steps = self.records / 'fixture-group/steps'
        steps.mkdir()
        (steps / 'group.json').write_text(json.dumps({'node_version': 1, 'id': 'group',
            'run_id': 'fixture-group', 'node_id': 'tests', 'status': 'passed',
            'graph_sha256': hashlib.sha256(json.dumps(graph, sort_keys=True).encode()).hexdigest(),
            'attempt_started': True, 'selection_reason': ['selected group'], 'execution_scope': ['sample.py'],
            'started_at': '2026-09-22T00:00:00Z', 'ended_at': '2026-09-22T00:00:02Z', 'duration_seconds': 2}))
        self.write('fixture-legacy', record_version=0)
        self.write('fixture-interrupted', status='interrupted')
        self.write('fixture-failed', status='failed')
        self.write('fixture-cached', graph=graph, cache={'status': 'hit', 'producer': 'fixture-group'}, attempt_started=False)
        requests = self.records / 'requests'
        requests.mkdir()
        (requests / 'reuse.json').write_text(json.dumps({'version': 1, 'id': 'reuse',
            'outcome': 'reused', 'run_id': 'fixture-group'}))
        self.start()
        code, detail = self.get('/api/v1/runs/fixture-group')
        self.assertEqual((code, detail['has_graph'], detail['evidence']),
                         (200, True, 'fixture-group/report.json'))
        code, files = self.get('/api/v1/runs/fixture-group/files')
        self.assertEqual([(f['path'], f['selected'], f['state'], f['duration_seconds']) for f in files['items']],
                         [('sample.py', 1, 'unknown', None)])
        code, attempts = self.get('/api/v1/runs/fixture-group/attempts')
        self.assertEqual([(a['node_id'], a['result'], a['duration_seconds']) for a in attempts['items']],
                         [('tests', 'passed', 2)])
        code, reuse = self.get('/api/v1/requests?run=fixture-group')
        self.assertEqual([(r['outcome'], r['run_id']) for r in reuse['items']], [('reused', 'fixture-group')])
        code, legacy = self.get('/api/v1/runs/fixture-legacy')
        self.assertEqual((legacy['has_graph'], legacy['record']['issue']), (False, None))
        cached = self.get('/api/v1/runs/fixture-cached/files')[1]
        self.assertEqual([(f['state'], f['duration_seconds'], f['producer']) for f in cached['items']],
                         [('cached', None, 'fixture-group')])
        self.assertEqual(self.get('/api/v1/runs/fixture-cached/attempts')[1]['items'], [])
        states = self.get('/api/v1/runs')[1]['items']
        self.assertEqual({r['run']: r['status'] for r in states}, {
            'fixture-group': 'passed', 'fixture-legacy': 'passed', 'fixture-cached': 'passed',
            'fixture-interrupted': 'interrupted', 'fixture-failed': 'failed'})
        self.assertEqual(self.get('/api/v1/runs/missing')[0], 404)

    def test_empty_unavailable_and_invalid_queries_never_write(self):
        self.start()
        before = self.database.read_bytes()
        self.assertEqual(self.get('/api/v1/runs')[1]['items'], [])
        for path in ('/api/v1/runs?sql=DELETE', '/api/v1/runs?limit=101',
                     '/api/v1/runs?limit=1&limit=2', '/api/v1/runs?sort=invalid',
                     '/api/v1/runs?offset=-1'):
            self.assertEqual(self.get(path)[0], 400, path)
        self.assertEqual(self.get('/api/v1/runs', method='POST', data=b'{}')[0], 405)
        self.assertEqual(self.get('/api/v1/runs', headers={'Origin': 'http://untrusted.invalid'})[0], 403)
        self.assertEqual(self.get('/api/v1/runs/../../etc/passwd')[0], 404)
        self.assertEqual(before, self.database.read_bytes())
        self.database.unlink()
        self.assertEqual(self.get('/api/v1/runs')[0], 503)
        self.assertFalse(self.database.exists())

    def test_health_keeps_stale_collector_and_query_failure_independent(self):
        (self.root / 'probe-collector.json').write_text(json.dumps({
            'checked_at': '2000-01-01T00:00:00+00:00', 'status': 'ok', 'pending': 0, 'records': 7}))
        self.start(missing_database=True)
        code, health = self.get('/api/v1/health')
        self.assertEqual(code, 200)
        self.assertEqual({name: health[name]['status'] for name in ('collector', 'query', 'grafana')},
                         {'collector': 'stale', 'query': 'unavailable', 'grafana': 'unavailable'})
        self.assertEqual(health['collector']['records'], 7)
        self.assertFalse(self.database.exists())
        self.assertTrue((self.root / 'probe.json').is_file())

    def test_grafana_can_be_healthy_when_collection_is_unavailable(self):
        class GrafanaFixture(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"database":"ok"}')
            def log_message(self, *_args):
                pass
        server = HTTPServer(('127.0.0.1', 0), GrafanaFixture)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        def stop():
            server.shutdown()
            thread.join(timeout=3)
            server.server_close()
        self.addCleanup(stop)
        self.start(grafana=f'http://127.0.0.1:{server.server_port}')
        code, health = self.get('/api/v1/health')
        self.assertEqual((code, health['collector']['status'], health['query']['status'], health['grafana']['status']),
                         (200, 'unavailable', 'ok', 'ok'))

    def test_corrupt_collector_numbers_do_not_hide_other_health(self):
        (self.root / 'probe-collector.json').write_text(
            '{"status":"ok","checked_at":"2026-09-22T00:00:00Z","seconds":1e999}')
        self.start()
        code, health = self.get('/api/v1/health')
        self.assertEqual((code, health.get('collector', {}).get('status'), health.get('query', {}).get('status')),
                         (200, 'unavailable', 'ok'))

    def test_overview_cost_counts_only_actual_roots_and_keeps_unknown_duration(self):
        from datetime import datetime, timezone
        now = datetime.now(timezone.utc).isoformat()
        self.write('actual', attempt_started=True, actual_started_at=now, duration_seconds=12)
        self.write('unknown-cost', attempt_started=True, actual_started_at=now, status='interrupted')
        self.write('reused', attempt_started=False, actual_started_at=now, duration_seconds=999)
        self.start()
        code, value = self.get('/api/v1/overview')
        self.assertEqual((code, value.get('starts'), value.get('seconds')), (200, 2, None))

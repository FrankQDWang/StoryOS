"""Check actual Grafana queries against a disposable managed command."""

import http.client
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

import verification_tests
import verification_observation_dashboard_smoke as dag


def main():
    repo = verification_tests.VerificationCommandTests()
    repo.setUp()
    root = Path(__file__).resolve().parents[1]
    compose = ['docker', 'compose', '-p', 'storyos-observation-smoke', '-f',
               str(root / 'scripts/observation/compose.yaml')]
    process = None
    (root / 'target/observation').mkdir(parents=True, exist_ok=True)
    temporary_directory = tempfile.TemporaryDirectory(dir=root / 'target/observation')
    try:
        temporary = temporary_directory.name
        output = Path(temporary)
        (output / 'data').mkdir()
        (output / 'health').mkdir()
        (output / 'grafana').mkdir(mode=0o777)
        (output / 'grafana').chmod(0o777)
        policy = repo.root / 'docs/agents/verification-policy.json'
        value = json.loads(policy.read_text())
        child = "import sys; print('ready', flush=True); sys.stdin.read(1)"
        value['targeted'] = {'sample': {'command': [sys.executable, '-c', child], 'clean': False}}
        policy.write_text(json.dumps(value))
        process = subprocess.Popen([sys.executable, str(verification_tests.COMMAND), 'targeted',
            '--check', 'sample', '--issue', '750'], cwd=repo.root, env=repo.environment,
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL)
        records = repo.root / 'target/verification'
        deadline = time.monotonic() + 120
        while not list(records.glob('*/steps/*.json')):
            if time.monotonic() > deadline or process.poll() is not None:
                raise RuntimeError('Synthetic managed command did not start')
            time.sleep(0.1)
        for index in range(1000):
            directory = records / f'synthetic-{index:04}'
            directory.mkdir()
            (directory / 'report.json').write_text(json.dumps({'record_version': 1,
                'run_id': directory.name, 'status': 'interrupted', 'profile': 'targeted',
                'attempt_started': True, 'started_monotonic': 100, 'duration_seconds': 10,
                'execution_scope': {'synthetic_sort_payload': 'x' * 4096},
                'started_at': '2026-09-21T00:00:00+00:00'}))
        dag.prepare(records)
        measured = subprocess.check_output([sys.executable, str(root / 'scripts/verification_observation.py'),
            'collect', '--records', str(records), '--database', str(output / 'data/runs.sqlite')], text=True)
        overhead = [json.loads(line) for line in measured.splitlines()]
        changed = records / 'synthetic-0000/report.json'
        refreshed = json.loads(changed.read_text())
        refreshed['status'] = 'passed'
        changed.write_text(json.dumps(refreshed))
        override = output / 'compose.yaml'
        override.write_text('services:\n  collector:\n    volumes:\n'
            f'      - {records}:/records:ro\n      - {output}/data:/observation\n'
            '  grafana:\n    ports: !override ["127.0.0.1::3000"]\n    volumes:\n'
            f'      - {output}/data:/observation:ro\n'
            f'      - {output}/grafana:/var/lib/storyos-grafana\n'
            '  query:\n    ports: !override ["127.0.0.1::3754"]\n    volumes:\n'
            f'      - {output}/data:/observation:ro\n      - {output}/health:/health\n')
        compose += ['-f', str(override)]
        subprocess.run([*compose, 'up', '-d', '--build'], check=True, timeout=180)
        port = subprocess.check_output([*compose, 'port', 'grafana', '3000'], text=True).strip()
        url = 'http://' + port
        query_port = subprocess.check_output([*compose, 'port', 'query', '3754'], text=True).strip()
        dashboard = json.loads((root / 'scripts/observation/dashboards/live.json').read_text())
        deadline = time.monotonic() + 90
        refresh_started = time.monotonic()
        while True:
            try:
                request = urllib.request.Request('http://' + query_port + '/api/v1/runs?q=synthetic-0000',
                                                 headers={'Host': '127.0.0.1:3754'})
                with urllib.request.urlopen(request, timeout=8) as response:
                    roots = json.load(response)['items']
                if len(roots) != 1 or roots[0]['status'] != 'passed':
                    raise RuntimeError('Query service has not observed the retained update')
                request = urllib.request.Request('http://' + query_port + '/api/v1/health',
                                                 headers={'Host': '127.0.0.1:3754'})
                with urllib.request.urlopen(request, timeout=10) as response:
                    health = json.load(response)
                if any(health[name]['status'] != 'ok' for name in ('collector', 'query', 'grafana')):
                    raise RuntimeError('Independent health probes are not ready')
                with urllib.request.urlopen(url + '/api/dashboards/uid/storyos-verification', timeout=3) as response:
                    assert json.load(response)['dashboard']['title'] == dashboard['title']
                frames = []
                for panel in dashboard['panels']:
                    query = {**panel['targets'][0], 'datasource': panel['datasource']}
                    request = urllib.request.Request(url + '/api/ds/query', data=json.dumps({'queries': [query]}).encode(),
                                                     headers={'Content-Type': 'application/json'})
                    with urllib.request.urlopen(request, timeout=5) as response:
                        result = json.load(response)['results']['A']
                    if result.get('error'):
                        raise RuntimeError(result['error'])
                    frames.extend(result['frames'])
                sql = "SELECT run FROM observed_runs WHERE run='synthetic-0000' AND status='passed'"
                query = {**dashboard['panels'][0]['targets'][0], 'queryText': sql,
                         'rawQueryText': sql, 'datasource': dashboard['panels'][0]['datasource']}
                request = urllib.request.Request(url + '/api/ds/query', data=json.dumps({'queries': [query]}).encode(),
                                                 headers={'Content-Type': 'application/json'})
                with urllib.request.urlopen(request, timeout=5) as response:
                    live = json.load(response)['results']['A']
                if live.get('error'):
                    raise RuntimeError(live['error'])
                updated_rows = [frame['data']['values'] for frame in live.get('frames', [])]
                if 'sample' in json.dumps(frames) and '750' in json.dumps(frames) and updated_rows == [[['synthetic-0000']]]:
                    break
            except (OSError, http.client.HTTPException, RuntimeError):
                if time.monotonic() > deadline:
                    raise
            if time.monotonic() > deadline:
                raise RuntimeError('The Grafana live query did not show the managed command')
            time.sleep(1)
        with urllib.request.urlopen(url + '/api/prometheus/grafana/api/v1/rules', timeout=5) as response:
            alert_rules = json.load(response)
        deadline = time.monotonic() + 60
        while not any(rule.get('name') == 'unassigned' and rule.get('state') == 'firing'
                      for group in alert_rules['data']['groups'] for rule in group['rules']):
            if time.monotonic() > deadline:
                raise RuntimeError(f'Grafana did not evaluate the synthetic violation: {alert_rules}')
            time.sleep(1)
            with urllib.request.urlopen(url + '/api/prometheus/grafana/api/v1/rules', timeout=5) as response:
                alert_rules = json.load(response)
        subprocess.run([*compose, 'restart', 'collector', 'grafana'], check=True, timeout=30)
        url = 'http://' + subprocess.check_output([*compose, 'port', 'grafana', '3000'], text=True).strip()
        if process.poll() is not None or len(list(records.glob('*/report.json'))) != 1001 + len(dag.EXAMPLES):
            raise RuntimeError('Observation restart changed the managed run')
        deadline = time.monotonic() + 60
        while True:
            if time.monotonic() > deadline:
                raise RuntimeError('Grafana did not recover after restart')
            try:
                with urllib.request.urlopen(url + '/api/health', timeout=3) as response:
                    if json.load(response)['database'] == 'ok':
                        break
            except (OSError, http.client.HTTPException):
                if time.monotonic() > deadline:
                    raise
            time.sleep(1)
        query = {**dashboard['panels'][0]['targets'][0], 'datasource': dashboard['panels'][0]['datasource']}
        deadline = time.monotonic() + 30
        while True:
            request = urllib.request.Request(url + '/api/ds/query', data=json.dumps({'queries': [query]}).encode(),
                                             headers={'Content-Type': 'application/json'})
            with urllib.request.urlopen(request, timeout=5) as response:
                recovered = json.load(response)['results']['A']
            if not recovered.get('error') and 'sample' in json.dumps(recovered):
                break
            if time.monotonic() > deadline:
                raise RuntimeError('The observation query did not recover')
            time.sleep(1)
        dag_queries = dag.check(url)
        refresh_seconds = time.monotonic() - refresh_started
        stats = subprocess.check_output(['docker', 'stats', '--no-stream', '--format', '{{json .}}',
            *subprocess.check_output([*compose, 'ps', '-q'], text=True).split()], text=True)
        for sql in ("CREATE TABLE forbidden_write (value TEXT)",
                    "ATTACH DATABASE '/var/lib/grafana/grafana.db' AS private"):
            query = {**dashboard['panels'][0]['targets'][0], 'queryText': sql,
                     'datasource': dashboard['panels'][0]['datasource']}
            request = urllib.request.Request(url + '/api/ds/query', data=json.dumps({'queries': [query]}).encode(),
                                             headers={'Content-Type': 'application/json'})
            try:
                response = urllib.request.urlopen(request, timeout=5)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                rejected = json.load(response)['results']['A']
            if not rejected.get('error'):
                raise RuntimeError('The data source did not reject a forbidden query')
        print(json.dumps({'result': 'PASS', 'url': url, 'overhead': overhead, 'refresh_and_alert_seconds': refresh_seconds, 'container_stats': stats, 'alert_evaluation': 'firing', 'queried_panels': len(dashboard['panels']), 'dag_queries': dag_queries}), flush=True)
    finally:
        if process:
            process.communicate(input=b'x', timeout=15)
        subprocess.run([*compose, 'down', '--volumes'], check=False, timeout=30)
        temporary_directory.cleanup()
        repo.doCleanups()


if __name__ == '__main__':
    main()

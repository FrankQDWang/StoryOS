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


def main():
    repo = verification_tests.VerificationCommandTests()
    repo.setUp()
    root = Path(__file__).resolve().parents[1]
    compose = ['docker', 'compose', '-p', 'storyos-observation-smoke', '-f',
               str(root / 'scripts/observation/compose.yaml')]
    process = None
    temporary_directory = tempfile.TemporaryDirectory()
    try:
        temporary = temporary_directory.name
        output = Path(temporary)
        (output / 'data').mkdir()
        policy = repo.root / 'docs/agents/verification-policy.json'
        value = json.loads(policy.read_text())
        child = "import sys; print('ready', flush=True); sys.stdin.read(1)"
        value['targeted'] = {'sample': {'command': [sys.executable, '-c', child], 'clean': False}}
        policy.write_text(json.dumps(value))
        process = subprocess.Popen([sys.executable, str(verification_tests.COMMAND), 'targeted',
            '--check', 'sample', '--issue', '749'], cwd=repo.root, env=repo.environment,
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL)
        records = repo.root / 'target/verification'
        deadline = time.monotonic() + 120
        while not list(records.glob('*/steps/*.json')):
            if time.monotonic() > deadline or process.poll() is not None:
                raise RuntimeError('Synthetic managed command did not start')
            time.sleep(0.1)
        override = output / 'compose.yaml'
        override.write_text('services:\n  collector:\n    volumes:\n'
            f'      - {records}:/records:ro\n      - {output}/data:/observation\n'
            '  grafana:\n    ports: !override ["127.0.0.1::3000"]\n    volumes:\n'
            f'      - {output}/data:/observation:ro\n')
        compose += ['-f', str(override)]
        subprocess.run([*compose, 'up', '-d', '--build'], check=True, timeout=180)
        port = subprocess.check_output([*compose, 'port', 'grafana', '3000'], text=True).strip()
        url = 'http://' + port
        dashboard = json.loads((root / 'scripts/observation/dashboards/live.json').read_text())
        deadline = time.monotonic() + 90
        while True:
            try:
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
                if 'sample' in json.dumps(frames) and '749' in json.dumps(frames):
                    break
            except (OSError, http.client.HTTPException, RuntimeError):
                if time.monotonic() > deadline:
                    raise
            if time.monotonic() > deadline:
                raise RuntimeError('The Grafana live query did not show the managed command')
            time.sleep(1)
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
        print(json.dumps({'result': 'PASS', 'url': url, 'frames': frames}), flush=True)
    finally:
        if process:
            process.communicate(input=b'x', timeout=15)
        subprocess.run([*compose, 'down', '--volumes'], check=False, timeout=30)
        temporary_directory.cleanup()
        repo.doCleanups()


if __name__ == '__main__':
    main()

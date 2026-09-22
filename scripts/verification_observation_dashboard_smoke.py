"""Exercise the provisioned run dashboard with isolated, labelled examples."""

from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import urllib.request
import urllib.error


EXAMPLES = ('partial', 'complete', 'running', 'failed', 'reused', 'legacy')


def prepare(records):
    now = datetime.now(timezone.utc).isoformat()
    for example in EXAMPLES:
        run = 'synthetic-dag-' + example
        directory = records / run
        (directory / 'steps').mkdir(parents=True, exist_ok=True)
        graph = {'version': 1, 'identity': {'source': {'tree': 'synthetic-example'}},
            'nodes': [{'id': name, 'type': kind, 'selected': example=='complete' or name!='optional'}
                      for name, kind in [('setup', 'setup'), ('build', 'build'), ('tests', 'aggregate'),
                                         ('file-a', 'test-file'), ('file-b', 'test-file'),
                                         ('cleanup', 'cleanup'), ('optional', 'check')]],
            'dependencies': [{'from': a, 'to': b} for a,b in [('setup','build'),('build','tests'),('tests','cleanup')]],
            'relations': [{'from': 'tests', 'to': name, 'type': 'member'} for name in ['file-a','file-b']]}
        for node in graph['nodes']:
            if node['type']=='test-file':
                node['path'] = 'synthetic/' + node['id'] + '_tests.py'
        report = {'record_version': 1, 'run_id': run, 'issue': 761, 'profile': 'synthetic-' + example,
                  'status': 'running' if example=='running' else 'failed' if example=='failed' else 'passed',
                  'started_at': now, 'heartbeat_at': now, 'attempt_started': example!='reused', 'graph': graph}
        if example!='running':
            report['ended_at'] = now
        if example=='reused':
            report['cache'] = {'status': 'hit', 'producer': 'synthetic-dag-complete/report.json'}
        if example=='legacy':
            report = {'status': 'passed'}
        (directory / 'report.json').write_text(json.dumps(report))
        if example in {'reused', 'legacy'}:
            continue
        for index, name in enumerate(['setup','build','tests','cleanup']):
            if example in {'running','failed'} and index>1:
                break
            status = report['status'] if name=='build' else 'passed'
            attempt = {'node_version': 1, 'run_id': run, 'id': name, 'node_id': name,
                'graph_sha256': hashlib.sha256(json.dumps(graph, sort_keys=True).encode()).hexdigest(),
                'attempt_started': True, 'status': status, 'started_at': now,
                'selection_reason': ['synthetic visual example'], 'execution_scope': {'group': name}}
            if status!='running':
                attempt.update(ended_at=now, duration_seconds=0)
            (directory / f'steps/{name}.json').write_text(json.dumps(attempt))


def check(url):
    dashboard = json.loads((Path(__file__).with_name('observation') / 'dashboards/run.json').read_text())
    queried = 0
    for example in EXAMPLES:
        for group in ['__none', 'tests']:
            for panel in dashboard['panels']:
                for target in panel.get('targets', []):
                    query = {**target, 'datasource': panel['datasource']}
                    for field in ['queryText', 'rawQueryText']:
                        for key,value in {'run': 'synthetic-dag-'+example, 'group': group, 'node': 'build'}.items():
                            query[field] = query[field].replace('${'+key+':sqlstring}', "'"+value+"'")
                    request = urllib.request.Request(url+'/api/ds/query', data=json.dumps({'queries':[query]}).encode(),
                                                     headers={'Content-Type':'application/json'})
                    try:
                        response = urllib.request.urlopen(request, timeout=10)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        result = json.load(response)['results'][target['refId']]
                    if result.get('error'):
                        raise RuntimeError(f"{example} / {panel['title']}: {result['error']}")
                    if target['refId']=='nodes':
                        frames = result['frames']
                        ids = frames[0]['data']['values'][0] if frames else []
                        expected = 0 if example=='legacy' else 5 if group=='__none' else 7
                        if len(ids)!=expected:
                            raise RuntimeError(f'{example} / {group}: expected {expected} nodes, got {ids}')
                    queried += 1
    return queried + check_comparison(url)


def check_comparison(url):
    dashboard = json.loads((Path(__file__).with_name('observation') / 'dashboards/compare.json').read_text())
    queried = 0
    for left,right in [('partial','complete'), ('failed','complete'), ('reused','complete'), ('legacy','partial')]:
        for panel in dashboard['panels']:
            for target in panel.get('targets', []):
                query = {**target, 'datasource': panel['datasource']}
                for field in ['queryText', 'rawQueryText']:
                    for key,value in {'left': left, 'right': right}.items():
                        query[field] = query[field].replace('${'+key+':sqlstring}', "'synthetic-dag-"+value+"'")
                request = urllib.request.Request(url+'/api/ds/query', data=json.dumps({'queries': [query]}).encode(),
                                                 headers={'Content-Type': 'application/json'})
                with urllib.request.urlopen(request, timeout=30) as response:
                    result = json.load(response)['results'][target['refId']]
                if result.get('error'):
                    raise RuntimeError(f"Comparison / {panel['title']}: {result['error']}")
                frames = result.get('frames', [])
                if panel['id'] in {1,2,3} and not (frames and frames[0]['data']['values'][0]):
                    raise RuntimeError('Comparison lost retained run or graph rows')
                if panel['id']==1:
                    expected = 'not comparable' if left=='legacy' else 'descriptive only'
                    if frames[0]['data']['values'][0] != [expected]:
                        raise RuntimeError('Comparison fabricated comparable evidence')
                queried += 1
    return queried

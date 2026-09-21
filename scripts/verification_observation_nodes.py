"""Project retained graph membership and actual command attempts separately."""

from datetime import datetime
import hashlib
import json
import math


SCHEMA = """
CREATE TABLE IF NOT EXISTS run_graphs (run_id TEXT PRIMARY KEY, graph_sha256 TEXT, payload TEXT);
CREATE TABLE IF NOT EXISTS node_attempts (
 run_id TEXT, graph_sha256 TEXT, node_id TEXT, attempt_id TEXT, parent TEXT,
 selection_reason TEXT, execution_scope TEXT, started_at TEXT, ended_at TEXT,
 duration_seconds REAL, result TEXT, PRIMARY KEY(run_id, attempt_id));
CREATE TABLE IF NOT EXISTS node_states (
 run_id TEXT, graph_sha256 TEXT, node_id TEXT, type TEXT, path TEXT, selected INTEGER,
 state TEXT, duration_seconds REAL, producer TEXT, PRIMARY KEY(run_id, node_id));
"""


def validate(value):
    producer = (value.get('cache') or {}).get('producer')
    if producer is not None and not isinstance(producer, str):
        raise ValueError('Invalid cache producer')
    graph = value.get('graph') or (value.get('plan') or {}).get('graph')
    if graph is not None:
        if (not isinstance(graph, dict) or graph.get('version') != 1
                or not isinstance(graph.get('nodes'), list) or not isinstance(graph.get('relations'), list)):
            raise ValueError('Invalid retained graph')
        ids = [node['id'] for node in graph['nodes']]
        if len(set(ids)) != len(ids) or not all(isinstance(name, str) for name in ids):
            raise ValueError('Invalid graph node identities')
        for node in graph['nodes']:
            if (type(node.get('selected')) is not bool or not isinstance(node.get('type'), str)
                    or any(node.get(key) is not None and not isinstance(node[key], str) for key in ('profile', 'path'))):
                raise ValueError('Invalid graph node')
        for edges in (graph.get('dependencies'), graph['relations']):
            if not isinstance(edges, list) or any(not isinstance(edge, dict) or
                    edge.get('from') not in ids or edge.get('to') not in ids for edge in edges):
                raise ValueError('Invalid graph edges')
        checks = (value.get('plan') or {}).get('checks', [])
        if not isinstance(checks, list) or any(not isinstance(check, dict) or
                not isinstance(check.get('group'), str) for check in checks):
            raise ValueError('Invalid node selection')
    if 'node_version' in value:
        if value['node_version'] != 1 or any(not isinstance(value.get(key), str) or not value[key]
                for key in ('run_id', 'graph_sha256', 'node_id', 'id')):
            raise ValueError('Invalid node attempt identity')
        if not isinstance(value.get('selection_reason'), list) or not value.get('execution_scope'):
            raise ValueError('Missing node attempt scope')
        for key in ('started_at', 'ended_at'):
            item = value.get(key)
            if item is not None and (not isinstance(item, str) or datetime.fromisoformat(item).tzinfo is None):
                raise ValueError('Invalid node timestamp')
        for key in ('duration_seconds', 'started_monotonic', 'ended_monotonic'):
            item = value.get(key)
            if item is not None and (type(item) not in (int, float) or not math.isfinite(item) or item < 0):
                raise ValueError('Invalid node attempt time')
    return graph


def refresh(connection, runs):
    for run in runs:
        for table in ('run_graphs', 'node_attempts', 'node_states'):
            connection.execute(f'DELETE FROM {table} WHERE run_id=?', (run,))
        rows = connection.execute("SELECT kind,payload FROM records WHERE run=? AND quality='valid'", (run,)).fetchall()
        roots = [json.loads(raw) for kind, raw in rows if kind == 'run']
        if not roots or not roots[0].get('graph'):
            continue
        root, graph = roots[0], roots[0]['graph']
        digest = hashlib.sha256(json.dumps(graph, sort_keys=True).encode()).hexdigest()
        connection.execute('INSERT INTO run_graphs VALUES (?,?,?)', (run, digest, json.dumps(graph)))
        nodes = {node['id']: node for node in graph['nodes']}
        attempts = {}
        for kind, raw in rows:
            step = json.loads(raw)
            name = step.get('node_id')
            if (kind != 'step' or step.get('node_version') != 1 or step.get('run_id') != run
                    or step.get('graph_sha256') != digest or name not in nodes or step.get('attempt_started') is not True):
                continue
            attempts.setdefault(name, {})[step['id']] = step
        for name, entries in attempts.items():
            for step in entries.values():
                connection.execute('INSERT INTO node_attempts VALUES (?,?,?,?,?,?,?,?,?,?,?)',
                    (run, digest, name, step['id'], step.get('parent'), json.dumps(step['selection_reason']),
                     json.dumps(step['execution_scope']), step.get('started_at'), step.get('ended_at'),
                     step.get('duration_seconds'), step['status']))
        failed = {name for name, entries in attempts.items() if
                  max(entries.values(), key=lambda step: step.get('started_monotonic', 0))['status'] in {'failed', 'interrupted'}}
        blocked = set()
        while True:
            following = {edge['to'] for edge in graph.get('dependencies', []) if
                         edge['from'] in failed | blocked and nodes[edge['to']]['selected'] and
                         (edge.get('when') != 'both-selected' or nodes[edge['from']]['selected'])}
            if following <= blocked:
                break
            blocked.update(following)
        checks = {check['group']: check for check in root.get('node_checks', [])}
        for name, node in nodes.items():
            entries = list(attempts.get(name, {}).values())
            producer, duration = None, None
            state = 'pending' if node['selected'] else 'not-selected'
            if node['selected'] and root.get('cache_status') == 'hit':
                state, producer = 'cached', root.get('cache_producer')
            elif entries:
                latest = max(entries, key=lambda step: step.get('started_monotonic', 0))
                state, duration = latest['status'], latest.get('duration_seconds')
                if (state == 'running' and root.get('status') != 'running') or (state != 'running' and not latest.get('ended_at')):
                    state = 'unknown'
            elif node['selected'] and checks.get(node.get('profile'), {}).get('status') == 'pending':
                state = 'pending'
            elif name in blocked:
                state = 'blocked'
            elif node['selected'] and root.get('status') not in {'running', 'pending'}:
                state = 'unknown'
            connection.execute('INSERT INTO node_states VALUES (?,?,?,?,?,?,?,?,?)',
                (run, digest, name, node['type'], node.get('path'), node['selected'], state, duration, producer))

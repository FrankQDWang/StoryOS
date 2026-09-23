"""Serve bounded read-only queries over the disposable observation projection."""

import argparse
from contextlib import closing
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import re
from pathlib import Path
import sqlite3
import time
from urllib.parse import parse_qs, urlsplit

from verification_observation_health import Probe
import verification_observation_compare as comparison


FIELDS = ('issue', 'profile', 'status', 'started_at', 'actual_started_at',
          'ended_at', 'heartbeat_at', 'duration_seconds', 'attempt_started', 'cache_status', 'cache_producer')
RUNS = "SELECT run,quality," + ','.join(
    f"json_extract(payload,'$.{field}') AS {field}" for field in FIELDS) + " FROM records WHERE kind='run'"
RUN_ID = re.compile(r'[A-Za-z0-9][A-Za-z0-9_-]{0,127}\Z')


def page_parameters(parameters, allowed, max_limit=100):
    if set(parameters) - allowed:
        raise ValueError('unsupported_parameter')
    limit, offset = int(parameters.get('limit', '50')), int(parameters.get('offset', '0'))
    if not 1 <= limit <= max_limit or not 0 <= offset <= 1000000:
        raise ValueError('invalid_page')
    return limit, offset


def rows_page(connection, sql, args, order, limit, offset):
    total = connection.execute('SELECT count(*) FROM (' + sql + ')', args).fetchone()[0]
    rows = [dict(row) for row in connection.execute(sql + ' ORDER BY ' + order +
            ' LIMIT ? OFFSET ?', [*args, limit, offset])]
    return {'items': rows, 'total': total, 'offset': offset,
            'next_offset': offset + len(rows) if offset + len(rows) < total else None}


def query(connection, path, parameters):
    if path == '/api/v1/violations':
        limit, offset = page_parameters(parameters, {'limit', 'offset'})
        sql = ("SELECT v.run,v.rule,v.disposition,v.evidence," + ','.join(
            f"COALESCE(json_extract(r.payload,'$.{field}'),json_extract(src.payload,'$.{source}')) AS {field}" for field, source in
            (('issue', 'issue'), ('profile', 'profile'), ('status', 'status'), ('started_at', 'utc'))) +
            " FROM violations v LEFT JOIN records r ON r.run=v.run AND r.kind='run' LEFT JOIN records src ON src.path=v.evidence")
        return rows_page(connection, sql, [], 'v.rule,started_at DESC,v.run,v.evidence', limit, offset)
    if path == '/api/v1/compare':
        left, right = parameters.get('left', ''), parameters.get('right', '')
        limit, offset = page_parameters(parameters, {'left', 'right', 'limit', 'offset'}, 500)
        if not RUN_ID.fullmatch(left) or not RUN_ID.fullmatch(right) or left == right:
            raise ValueError('invalid_run')
        if connection.execute("SELECT count(*) FROM records WHERE kind='run' AND run IN (?,?)", (left, right)).fetchone()[0] != 2:
            raise LookupError('not_found')
        def compare_sql(statement):
            return statement.replace(comparison.LEFT, '?').replace(comparison.RIGHT, '?')
        args = [left, right]
        quality = connection.execute(compare_sql(comparison.COMPARISON), args).fetchone()
        summary = [dict(row) for row in connection.execute(compare_sql(comparison.SUMMARY), args)]
        differences_all = [dict(row) for row in connection.execute(compare_sql(comparison.DIFFERENCE), args)]
        differences = {'items': differences_all[offset:offset + limit], 'total': len(differences_all),
                       'offset': offset, 'next_offset': offset + limit if offset + limit < len(differences_all) else None}
        return {'comparison': dict(quality), 'runs': summary, 'differences': differences}
    if path == '/api/v1/overview':
        if parameters:
            raise ValueError('unsupported_parameter')
        heartbeat = connection.execute("SELECT value FROM observation_settings WHERE name='heartbeat_seconds'").fetchone()[0]
        counts = connection.execute("SELECT count(*) AS unfinished, count(CASE WHEN "
            "(julianday('now')-julianday(heartbeat_at))*86400 BETWEEN 0 AND ? THEN 1 END) AS active "
            "FROM (" + RUNS + ") WHERE status='running'", (heartbeat,)).fetchone()
        costs = connection.execute("SELECT count(*) AS starts, CASE WHEN count(c.seconds)=count(*) "
            "THEN coalesce(sum(c.seconds),0) END AS seconds FROM run_cost c JOIN records r ON r.run=c.run "
            "AND r.kind='run' WHERE julianday(coalesce(json_extract(r.payload,'$.actual_started_at'),"
            "json_extract(r.payload,'$.started_at'))) BETWEEN julianday('now','-1 day') AND julianday('now')").fetchone()
        return {**dict(counts), **dict(costs), 'heartbeat_seconds': heartbeat}
    match = re.fullmatch(r'/api/v1/runs/([A-Za-z0-9][A-Za-z0-9_-]{0,127})(?:/(files|attempts|graph|cost))?', path)
    args, allowed = [], {'limit', 'offset'}
    if path == '/api/v1/runs':
        allowed |= {'q', 'status', 'sort'}
        search, status = parameters.get('q', ''), parameters.get('status', '')
        if len(search) > 128 or len(status) > 32:
            raise ValueError('invalid_filter')
        clock = "replace(replace(started_at,'-',''),':','')"
        orders = {'newest': clock + ' DESC,run DESC', 'oldest': clock + ',run',
                  'duration': 'duration_seconds DESC,run DESC'}
        order = orders.get(parameters.get('sort', 'newest'))
        if order is None:
            raise ValueError('invalid_sort')
        sql = 'SELECT * FROM (' + RUNS + ") WHERE instr(lower(run||' '||coalesce(issue,'')||' '||coalesce(profile,'')),lower(?))>0"
        args.append(search)
        if status:
            sql += ' AND status=?'
            args.append(status)
    elif path == '/api/v1/requests':
        allowed |= {'run'}
        sql = "SELECT path AS evidence,quality," + ','.join(
            f"json_extract(payload,'$.{field}') AS {field}" for field in
            ('id', 'run_id', 'issue', 'outcome', 'utc', 'profile')) + " FROM records WHERE kind='request'"
        if 'run' in parameters:
            sql += " AND json_extract(payload,'$.run_id')=?"
            args.append(parameters['run'])
        order = 'utc DESC,evidence'
    elif match:
        run, section = match.groups()
        root = connection.execute(RUNS + ' AND run=?', (run,)).fetchone()
        if root is None:
            raise LookupError('not_found')
        if section is None:
            if parameters:
                raise ValueError('unsupported_parameter')
            retained = connection.execute("SELECT payload FROM records WHERE kind='run' AND run=?", (run,)).fetchone()
            payload = json.loads(retained[0])
            return {'record': dict(root), 'reason': payload.get('reason'),
                    'has_graph': connection.execute('SELECT 1 FROM run_graphs WHERE run_id=?', (run,)).fetchone() is not None,
                    'evidence': run + '/report.json'}
        if section in {'graph', 'cost'}:
            if parameters:
                raise ValueError('unsupported_parameter')
            if section == 'graph':
                graph = connection.execute('SELECT payload FROM run_graphs WHERE run_id=?', (run,)).fetchone()
                states = [dict(row) for row in connection.execute(
                    'SELECT node_id,type,path,selected,state,producer FROM node_states WHERE run_id=? ORDER BY node_id', (run,))]
                return {'graph': json.loads(graph[0]) if graph else None, 'states': states}
            root_cost = connection.execute('SELECT * FROM run_cost WHERE run=?', (run,)).fetchone()
            stage_cost = [dict(row) for row in connection.execute(
                'SELECT stage,seconds FROM stage_cost WHERE run=? ORDER BY seconds DESC,stage', (run,))]
            issue = root['issue']
            if issue is None:
                return {'root': dict(root_cost) if root_cost else None, 'stages': stage_cost, 'issue': None}
            profiles = [dict(row) for row in connection.execute(
                'SELECT profile,status,attempts,seconds FROM issue_cost WHERE issue=? ORDER BY profile,status', (issue,))]
            requests = [dict(row) for row in connection.execute(
                'SELECT outcome,requests FROM request_cost WHERE issue=? ORDER BY outcome', (issue,))]
            stages = [dict(row) for row in connection.execute(
                'SELECT s.stage,SUM(s.seconds) AS seconds FROM stage_cost s JOIN run_cost c ON c.run=s.run '
                'WHERE c.issue=? GROUP BY s.stage ORDER BY seconds DESC,s.stage', (issue,))]
            wait = connection.execute('SELECT seconds FROM issue_wait WHERE issue=?', (issue,)).fetchone()
            return {'root': dict(root_cost) if root_cost else None, 'stages': stage_cost,
                    'issue': {'id': issue, 'profiles': profiles, 'requests': requests,
                              'stages': stages, 'blocked_seconds': wait[0] if wait else None}}
        if section == 'files':
            sql, order = "SELECT * FROM node_states WHERE run_id=? AND type='test-file'", 'path,node_id'
        else:
            sql, order = 'SELECT * FROM node_attempts WHERE run_id=?', 'started_at,attempt_id'
        args.append(run)
    else:
        raise LookupError('not_found')
    limit, offset = page_parameters(parameters, allowed)
    page = rows_page(connection, sql, args, order, limit, offset)
    rows = page['items']
    for row in rows:
        if 'evidence' in row and not re.fullmatch(r'requests/[A-Za-z0-9_-]+\.json', row['evidence']):
            row['evidence'] = None
    return page


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(3)

    def do_GET(self):
        status, data = 200, {}
        try:
            if self.headers.get('Host') not in {f'127.0.0.1:{self.server.server_port}', f'localhost:{self.server.server_port}'} or self.headers.get('Origin') not in {None, 'http://127.0.0.1:3749'}:
                self.respond(403, {'error': 'local_origin_required'})
                return
            if len(self.path) > 2048:
                raise ValueError('request_too_long')
            parts = urlsplit(self.path)
            parameters = parse_qs(parts.query, keep_blank_values=True, max_num_fields=8)
            if any(len(values) != 1 for values in parameters.values()):
                raise ValueError('duplicate_parameter')
            if parts.path == '/api/v1/health':
                if parameters:
                    raise ValueError('unsupported_parameter')
                self.respond(200, self.server.probe.snapshot())
                return
            with closing(sqlite3.connect(self.server.database.as_uri() + '?mode=ro', uri=True, timeout=1)) as connection:
                connection.row_factory = sqlite3.Row
                deadline = time.monotonic() + 5
                connection.set_progress_handler(lambda: time.monotonic() > deadline, 1000)
                connection.execute('PRAGMA query_only=ON')
                connection.execute('BEGIN')
                if connection.execute('PRAGMA application_id').fetchone()[0] != 749:
                    raise sqlite3.DatabaseError('Not an observation database')
                data = query(connection, parts.path, {key: values[0] for key, values in parameters.items()})
                data['projection_modified_at'] = datetime.fromtimestamp(
                    self.server.database.stat().st_mtime, timezone.utc).isoformat()
        except LookupError:
            status, data = 404, {'error': 'not_found'}
        except (ValueError, KeyError):
            status, data = 400, {'error': 'invalid_request'}
        except (OSError, sqlite3.Error):
            status, data = 503, {'error': 'projection_unavailable'}
        self.respond(status, data)

    def respond(self, status, data):
        body = json.dumps({'version': 1, 'read_only': True,
            'queried_at': datetime.now(timezone.utc).isoformat(), **data}, allow_nan=False).encode()
        if len(body) > 1024 * 1024:
            self.respond(413, {'error': 'response_limit'})
            return
        self.send_response(status)
        self.send_header('Content-Type', 'application/json; charset=utf-8')
        self.send_header('Cache-Control', 'no-store')
        self.send_header('Access-Control-Allow-Origin', 'http://127.0.0.1:3749')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        self.respond(405, {'error': 'read_only'})

    do_PUT = do_DELETE = do_PATCH = do_POST

    def log_message(self, *_args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--database', type=Path, default=Path('target/observation/data/runs.sqlite'))
    parser.add_argument('--port', type=int, default=3754)
    parser.add_argument('--container', action='store_true')
    parser.add_argument('--collector-health', type=Path, default=Path('target/observation/data/collector.json'))
    parser.add_argument('--health-file', type=Path, default=Path('target/observation/health/probe.json'))
    parser.add_argument('--grafana-url', default='http://127.0.0.1:3749')
    args = parser.parse_args()
    grafana = urlsplit(args.grafana_url)
    if grafana.scheme != 'http' or grafana.hostname not in {'127.0.0.1', 'grafana'} or grafana.path or grafana.query or grafana.fragment or grafana.username:
        parser.error('Use a local Grafana HTTP origin')
    with HTTPServer(('0.0.0.0' if args.container else '127.0.0.1', args.port), Handler) as server:
        server.database = args.database.resolve()
        server.probe = Probe(server.database, args.collector_health, args.grafana_url, args.health_file)
        server.probe.thread.start()
        print(f'http://127.0.0.1:{server.server_port}', flush=True)
        try:
            server.serve_forever()
        finally:
            server.probe.stop.set()
            server.probe.thread.join(timeout=7)


if __name__ == '__main__':
    main()

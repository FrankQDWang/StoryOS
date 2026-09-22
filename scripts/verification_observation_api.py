"""Serve bounded read-only queries over the disposable observation projection."""

import argparse
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import re
from pathlib import Path
import sqlite3
import time
from urllib.parse import parse_qs, urlsplit


FIELDS = ('issue', 'profile', 'status', 'started_at', 'actual_started_at',
          'ended_at', 'heartbeat_at', 'duration_seconds', 'attempt_started', 'cache_status', 'cache_producer')
RUNS = "SELECT run,quality," + ','.join(
    f"json_extract(payload,'$.{field}') AS {field}" for field in FIELDS) + " FROM records WHERE kind='run'"


def query(connection, path, parameters):
    match = re.fullmatch(r'/api/v1/runs/([A-Za-z0-9][A-Za-z0-9_-]{0,127})(?:/(files|attempts))?', path)
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
        if section == 'files':
            sql, order = "SELECT * FROM node_states WHERE run_id=? AND type='test-file'", 'path,node_id'
        else:
            sql, order = 'SELECT * FROM node_attempts WHERE run_id=?', 'started_at,attempt_id'
        args.append(run)
    else:
        raise LookupError('not_found')
    if set(parameters) - allowed:
        raise ValueError('unsupported_parameter')
    limit, offset = int(parameters.get('limit', '50')), int(parameters.get('offset', '0'))
    if not 1 <= limit <= 100 or not 0 <= offset <= 1000000:
        raise ValueError('invalid_page')
    total = connection.execute('SELECT count(*) FROM (' + sql + ')', args).fetchone()[0]
    rows = [dict(row) for row in connection.execute(sql + ' ORDER BY ' + order +
            ' LIMIT ? OFFSET ?', [*args, limit, offset])]
    for row in rows:
        if 'evidence' in row and not re.fullmatch(r'requests/[A-Za-z0-9_-]+\.json', row['evidence']):
            row['evidence'] = None
    return {'items': rows, 'total': total, 'offset': offset,
            'next_offset': offset + len(rows) if offset + len(rows) < total else None}


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
            with sqlite3.connect(self.server.database.as_uri() + '?mode=ro', uri=True, timeout=1) as connection:
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
    args = parser.parse_args()
    with HTTPServer(('0.0.0.0' if args.container else '127.0.0.1', args.port), Handler) as server:
        server.database = args.database.resolve()
        print(f'http://127.0.0.1:{server.server_port}', flush=True)
        server.serve_forever()


if __name__ == '__main__':
    main()

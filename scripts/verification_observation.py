"""Build a disposable observation database without importing the executor."""

import argparse
import hashlib
import json
from pathlib import Path
import signal
import sqlite3
import threading
import time


SCHEMA = """
CREATE TABLE IF NOT EXISTS records (
 path TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, kind TEXT NOT NULL,
 run TEXT, quality TEXT NOT NULL, diagnostic TEXT, payload TEXT NOT NULL);
CREATE VIEW IF NOT EXISTS current_execution AS
SELECT r.run AS run_id, json_extract(r.payload, '$.issue') AS issue,
 json_extract(r.payload, '$.profile') AS profile,
 COALESCE(json_extract(s.payload, '$.stage'), 'starting') AS stage,
 json_extract(s.payload, '$.parent') AS parent_step,
 json_extract(r.payload, '$.scope') AS scope,
 json_extract(r.payload, '$.reason') AS reason,
 json_extract(r.payload, '$.status') AS status,
 json_extract(r.payload, '$.heartbeat_at') AS heartbeat_at,
 CASE WHEN julianday('now') - julianday(json_extract(r.payload, '$.heartbeat_at')) < 30.0/86400
 THEN 'recent heartbeat' ELSE 'stale or unknown' END AS liveness,
 MAX(0, (julianday(json_extract(r.payload, '$.heartbeat_at')) -
         julianday(json_extract(r.payload, '$.started_at')))) * 86400 AS elapsed_seconds,
 MAX(0, (julianday('now') - julianday(json_extract(r.payload, '$.heartbeat_at')))) * 86400 AS heartbeat_age_seconds
FROM records r LEFT JOIN records s ON s.run = r.run AND s.kind = 'step'
 AND s.quality = 'valid' AND json_extract(s.payload, '$.status') = 'running'
WHERE r.kind = 'run' AND r.quality = 'valid' AND json_extract(r.payload, '$.status') = 'running';
"""
FIELDS = ('run_id', 'id', 'issue', 'pr', 'profile', 'status', 'stage', 'parent',
          'started_at', 'ended_at', 'heartbeat_at', 'duration_seconds', 'started_monotonic',
          'ended_monotonic', 'attempt_started', 'actual_started_at', 'outcome', 'utc', 'recovery_of')
STATUSES = {'running', 'passed', 'failed', 'pending', 'interrupted', 'incomplete',
            'source-changed', 'infrastructure-failed', 'cached'}


def read_record(path, records):
    relative = path.relative_to(records)
    kind = 'request' if relative.parts[0] == 'requests' else ('run' if path.name == 'report.json' else 'step')
    run = relative.parts[0] if kind != 'request' else None
    fingerprint = 'unreadable'
    try:
        if path.is_symlink() or not path.resolve().is_relative_to(records) or path.stat().st_size > 16 * 1024 * 1024:
            raise ValueError('Record is a link outside the input boundary or exceeds 16 MiB')
        with path.open('rb') as source:
            raw = source.read(16 * 1024 * 1024 + 1)
        if len(raw) > 16 * 1024 * 1024:
            raise ValueError('Record exceeds 16 MiB')
        fingerprint = hashlib.sha256(raw).hexdigest()
        value = json.loads(raw)
        if not isinstance(value, dict):
            raise ValueError('Expected a record object')
        quality = 'valid'
        if (kind == 'run' and value.get('record_version') != 1) or (kind == 'step' and not value.get('id')):
            quality = 'legacy'
        if kind != 'request' and value.get('status') not in STATUSES:
            raise ValueError('Missing or unknown status')
        if kind == 'run' and quality == 'valid' and value.get('run_id') != run:
            raise ValueError('Run identity does not match its retained directory')
        if kind == 'request' and (value.get('version') != 1 or not value.get('id') or not value.get('outcome')):
            quality = 'legacy'
        payload = {key: value[key] for key in FIELDS if key in value}
        if quality == 'legacy':
            payload.pop('issue', None)
            payload.pop('pr', None)
        plan = value.get('plan') or value.get('effective_scope') or {}
        checks = plan.get('checks', []) if isinstance(plan, dict) else []
        payload['scope'] = [{'group': check.get('group'), 'files': check.get('files'),
                             'reasons': check.get('reasons')} for check in checks]
        if not checks:
            payload['scope'] = {key: plan[key] for key in ('stages', 'test_files') if key in plan} if isinstance(plan, dict) else None
        payload['reason'] = value.get('retry_reason') or value.get('reason') or value.get('trigger')
        return str(relative), fingerprint, kind, run, quality, ('Unversioned record; facts are historical' if quality == 'legacy' else None), json.dumps(payload, sort_keys=True, allow_nan=False)
    except (OSError, ValueError, TypeError, AttributeError) as error:
        return str(relative), fingerprint, kind, run, 'malformed', str(error), '{}'


def collect(records, database, rebuild=False):
    if not records.is_dir() or database.is_relative_to(records):
        raise ValueError('Use an existing record directory and a separate observation database')
    database.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with sqlite3.connect(database, timeout=5) as connection:
        application = connection.execute('PRAGMA application_id').fetchone()[0]
        if application not in {0, 749} or (application == 0 and connection.execute(
                "SELECT 1 FROM sqlite_master WHERE type='table'").fetchone()):
            raise ValueError('The database is not a StoryOS observation read model')
        connection.execute('PRAGMA application_id=749')
        if rebuild:
            connection.execute('DROP VIEW IF EXISTS current_execution')
        connection.executescript(SCHEMA)
        connection.execute('BEGIN IMMEDIATE')
        if rebuild:
            connection.execute('DELETE FROM records')
        previous = dict(connection.execute('SELECT path, fingerprint FROM records'))
        updated = 0
        paths = sorted([*records.glob('*/report.json'), *records.glob('*/steps/*.json'),
                        *records.glob('requests/*.json')])
        pending = 0
        for path in paths:
            row = read_record(path, records)
            if previous.get(row[0]) == row[1]:
                continue
            if updated >= 200:
                pending += 1
                continue
            connection.execute('INSERT OR REPLACE INTO records VALUES (?, ?, ?, ?, ?, ?, ?)', row)
            updated += 1
        count = connection.execute('SELECT COUNT(*) FROM records').fetchone()[0]
    return {'updated': updated, 'records': count, 'pending': pending, 'seconds': time.monotonic() - started}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['collect', 'rebuild', 'watch', 'status'])
    parser.add_argument('--records', type=Path, default=Path('target/verification'))
    parser.add_argument('--database', type=Path, default=Path('target/observation/data/runs.sqlite'))
    parser.add_argument('--interval', type=float, default=5)
    args = parser.parse_args()
    args.records, args.database = args.records.resolve(), args.database.resolve()
    if not 1 <= args.interval <= 3600:
        parser.error('Use a collection interval between 1 and 3600 seconds')
    if args.action == 'status':
        with sqlite3.connect(f'{args.database.as_uri()}?mode=ro', uri=True) as connection:
            connection.row_factory = sqlite3.Row
            print(json.dumps({'current': [dict(row) for row in connection.execute('SELECT * FROM current_execution')],
                              'quality': dict(connection.execute('SELECT quality, COUNT(*) FROM records GROUP BY quality'))}))
        return
    stop = threading.Event()
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda *_: stop.set())
    while not stop.is_set():
        try:
            result = collect(args.records, args.database, args.action == 'rebuild')
            print(json.dumps(result), flush=True)
            if args.action == 'rebuild':
                args.action = 'collect'
        except (OSError, ValueError, sqlite3.Error) as error:
            if args.action != 'watch':
                parser.exit(1, f'{error}\n')
            print(json.dumps({'error': str(error)}), flush=True)
        if args.action != 'watch':
            if result['pending']:
                continue
            break
        stop.wait(args.interval)


if __name__ == '__main__':
    main()

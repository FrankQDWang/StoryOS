"""Build a disposable observation database without importing the executor."""

import argparse
import hashlib
import json
from pathlib import Path
import resource
import sys
import signal
import sqlite3
import threading
import time

import verification_observation_cost as cost
import verification_observation_rules as rules
import verification_observation_nodes as nodes


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
 CASE WHEN julianday('now') - julianday(json_extract(r.payload, '$.heartbeat_at')) < (SELECT value FROM observation_settings WHERE name='heartbeat_seconds')/86400.0
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
          'repository', 'retry_reason', 'requested_scope', 'complete_dispatch_attempt', 'build_state', 'blocked_clock', 'blocked_intervals', 'ended_monotonic', 'attempt_started', 'actual_started_at', 'outcome', 'utc', 'recovery_of', 'node_version', 'node_id', 'graph_sha256', 'selection_reason', 'execution_scope')
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
        fingerprint = '760:' + hashlib.sha256(raw).hexdigest()
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
        cost.validate(payload)
        graph = nodes.validate(value)
        if "node_version" in value and (value.get("run_id") != run or value.get("id") != path.stem):
            raise ValueError("Node attempt identity does not match its retained path")
        if graph is not None:
            payload["graph"] = graph
            payload["node_checks"] = (value.get("plan") or {}).get("checks", [])
        payload["cache_producer"] = (value.get("cache") or {}).get("producer")
        if quality == 'legacy':
            payload.pop('issue', None)
            payload.pop('pr', None)
        plan = value.get('plan') or value.get('effective_scope') or {}
        checks = plan.get('checks', []) if isinstance(plan, dict) else []
        payload['complete_dispatch_attempt'] = value.get('complete_dispatch_attempt') is True or 'verify-local-steps' in value.get('command', [])
        payload['scope'] = [{'group': check.get('group'), 'files': check.get('files'),
                             'reasons': check.get('reasons')} for check in checks]
        if not checks:
            payload['scope'] = {key: plan[key] for key in ('stages', 'test_files') if key in plan} if isinstance(plan, dict) else None
        candidate = value.get('candidate')
        payload['cache_status'] = (value.get('cache') or {}).get('status')
        payload['candidate_key'] = hashlib.sha256(json.dumps(candidate, sort_keys=True).encode()).hexdigest() if candidate else None
        comparison = {key: candidate.get(key) for key in ('tools', 'inputs', 'runners', 'host')} if isinstance(candidate, dict) else {}
        comparison.update(build_state=value.get('build_state'), policy=(candidate or {}).get('plan', {}).get('policy_sha256'), profile=value.get('profile'), plan=payload['scope'], repository=value.get('repository'))
        payload['comparison_key'] = hashlib.sha256(json.dumps(comparison, sort_keys=True).encode()).hexdigest() if isinstance(candidate, dict) and all(candidate.get(key) for key in ('tools', 'inputs', 'runners', 'host')) and comparison['policy'] and value.get('build_state') in {'cold', 'warm'} else None
        payload['reason'] = value.get('retry_reason') or value.get('reason') or value.get('trigger')
        return str(relative), fingerprint, kind, run, quality, ('Unversioned record; facts are historical' if quality == 'legacy' else None), json.dumps(payload, sort_keys=True, allow_nan=False)
    except (OSError, ValueError, TypeError, AttributeError, KeyError) as error:
        return str(relative), fingerprint, kind, run, 'malformed', str(error), '{}'


def collect(records, database, rebuild=False):
    settings = json.loads((Path(__file__).parent / "observation/settings.json").read_text())
    if not (1 <= settings['max_batch_records'] <= 200 and 5 <= settings['heartbeat_seconds'] <= 3600
            and 3 <= settings['regression_min_samples'] <= 100 and settings['regression_ratio'] > 1
            and settings['regression_min_seconds'] > 0):
        raise ValueError('Invalid observation limits')
    if not records.is_dir() or database.is_relative_to(records):
        raise ValueError('Use an existing record directory and a separate observation database')
    database.parent.mkdir(parents=True, exist_ok=True)
    if not rebuild and database.exists() and database.stat().st_size > settings['max_database_bytes']:
        raise ValueError('Observation database size limit reached; archive retained inputs and rebuild')
    started = time.monotonic()
    cpu = time.process_time()
    with sqlite3.connect(database, timeout=5) as connection:
        application = connection.execute('PRAGMA application_id').fetchone()[0]
        if application not in {0, 749} or (application == 0 and connection.execute(
                "SELECT 1 FROM sqlite_master WHERE type='table'").fetchone()):
            raise ValueError('The database is not a StoryOS observation read model')
        connection.execute('PRAGMA application_id=749')
        if rebuild or connection.execute('PRAGMA user_version').fetchone()[0] != 750:
            connection.execute('DROP VIEW IF EXISTS current_execution')
            connection.execute('PRAGMA user_version=750')
        connection.executescript(SCHEMA + cost.SCHEMA + rules.SCHEMA + nodes.SCHEMA)
        connection.execute('BEGIN IMMEDIATE')
        if rebuild:
            connection.execute('DELETE FROM records')
            connection.execute('DELETE FROM run_cost')
            connection.execute('DELETE FROM stage_cost')
            connection.execute('DELETE FROM timing_comparison')
            for table in ('run_graphs', 'node_attempts', 'node_states'):
                connection.execute(f'DELETE FROM {table}')
        previous = dict(connection.execute('SELECT path, fingerprint FROM records'))
        updated = 0
        changed_runs = set()
        paths = sorted([*records.glob('*/report.json'), *records.glob('*/steps/*.json'), *records.glob('*/nodes/*.json'),
                        *records.glob('requests/*.json')])
        pending = 0
        for path in paths:
            row = read_record(path, records)
            if previous.get(row[0]) == row[1]:
                continue
            if updated >= settings['max_batch_records']:
                pending += 1
                continue
            connection.execute('INSERT OR REPLACE INTO records VALUES (?, ?, ?, ?, ?, ?, ?)', row)
            updated += 1
            if row[3]:
                changed_runs.add(row[3])
        nodes.refresh(connection, changed_runs)
        cost.refresh(connection, changed_runs)
        rules.refresh(connection, settings)
        count = connection.execute('SELECT COUNT(*) FROM records').fetchone()[0]
    if rebuild:
        with sqlite3.connect(database) as connection:
            connection.execute('VACUUM')
    return {'updated': updated, 'records': count, 'pending': pending, 'seconds': time.monotonic() - started,
            'cpu_seconds': time.process_time() - cpu, 'database_bytes': database.stat().st_size,
            'peak_rss_bytes': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * (1 if sys.platform == 'darwin' else 1024)}


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

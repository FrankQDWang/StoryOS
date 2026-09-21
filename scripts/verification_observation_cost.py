"""Derive additive elapsed cost from retained root and step intervals."""

import json
import math


SCHEMA = """
CREATE TABLE IF NOT EXISTS run_cost (
 run TEXT PRIMARY KEY, issue INTEGER, profile TEXT, status TEXT,
 seconds REAL, blocked_seconds REAL);
CREATE TABLE IF NOT EXISTS stage_cost (run TEXT, stage TEXT, seconds REAL);
CREATE TABLE IF NOT EXISTS issue_wait (issue INTEGER, seconds REAL);
CREATE VIEW IF NOT EXISTS issue_cost AS
 SELECT issue, profile, status, COUNT(*) AS attempts,
 CASE WHEN COUNT(seconds)=COUNT(*) THEN SUM(seconds) END AS seconds,
 (SELECT seconds FROM issue_wait w WHERE w.issue IS run_cost.issue) AS blocked_seconds
 FROM run_cost GROUP BY issue, profile, status;
CREATE VIEW IF NOT EXISTS request_cost AS
 SELECT COALESCE(json_extract(q.payload,'$.issue'), json_extract(r.payload,'$.issue')) AS issue,
 json_extract(q.payload,'$.outcome') AS outcome, COUNT(*) AS requests
 FROM records q LEFT JOIN records r ON r.kind='run' AND r.quality='valid'
 AND r.run=json_extract(q.payload,'$.run_id')
 WHERE q.kind='request' AND q.quality='valid' GROUP BY issue, outcome
 UNION ALL SELECT json_extract(payload,'$.issue'), 'daily-reused', COUNT(*) FROM records
 WHERE kind='run' AND quality='valid' AND json_extract(payload,'$.cache_status')='hit'
 GROUP BY json_extract(payload,'$.issue');
CREATE VIEW IF NOT EXISTS observed_runs AS SELECT run, quality,
 json_extract(payload,'$.issue') AS issue, json_extract(payload,'$.status') AS status,
 json_extract(payload,'$.attempt_started') AS actual_start,
 json_extract(payload,'$.duration_seconds') AS observed_seconds FROM records WHERE kind='run';
"""


def number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def validate(value):
    for key in ('blocked_clock', 'stage', 'id', 'parent', 'profile'):
        if value.get(key) is not None and not isinstance(value[key], str):
            raise ValueError(f'Invalid {key}')
    if value.get('issue') is not None and type(value['issue']) is not int:
        raise ValueError('Invalid Issue attribution')
    if 'blocked_intervals' in value and not (isinstance(value['blocked_intervals'], list) and all(
            isinstance(pair, list) and len(pair) == 2 and all(number(x) for x in pair)
            and pair[0] <= pair[1] for pair in value['blocked_intervals'])):
        raise ValueError('Invalid blocking intervals')

def partition(start, end, intervals):
    points = sorted({start, end, *(x for a, b, *_ in intervals for x in (a, b))})
    for a, b in zip(points, points[1:]):
        if start <= a < b <= end:
            yield b - a, [entry for entry in intervals if entry[0] <= a and entry[1] >= b]


def refresh(connection, runs):
    for run in runs:
        connection.execute('DELETE FROM run_cost WHERE run=?', (run,))
        connection.execute('DELETE FROM stage_cost WHERE run=?', (run,))
        rows = connection.execute("SELECT kind, payload FROM records WHERE run=? AND quality='valid'", (run,)).fetchall()
        roots = [json.loads(raw) for kind, raw in rows if kind == 'run']
        if not roots or roots[0].get('parent') or roots[0].get('attempt_started') is not True:
            continue
        root = roots[0]
        start, duration = root.get('started_monotonic'), root.get('duration_seconds')
        duration = duration if number(duration) and duration >= 0 else None
        blocked = None
        costs = {}
        if number(start) and duration is not None:
            end = start + duration
            steps = []
            for kind, raw in rows:
                step = json.loads(raw)
                a, b = step.get('started_monotonic'), step.get('ended_monotonic')
                if kind == 'step' and number(a) and number(b) and a <= b:
                    steps.append((max(start, a), min(end, b), step))
            for seconds, active in partition(start, end, steps):
                parents = {entry[2].get('parent') for entry in active}
                leaves = [entry[2] for entry in active if entry[2].get('id') not in parents]
                stage = leaves[0].get('stage', 'unclassified') if len(leaves) == 1 else ('concurrent' if leaves else 'unclassified')
                costs[stage] = costs.get(stage, 0) + seconds
            waits = root.get('blocked_intervals')
            if isinstance(waits, list) and all(isinstance(pair, list) and len(pair) == 2
                    and all(number(x) for x in pair) and start <= pair[0] <= pair[1] <= end for pair in waits):
                blocked = sum(seconds for seconds, active in partition(start, end, waits) if active)
        connection.execute('INSERT INTO run_cost VALUES (?,?,?,?,?,?)',
            (run, root.get('issue'), root.get('profile'), root.get('status'), duration, blocked))
        connection.executemany('INSERT INTO stage_cost VALUES (?,?,?)',
                               [(run, stage, seconds) for stage, seconds in costs.items()])

    connection.execute('DELETE FROM issue_wait')
    for (issue,) in connection.execute('SELECT DISTINCT issue FROM run_cost').fetchall():
        roots = [json.loads(raw) for (raw,) in connection.execute(
            "SELECT payload FROM records JOIN run_cost c ON c.run=records.run WHERE kind='run' AND c.issue IS ?", (issue,))]
        known = all(root.get('blocked_intervals') is not None for root in roots)
        clocks = {root.get('blocked_clock') for root in roots}
        valid = connection.execute('SELECT COUNT(*)=COUNT(blocked_seconds) FROM run_cost WHERE issue IS ?', (issue,)).fetchone()[0]
        seconds = None
        if known and valid and (len(roots) == 1 or (len(clocks) == 1 and None not in clocks)):
            waits = [pair for root in roots for pair in root['blocked_intervals']]
            seconds = sum(size for size, active in partition(min(pair[0] for pair in waits),
                max(pair[1] for pair in waits), waits) if active) if waits else 0
        connection.execute('INSERT INTO issue_wait VALUES (?,?)', (issue, seconds))

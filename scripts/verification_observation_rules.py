"""Build advisory rule observations without consulting or changing admission."""

import json
import statistics


SCHEMA = """
CREATE INDEX IF NOT EXISTS records_run ON records(run);
CREATE INDEX IF NOT EXISTS records_candidate ON records(json_extract(payload,'$.candidate_key'));
CREATE INDEX IF NOT EXISTS records_repository ON records(json_extract(payload,'$.repository'));
CREATE TABLE IF NOT EXISTS observation_settings (name TEXT PRIMARY KEY, value REAL);
CREATE TABLE IF NOT EXISTS timing_comparison (run TEXT PRIMARY KEY, samples INTEGER, baseline REAL, state TEXT);
CREATE VIEW IF NOT EXISTS violations AS
 WITH active_roots AS MATERIALIZED (
 SELECT run,path,json_extract(payload,'$.repository') AS repository,
 json_extract(payload,'$.candidate_key') AS candidate_key,
 json_extract(payload,'$.started_at') AS started_at,
 json_extract(payload,'$.retry_reason') AS retry_reason,
 julianday(json_extract(payload,'$.actual_started_at')) AS actual_start,
 julianday(COALESCE(json_extract(payload,'$.ended_at'),json_extract(payload,'$.heartbeat_at'))) AS observed_end
 FROM records WHERE kind='run' AND quality='valid' AND json_extract(payload,'$.attempt_started')=1)
 SELECT r.run, 'unassigned' AS rule, 'executed' AS disposition, r.path AS evidence FROM records r
 WHERE r.kind='run' AND r.quality='valid' AND json_extract(r.payload,'$.attempt_started')=1
 AND json_extract(r.payload,'$.issue') IS NULL
 UNION ALL
 SELECT r.run, 'stale-heartbeat', 'unknown-liveness', r.path FROM records r
 WHERE r.kind='run' AND r.quality='valid' AND json_extract(r.payload,'$.status')='running'
 AND (json_extract(r.payload,'$.heartbeat_at') IS NULL OR
 (julianday('now')-julianday(json_extract(r.payload,'$.heartbeat_at')))*86400 >
 (SELECT value FROM observation_settings WHERE name='heartbeat_seconds'))
 UNION ALL
 SELECT r.run, 'duplicate-candidate', 'executed', r.path FROM active_roots r
 WHERE r.candidate_key IS NOT NULL AND EXISTS (
 SELECT 1 FROM active_roots p WHERE p.run!=r.run
 AND p.candidate_key=r.candidate_key AND p.repository=r.repository
 AND (p.started_at,p.run)<(r.started_at,r.run))
 AND r.retry_reason IS NULL
 UNION ALL
 SELECT t.run, 'runtime-regression', 'executed', r.path FROM timing_comparison t
 JOIN records r ON r.run=t.run AND r.kind='run' WHERE t.state='regression'
 UNION ALL
 SELECT r.run, 'daily-complete', CASE WHEN r.kind='request' THEN 'prevented' ELSE 'executed' END, r.path
 FROM records r WHERE r.quality='valid' AND json_extract(r.payload,'$.requested_scope')='daily'
 AND (json_extract(r.payload,'$.profile')='complete' OR json_extract(r.payload,'$.complete_dispatch_attempt')=1
 OR EXISTS (SELECT 1 FROM records s WHERE s.kind='step' AND s.run=r.run AND s.quality='valid'
 AND json_extract(s.payload,'$.complete_dispatch_attempt')=1))
 UNION ALL
 SELECT r.run, 'resource-overlap', 'executed', r.path FROM active_roots r
 WHERE EXISTS (SELECT 1 FROM active_roots p WHERE p.run<r.run
 AND p.repository=r.repository AND p.actual_start<r.observed_end AND r.actual_start<p.observed_end);
"""


def refresh(connection, settings):
    connection.executemany('INSERT OR REPLACE INTO observation_settings VALUES (?,?)', settings.items())
    previous = dict(connection.execute('SELECT run, json_array(samples,baseline,state) FROM timing_comparison'))
    samples = {}
    for run, raw in connection.execute("SELECT run,payload FROM records WHERE kind='run' AND quality='valid' ORDER BY json_extract(payload,'$.started_at'),run"):
        value = json.loads(raw)
        key = value.get('comparison_key')
        prior = samples.get(key, []) if key else []
        baseline = statistics.median(prior) if len(prior) >= settings['regression_min_samples'] else None
        duration = value.get('duration_seconds')
        state = 'unknown'
        if baseline is not None and isinstance(duration, (float, int)):
            state = 'regression' if duration > max(baseline * settings['regression_ratio'], baseline + settings['regression_min_seconds']) else 'comparable'
        if json.loads(previous.get(run, 'null')) != [len(prior), baseline, state]:
            connection.execute('INSERT OR REPLACE INTO timing_comparison VALUES (?,?,?,?)', (run, len(prior), baseline, state))
        if key and value.get('attempt_started') is True and value.get('status') == 'passed' and isinstance(duration, (float, int)) and duration >= 0:
            samples.setdefault(key, []).append(duration)

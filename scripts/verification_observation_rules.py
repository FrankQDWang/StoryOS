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
 SELECT r.run, 'unassigned' AS rule, 'executed' AS disposition FROM records r
 WHERE r.kind='run' AND r.quality='valid' AND json_extract(r.payload,'$.attempt_started')=1
 AND json_extract(r.payload,'$.issue') IS NULL
 UNION ALL
 SELECT r.run, 'stale-heartbeat', 'unknown-liveness' FROM records r
 WHERE r.kind='run' AND r.quality='valid' AND json_extract(r.payload,'$.status')='running'
 AND (json_extract(r.payload,'$.heartbeat_at') IS NULL OR
 (julianday('now')-julianday(json_extract(r.payload,'$.heartbeat_at')))*86400 >
 (SELECT value FROM observation_settings WHERE name='heartbeat_seconds'))
 UNION ALL
 SELECT r.run, 'duplicate-candidate', 'executed' FROM records r
 WHERE r.kind='run' AND r.quality='valid' AND json_extract(r.payload,'$.attempt_started')=1
 AND json_extract(r.payload,'$.candidate_key') IS NOT NULL AND EXISTS (
 SELECT 1 FROM records p WHERE p.kind='run' AND p.quality='valid' AND p.run!=r.run
 AND json_extract(p.payload,'$.attempt_started')=1
 AND json_extract(p.payload,'$.candidate_key')=json_extract(r.payload,'$.candidate_key')
 AND json_extract(p.payload,'$.repository')=json_extract(r.payload,'$.repository')
 AND (json_extract(p.payload,'$.started_at'),p.run)<(json_extract(r.payload,'$.started_at'),r.run))
 AND json_extract(r.payload,'$.retry_reason') IS NULL
 UNION ALL
 SELECT run, 'runtime-regression', 'executed' FROM timing_comparison WHERE state='regression'
 UNION ALL
 SELECT r.run, 'daily-complete', CASE WHEN r.kind='request' THEN 'prevented' ELSE 'executed' END
 FROM records r WHERE r.quality='valid' AND json_extract(r.payload,'$.requested_scope')='daily'
 AND (json_extract(r.payload,'$.profile')='complete' OR json_extract(r.payload,'$.complete_dispatch_attempt')=1
 OR EXISTS (SELECT 1 FROM records s WHERE s.kind='step' AND s.run=r.run AND s.quality='valid'
 AND json_extract(s.payload,'$.complete_dispatch_attempt')=1))
 UNION ALL
 SELECT r.run, 'resource-overlap', 'executed' FROM records r
 WHERE r.kind='run' AND r.quality='valid' AND json_extract(r.payload,'$.attempt_started')=1
 AND EXISTS (SELECT 1 FROM records p WHERE p.kind='run' AND p.quality='valid' AND p.run<r.run
 AND json_extract(p.payload,'$.attempt_started')=1
 AND json_extract(p.payload,'$.repository')=json_extract(r.payload,'$.repository')
 AND julianday(json_extract(p.payload,'$.actual_started_at'))<julianday(COALESCE(json_extract(r.payload,'$.ended_at'),json_extract(r.payload,'$.heartbeat_at')))
 AND julianday(json_extract(r.payload,'$.actual_started_at'))<julianday(COALESCE(json_extract(p.payload,'$.ended_at'),json_extract(p.payload,'$.heartbeat_at'))));
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

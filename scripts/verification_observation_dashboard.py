"""Generate the local run dashboard from reusable read-only queries."""

import json
from pathlib import Path
import sys


DESTINATION = Path(__file__).with_name('observation') / 'dashboards/run.json'
SOURCE = {'type': 'frser-sqlite-datasource', 'uid': 'storyos-verification'}
RUN = '${run:sqlstring}'
GROUP = '${group:sqlstring}'
NODE = '${node:sqlstring}'
GRAPH = f"""WITH RECURSIVE
 g AS (SELECT * FROM run_graphs WHERE run_id={RUN}),
 n AS (SELECT s.*,j.value AS definition FROM node_states s JOIN g USING(run_id),
       json_each(g.payload,'$.nodes') j WHERE s.node_id=json_extract(j.value,'$.id')),
 e AS (SELECT 'dependency:'||j.key AS id,json_extract(j.value,'$.from') AS source,
       json_extract(j.value,'$.to') AS target,'dependency' AS kind,
       COALESCE(json_extract(j.value,'$.when'),'required') AS rule
       FROM g,json_each(g.payload,'$.dependencies') j
       UNION ALL SELECT 'relation:'||j.key,json_extract(j.value,'$.from'),
       json_extract(j.value,'$.to'),json_extract(j.value,'$.type'),'membership only'
       FROM g,json_each(g.payload,'$.relations') j),
 members(id) AS (SELECT {GROUP} UNION SELECT e.target FROM e JOIN members m ON e.source=m.id
                 WHERE e.kind IN ('contains','member')),
 visible AS (SELECT * FROM n WHERE type!='test-file' OR node_id IN (SELECT id FROM members))
"""
NODES = GRAPH + """SELECT node_id AS id,
 COALESCE(path,REPLACE(node_id,'check:','')) AS title,state AS mainstat,
 type AS subtitle,COALESCE(CAST(ROUND(duration_seconds,3) AS TEXT)||' s','time unknown') AS secondarystat,
 CASE state WHEN 'passed' THEN '#73BF69' WHEN 'failed' THEN '#F2495C'
 WHEN 'running' THEN '#5794F2' WHEN 'blocked' THEN '#FF9830' WHEN 'pending' THEN '#FADE2A'
 WHEN 'interrupted' THEN '#B877D9' WHEN 'cached' THEN '#8AB8FF'
 WHEN 'not-selected' THEN '#6E7681' ELSE '#CCCCDC' END AS color
 FROM visible ORDER BY node_id"""
EDGES = GRAPH + """SELECT id,source,target,kind AS mainstat,rule AS detail__rule,
 CASE kind WHEN 'dependency' THEN '#8E8E9E' ELSE '#454554' END AS color,
 CASE kind WHEN 'dependency' THEN 2 ELSE 1 END AS thickness
 FROM e WHERE source IN (SELECT node_id FROM visible) AND target IN (SELECT node_id FROM visible)
 ORDER BY id"""
SUMMARY = f"""SELECT json_extract(r.payload,'$.issue') AS issue,
 json_extract(r.payload,'$.profile') AS profile,json_extract(r.payload,'$.status') AS state,
 (SELECT SUM(selected) FROM node_states WHERE run_id=r.run) AS selected,
 (SELECT COUNT(DISTINCT node_id) FROM node_attempts WHERE run_id=r.run) AS executed,
 (SELECT COUNT(*) FROM node_states WHERE run_id=r.run) AS total,
 CASE WHEN g.run_id IS NULL THEN 'unavailable' ELSE 'retained' END AS graph,r.run AS run_id,
 COALESCE(json_extract(r.payload,'$.actual_started_at'),
 CASE WHEN json_extract(r.payload,'$.attempt_started')=1 THEN json_extract(r.payload,'$.started_at') END,'unknown') AS actual_start_UTC,
 COALESCE(json_extract(r.payload,'$.ended_at'),'unknown / not ended') AS end_UTC,
 COALESCE(json_extract(g.payload,'$.identity.source'),'unavailable') AS source,
 r.path AS retained_report,g.graph_sha256,json_extract(r.payload,'$.heartbeat_at') AS heartbeat_UTC
 FROM records r LEFT JOIN run_graphs g ON r.run=g.run_id WHERE r.kind='run' AND r.run={RUN}"""
DETAIL = GRAPH + f"""SELECT n.node_id,n.type,n.state,n.selected,
 COALESCE(n.path,'not a file') AS file,json_extract(n.definition,'$.execution') AS membership,
 (SELECT group_concat(json_extract(c.value,'$.reasons'),char(10)) FROM records r,json_each(r.payload,'$.node_checks') c
 WHERE r.run=n.run_id AND r.kind='run' AND json_extract(c.value,'$.group')=json_extract(n.definition,'$.profile')) AS selection_reason,
 (SELECT group_concat(source, char(10)) FROM e WHERE target=n.node_id AND kind='dependency') AS prerequisites,
 (SELECT group_concat(target, char(10)) FROM e WHERE source=n.node_id AND kind='dependency') AS downstream,
 (SELECT group_concat(target, char(10)) FROM e WHERE source=n.node_id AND kind IN ('contains','member')) AS members,
 COALESCE(n.producer,'none recorded') AS producer,g.graph_sha256
 FROM n,g WHERE n.node_id={NODE}"""
ATTEMPTS = f"""SELECT a.attempt_id,a.parent,a.result,a.selection_reason,a.execution_scope,
 COALESCE(a.started_at,'unknown') AS start_UTC,COALESCE(a.ended_at,'unknown') AS end_UTC,
 COALESCE(CAST(a.duration_seconds AS TEXT),'unknown') AS seconds,r.path AS retained_evidence
 FROM node_attempts a LEFT JOIN records r ON r.run=a.run_id AND r.kind='step'
 AND json_extract(r.payload,'$.id')=a.attempt_id
 WHERE a.run_id={RUN} AND a.node_id={NODE} ORDER BY a.started_at,a.attempt_id"""


def target(sql, ref='A'):
    return {'refId': ref, 'queryText': sql, 'rawQueryText': sql, 'queryType': 'table', 'timeColumns': []}


def panel(number, title, y, height, sql):
    return {'id': number, 'title': title, 'type': 'table', 'datasource': SOURCE,
            'gridPos': {'x': 0, 'y': y, 'w': 24, 'h': height}, 'targets': [target(sql)],
            'fieldConfig': {'defaults': {'noValue': 'unknown', 'custom': {'cellOptions': {'type': 'auto'}, 'wrapText': True}}, 'overrides': []},
            'options': {'showHeader': True}}


def variable(name, label, query, default=None):
    value = {'name': name, 'label': label, 'type': 'query', 'datasource': SOURCE,
             'query': query, 'refresh': 1, 'sort': 0, 'multi': False, 'includeAll': False}
    if default is not None:
        value['current'] = {'text': default, 'value': default}
    return value


def dashboard():
    selector = "SELECT DISTINCT COALESCE(CAST(json_extract(payload,'$.%s') AS TEXT),'unknown') FROM records WHERE kind='run'"
    variables = [variable('issue', 'Issue', "SELECT '*' UNION " + selector % 'issue', '*'),
                 variable('profile', 'Profile', "SELECT '*' UNION " + selector % 'profile', '*'),
                 variable('run', 'Run · UTC', """SELECT run AS __value,
 COALESCE(json_extract(payload,'$.started_at'),'unknown time')||' · '||run AS __text
 FROM records WHERE kind='run'
 AND (${issue:sqlstring}='*' OR COALESCE(CAST(json_extract(payload,'$.issue') AS TEXT),'unknown')=${issue:sqlstring})
 AND (${profile:sqlstring}='*' OR COALESCE(json_extract(payload,'$.profile'),'unknown')=${profile:sqlstring})
 AND (json_extract(payload,'$.started_at') IS NULL OR
 unixepoch(json_extract(payload,'$.started_at'))*1000 BETWEEN ${__from} AND ${__to})
 ORDER BY json_extract(payload,'$.started_at') DESC,run"""),
                 variable('group', 'Expand group', f"SELECT '__none' UNION SELECT node_id FROM node_states WHERE run_id={RUN} AND type!='test-file' ORDER BY 1", '__none'),
                 variable('node', 'Inspect node', f"SELECT '__none' UNION SELECT node_id FROM node_states WHERE run_id={RUN} ORDER BY 1", '__none')]
    variables[2]['refresh'] = 2
    workflow = panel(3, 'Workflow', 4, 12, NODES)
    workflow.update(type='nodeGraph', targets=[target(NODES, 'nodes'), target(EDGES, 'edges')],
                    options={'layoutAlgorithm': 'layered', 'zoomMode': 'cooperative'})
    base = '/d/storyos-run?${issue:queryparam}&${profile:queryparam}&${run:queryparam}&from=${__from}&to=${__to}'
    workflow['fieldConfig'] = {'defaults': {}, 'overrides': [{'matcher': {'id': 'byName', 'options': 'id'},
        'properties': [{'id': 'links', 'value': [
        {'title': 'Inspect node and expand its files', 'targetBlank': True, 'url': base + '&var-node=${__data.fields.id:percentencode}&var-group=${__data.fields.id:percentencode}'},
        {'title': 'Collapse files', 'url': base + '&var-node=${__data.fields.id:percentencode}&var-group=__none'}]}]}]}
    help_panel = {'id': 1, 'title': 'Read the retained workflow', 'type': 'text',
        'gridPos': {'x': 0, 'y': 16, 'w': 24, 'h': 5}, 'options': {'mode': 'markdown', 'content':
        'Select a time range, Issue, profile, and run. Click a node to inspect it and expand its file members. '
        'Choose `__none` to collapse files. Refresh preserves these URL selections and the layered layout.\n\n'
        '**States:** green passed · red failed · blue running · orange blocked · yellow pending · purple interrupted · gray not-selected · pale unknown/cached (see label). '
        '**Edges:** dependency = required order; contains/member = grouping only. File membership does not prove execution or timing. '
        '**Times:** UTC; unknown is not zero. A running label is the last retained state; check the heartbeat.'}}
    requests = f"SELECT json_extract(payload,'$.utc') AS UTC,json_extract(payload,'$.outcome') AS outcome,path AS evidence FROM records WHERE kind='request' AND json_extract(payload,'$.run_id')={RUN} ORDER BY UTC"
    return {'uid': 'storyos-run', 'title': 'StoryOS · Verification run', 'schemaVersion': 39,
        'editable': False, 'timezone': 'utc', 'refresh': '5s', 'time': {'from': 'now-7d', 'to': 'now'},
        'templating': {'list': variables}, 'panels': [panel(2, 'Run', 0, 4, SUMMARY), workflow, help_panel,
        panel(4, 'Node', 21, 7, DETAIL), panel(5, 'Node attempts · UTC', 28, 8, ATTEMPTS),
        panel(6, 'Requests · Reuse starts no new execution', 36, 6, requests)]}


if __name__ == '__main__':
    rendered = json.dumps(dashboard(), indent=2) + '\n'
    if '--check' in sys.argv:
        if not DESTINATION.exists() or DESTINATION.read_text() != rendered:
            raise SystemExit('Run make observe-dashboard to update the generated dashboard')
    else:
        DESTINATION.write_text(rendered)

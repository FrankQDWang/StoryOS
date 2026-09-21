"""Generate the local run dashboard from reusable read-only queries."""

import json
from pathlib import Path
import sys


DESTINATION = Path(__file__).with_name('observation') / 'dashboards/run.json'
SOURCE = {'type': 'frser-sqlite-datasource', 'uid': 'storyos-verification'}
RUN = '${run:sqlstring}'
GROUP = '${group:sqlstring}'
NODE = '${node:sqlstring}'
STATES = {'passed': '通过', 'failed': '失败', 'running': '运行中', 'blocked': '被阻塞',
          'pending': '待执行', 'interrupted': '已中断', 'cached': '复用', 'not-selected': '未选择', 'unknown': '未知'}
STEPS = {'setup': '准备环境', 'build': '构建', 'tests': '测试', 'cleanup': '清理环境', 'optional': '可选检查',
         'input-ownership': '输入归属', 'project-inputs': '项目输入', 'verification-tests': '验证工具测试',
         'workspace-boundaries': '工作区边界', 'rust-format': 'Rust 格式', 'rust-clippy': 'Rust 静态检查',
         'rust-test-build': '编译测试', 'rust-tests': 'Rust 测试', 'rust-doc-tests': 'Rust 文档测试',
         'rust-release-build': '发布构建', 'generated-contracts': '生成契约', 'contracts': '契约验证',
         'node-install': '安装依赖', 'web-typecheck': 'Web 类型检查', 'web-build': 'Web 构建',
         'release-package': '发布包', 'project-scope': '项目范围', 'database-setup': '准备数据库',
         'database-cleanup': '清理数据库', 'exact-dist-host': '启动成品服务', 'exact-dist-reset': '重置成品环境',
         'exact-dist-cleanup': '清理成品环境', 'browser-source': '源码浏览器测试',
         'browser-exact-dist': '成品浏览器测试', 'verify-tracker': '票据检查', 'observation-smoke': '观测验收',
         'node-contract': 'Node 契约测试', 'node-postgresql': 'PostgreSQL 测试', 'node-process-cut': '进程中断测试',
         'recovery': '恢复验证', 'recovery-drill': '恢复演练', 'policy': '验证策略', 'verification-tools': '验证工具',
         'author-edit-policy': '写入策略', 'author-edit-process-cut': '写入故障', 'author-edit-self-test': '写入自检',
         'cargo': 'Rust 工作区', 'cargo:storyos-adapter-postgres': '数据库适配', 'cargo:storyos-application': '应用层测试',
         'cargo:storyos-contracts': '契约层测试', 'cargo:storyos-core': '核心层测试', 'cargo:storyos-server': '服务层测试',
         'daily-database': '日常数据库', 'database': '数据库测试', 'exact-dist': '成品验证', 'foundation-tests': '基础测试',
         'http-files': 'HTTP 文件', 'pending': '待执行分组', 'persistence-self-test': '持久化自检',
         'postgres-challenge': '数据库挑战', 'postgres-library': '数据库藏书', 'postgres-scope': '数据库范围',
         'project-export-process-cut': '项目导出故障', 'protocol-self-test': '协议自检',
         'readable-export-process-cut': '文本导出故障', 'recovery-check': '恢复检查', 'recovery-fixture': '恢复样本',
         'recovery-mixed': '混合恢复', 'shared-tests': '共享测试', 'tracker-self-test': '票据自检',
         'transaction-guards': '事务边界', 'transaction-self-test': '事务自检', 'http-bootstrap': 'HTTP 启动',
         'http-main': 'HTTP 主流程', 'verification-node-tests': '执行记录测试',
         'verification-observation-tests': '观测测试', 'verify-contract-inputs': '契约输入', 'verify-policy': '策略检查',
         'verify-pr': 'PR 检查', 'web': 'Web 验证', 'web-foundation': 'Web 基础'}
STEP_KEY = "REPLACE(REPLACE(REPLACE(REPLACE(node_id,'check:',''),'phase:',''),'prepare:',''),'targeted:','')"
STEP_NAME = "CASE " + STEP_KEY + " " + ' '.join(f"WHEN '{key}' THEN '{value}'" for key,value in STEPS.items()) + " ELSE COALESCE(SUBSTR(path,LENGTH(RTRIM(path,REPLACE(path,'/','')))+1),REPLACE(node_id,'check:','')) END"
STATE_NAME = 'CASE state ' + ' '.join(f"WHEN '{key}' THEN '{value}'" for key,value in STATES.items()) + ' ELSE state END'
GRAPH = f"""WITH RECURSIVE
 g AS (SELECT * FROM run_graphs WHERE run_id={RUN}),
 n AS MATERIALIZED (SELECT s.*,j.value AS definition FROM node_states s JOIN g USING(run_id),
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
LAYOUT = f""", levels(id,depth) AS (
 SELECT node_id,0 FROM n UNION SELECT e.target,l.depth+1 FROM levels l JOIN e ON e.source=l.id
 WHERE l.depth<(SELECT COUNT(*) FROM n)),
 ranked AS (SELECT n.*,MAX(levels.depth) AS depth FROM n JOIN levels ON levels.id=n.node_id GROUP BY node_id),
 step_positions AS (SELECT node_id,depth*180 AS x,
 (ROW_NUMBER() OVER (PARTITION BY depth ORDER BY EXISTS(SELECT 1 FROM e WHERE source=ranked.node_id AND kind='dependency') DESC,node_id)-1)*125 AS y FROM ranked WHERE type!='test-file'),
 positions AS (SELECT * FROM step_positions UNION ALL SELECT node_id,
 COALESCE((SELECT x FROM step_positions WHERE node_id={GROUP}),0)+((ROW_NUMBER() OVER (ORDER BY node_id)-1)%4)*180,
 (SELECT MAX(y) FROM step_positions)+125+((ROW_NUMBER() OVER (ORDER BY node_id)-1)/4)*125
 FROM visible WHERE type='test-file')
"""
NODES = GRAPH + LAYOUT + f"""SELECT node_id AS id,
 COALESCE(path,REPLACE(node_id,'check:','')) AS title,
 {STEP_NAME} AS mainstat,
 CASE WHEN node_id LIKE 'phase:%' THEN '阶段' WHEN node_id LIKE 'prepare:%' THEN '重置'
 WHEN node_id LIKE 'targeted:%' THEN '针对性验证' ELSE type END AS subtitle,{STATE_NAME}||' · '||COALESCE(CAST(ROUND(duration_seconds,2) AS TEXT)||' s','—') AS secondarystat,
 44 AS nodeRadius,x-(SELECT MAX(x) FROM positions JOIN n USING(node_id) WHERE type!='test-file')/2 AS fixedX,
 y-(SELECT MAX(y) FROM positions JOIN n USING(node_id) WHERE type!='test-file')/2-85 AS fixedY,
 CASE state WHEN 'passed' THEN '#73BF69' WHEN 'failed' THEN '#F2495C'
 WHEN 'running' THEN '#5794F2' WHEN 'blocked' THEN '#FF9830' WHEN 'pending' THEN '#FADE2A'
 WHEN 'interrupted' THEN '#B877D9' WHEN 'cached' THEN '#8AB8FF'
 WHEN 'not-selected' THEN '#6E7681' ELSE '#CCCCDC' END AS color
 FROM visible JOIN positions USING(node_id) ORDER BY node_id"""
EDGES = GRAPH + """SELECT id,source,target,kind AS mainstat,rule AS detail__rule,
 CASE kind WHEN 'dependency' THEN '#64748B' ELSE '#CBD5E1' END AS color,
 CASE kind WHEN 'dependency' THEN 2 ELSE 1 END AS thickness
 FROM e WHERE source IN (SELECT node_id FROM visible) AND target IN (SELECT node_id FROM visible)
 ORDER BY id"""
SUMMARY = f"""SELECT json_extract(r.payload,'$.issue') AS issue,
 json_extract(r.payload,'$.profile') AS profile,json_extract(r.payload,'$.status') AS state,
 (SELECT SUM(selected) FROM node_states WHERE run_id=r.run) AS selected,
 CASE WHEN g.run_id IS NOT NULL THEN (SELECT COUNT(DISTINCT node_id) FROM node_attempts WHERE run_id=r.run) END AS executed,
 CASE WHEN g.run_id IS NOT NULL THEN (SELECT COUNT(*) FROM node_states WHERE run_id=r.run) END AS total,
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
    labels = {'issue': '任务', 'profile': '验证模式', 'state': '状态', 'selected': '已选择', 'executed': '已执行',
        'total': '总节点', 'graph': '流程数据', 'run_id': '运行 ID', 'actual_start_UTC': '开始时间 UTC',
        'end_UTC': '结束时间 UTC', 'source': '源码身份', 'retained_report': '保留报告', 'graph_sha256': '流程指纹',
        'heartbeat_UTC': '最近更新 UTC', 'node_id': '步骤 ID', 'type': '类型', 'file': '文件',
        'membership': '执行归属', 'selection_reason': '选择原因', 'prerequisites': '先决步骤', 'downstream': '后续步骤',
        'members': '分组文件', 'producer': '复用来源', 'attempt_id': '执行 ID', 'parent': '父级', 'result': '结果',
        'execution_scope': '执行范围', 'start_UTC': '开始时间 UTC', 'seconds': '耗时（秒）',
        'retained_evidence': '证据路径', 'outcome': '请求结果', 'evidence': '证据路径'}
    overrides = [{'matcher': {'id': 'byName', 'options': name},
                  'properties': [{'id': 'displayName', 'value': label}]} for name,label in labels.items()]
    mappings = {key: {'text': value} for key,value in {**STATES, 'complete': '完整验证', 'targeted': '针对性验证',
        'daily': '日常验证', 'retained': '已保留', 'unavailable': '不可用', 'unknown / not ended': '未知 / 尚未结束',
        'not a file': '非文件节点', 'none recorded': '无记录'}.items()}
    return {'id': number, 'title': title, 'type': 'table', 'datasource': SOURCE,
            'gridPos': {'x': 0, 'y': y, 'w': 24, 'h': height}, 'targets': [target(sql)],
            'fieldConfig': {'defaults': {'noValue': '未知', 'mappings': [{'type': 'value', 'options': mappings}],
                'custom': {'cellOptions': {'type': 'auto'}, 'wrapText': False, 'inspect': True}}, 'overrides': overrides},
            'options': {'showHeader': True, 'cellHeight': 'sm'}}


def variable(name, label, query, default=None):
    value = {'name': name, 'label': label, 'type': 'query', 'datasource': SOURCE,
             'query': query, 'refresh': 1, 'sort': 0, 'multi': False, 'includeAll': False}
    if default is not None:
        value['current'] = {'text': '未选择' if default=='__none' else default, 'value': default}
    return value


def dashboard():
    selector = "SELECT DISTINCT COALESCE(CAST(json_extract(payload,'$.%s') AS TEXT),'unknown') FROM records WHERE kind='run'"
    variables = [variable('issue', '任务', "SELECT '*' UNION " + selector % 'issue', '*'),
                 variable('profile', '验证模式', "SELECT '*' UNION " + selector % 'profile', '*'),
                 variable('run', '运行 · UTC', """SELECT run AS __value,
 COALESCE(json_extract(payload,'$.started_at'),'unknown time')||' · '||run AS __text
 FROM records WHERE kind='run'
 AND (${issue:sqlstring}='*' OR COALESCE(CAST(json_extract(payload,'$.issue') AS TEXT),'unknown')=${issue:sqlstring})
 AND (${profile:sqlstring}='*' OR COALESCE(json_extract(payload,'$.profile'),'unknown')=${profile:sqlstring})
 AND (json_extract(payload,'$.started_at') IS NULL OR
 unixepoch(json_extract(payload,'$.started_at'))*1000 BETWEEN ${__from} AND ${__to})
 ORDER BY json_extract(payload,'$.started_at') DESC,run"""),
                 variable('group', '展开分组', f"SELECT '__none' AS __value,'未选择' AS __text UNION SELECT node_id,REPLACE(node_id,'check:','') FROM node_states WHERE run_id={RUN} AND type!='test-file' ORDER BY 1", '__none'),
                 variable('node', '查看步骤', f"SELECT '__none' AS __value,'未选择' AS __text UNION SELECT node_id,REPLACE(node_id,'check:','') FROM node_states WHERE run_id={RUN} ORDER BY 1", '__none')]
    variables[2]['refresh'] = 2
    workflow = panel(3, '验证流程 · 步骤与依赖', 4, 13, NODES)
    workflow.update(type='nodeGraph', targets=[target(NODES, 'nodes'), target(EDGES, 'edges')],
                    options={'layoutAlgorithm': 'layered', 'zoomMode': 'cooperative'})
    base = '/d/storyos-run?${issue:queryparam}&${profile:queryparam}&${run:queryparam}&from=${__from}&to=${__to}'
    workflow['fieldConfig'] = {'defaults': {}, 'overrides': [{'matcher': {'id': 'byName', 'options': 'id'},
        'properties': [{'id': 'links', 'value': [
        {'title': '查看步骤与文件', 'targetBlank': True, 'url': base + '&theme=light&var-node=${__data.fields.id:percentencode}&var-group=${__data.fields.id:percentencode}'},
        {'title': '收起文件', 'url': base + '&theme=light&var-node=${__data.fields.id:percentencode}&var-group=__none'}]}]}]}
    workflow['fieldConfig']['overrides'].extend(
        {'matcher': {'id': 'byName', 'options': name}, 'properties': [{'id': 'displayName', 'value': label}]}
        for name,label in [('mainstat', '步骤'), ('secondarystat', '状态 · 耗时'), ('color', '状态色')])
    workflow['gridPos']['w'] = 16
    steps = panel(7, '步骤清单 · 按依赖顺序', 4, 13, GRAPH + LAYOUT + f"""SELECT
 {STEP_NAME} AS step,{STATE_NAME} AS state,node_id
 FROM visible JOIN positions USING(node_id) WHERE type!='test-file' ORDER BY x,y,node_id""")
    steps['gridPos'].update(x=16, w=8)
    steps['fieldConfig']['overrides'].extend([
        {'matcher': {'id': 'byName', 'options': 'step'}, 'properties': [
            {'id': 'displayName', 'value': '步骤'}, {'id': 'links', 'value': [
                {'title': '查看步骤', 'url': base + '&var-node=${__data.fields.node_id:percentencode}&var-group=__none'}]}]},
        {'matcher': {'id': 'byName', 'options': 'node_id'}, 'properties': [{'id': 'custom.hidden', 'value': True}]}])
    help_panel = {'id': 1, 'title': '阅读提示', 'type': 'text',
        'gridPos': {'x': 0, 'y': 17, 'w': 24, 'h': 3}, 'options': {'mode': 'markdown', 'content':
        '圆心是步骤，第二行是状态与耗时。实线箭头表示先决依赖，细线表示分组归属。点击步骤查看详情；将展开分组设为「未选择」即可收起文件。 '
        '绿色通过 · 红色失败 · 蓝色运行中 · 橙色阻塞 · 黄色待执行 · 灰色未选择。时间为 UTC；— 表示未记录，未知不等于零。运行中是最后记录的状态，请结合「最近更新 UTC」判断是否仍活跃。'}}
    requests = f"SELECT json_extract(payload,'$.utc') AS UTC,json_extract(payload,'$.outcome') AS outcome,path AS evidence FROM records WHERE kind='request' AND json_extract(payload,'$.run_id')={RUN} ORDER BY UTC"
    return {'uid': 'storyos-run', 'title': 'StoryOS · 验证运行', 'schemaVersion': 39,
        'editable': False, 'timezone': 'utc', 'refresh': '5s', 'time': {'from': 'now-7d', 'to': 'now'},
        'templating': {'list': variables}, 'panels': [panel(2, '运行概览', 0, 4, SUMMARY), workflow, steps, help_panel,
        panel(4, '步骤详情', 20, 5, DETAIL), panel(5, '执行记录 · UTC', 25, 6, ATTEMPTS),
        panel(6, '请求记录 · 复用不会启动执行', 31, 5, requests)]}


if __name__ == '__main__':
    rendered = json.dumps(dashboard(), indent=2) + '\n'
    if '--check' in sys.argv:
        if not DESTINATION.exists() or DESTINATION.read_text() != rendered:
            raise SystemExit('Run make observe-dashboard to update the generated dashboard')
    else:
        DESTINATION.write_text(rendered)

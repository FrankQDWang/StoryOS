"""Generate read-only comparison queries from retained run snapshots."""

from verification_observation_dashboard import DESTINATION, STEPS, panel, variable

DESTINATION = DESTINATION.with_name('compare.json')
LEFT, RIGHT = '${left:sqlstring}', '${right:sqlstring}'
PAIR = f"""WITH pair(side,run_id) AS (VALUES ('left',{LEFT}),('right',{RIGHT})),
 roots AS MATERIALIZED (SELECT p.*,r.payload,r.quality,g.payload AS graph FROM pair p
 LEFT JOIN records r ON r.run=p.run_id AND r.kind='run'
 LEFT JOIN run_graphs g ON g.run_id=p.run_id),
 definitions AS (SELECT side,json_extract(j.value,'$.id') AS node_id,
 json_remove(j.value,'$.selected') AS definition FROM roots,json_each(graph,'$.nodes') j),
 edges AS MATERIALIZED (SELECT side,j.value AS edge FROM roots,json_each(graph,'$.dependencies') j
 UNION ALL SELECT side,j.value FROM roots,json_each(graph,'$.relations') j),
 incident AS (SELECT side,json_extract(edge,'$.from') AS node_id,edge FROM edges
 UNION SELECT side,json_extract(edge,'$.to'),edge FROM edges),
 edge_sets AS MATERIALIZED (SELECT side,node_id,group_concat(edge) AS edges
 FROM (SELECT * FROM incident ORDER BY side,node_id,edge) GROUP BY side,node_id),
 counts AS MATERIALIZED (SELECT run_id,node_id,COUNT(*) AS executed FROM node_attempts
 WHERE run_id IN (SELECT run_id FROM pair) GROUP BY run_id,node_id),
 n AS MATERIALIZED (SELECT d.*,s.selected,s.state,s.producer,COALESCE(c.executed,0) AS executed,e.edges
 FROM definitions d JOIN pair p USING(side) JOIN node_states s USING(run_id,node_id)
 LEFT JOIN counts c USING(run_id,node_id) LEFT JOIN edge_sets e USING(side,node_id))
"""
DIFFERENCE = PAIR + """SELECT ids.node_id,
 CASE WHEN (SELECT COUNT(graph) FROM roots)!=2 THEN 'unavailable'
 WHEN l.node_id IS NULL THEN 'only-right' WHEN r.node_id IS NULL THEN 'only-left'
 WHEN l.definition!=r.definition OR l.edges IS NOT r.edges THEN 'changed-definition' ELSE 'common' END AS difference,
 l.selected AS left_selected,r.selected AS right_selected,
 l.executed AS left_attempts,r.executed AS right_attempts,
 l.state AS left_state,r.state AS right_state,l.producer AS left_reuse,r.producer AS right_reuse,
 l.definition AS left_definition,r.definition AS right_definition,l.edges AS left_edges,r.edges AS right_edges
 FROM (SELECT DISTINCT node_id FROM n) ids
 LEFT JOIN n l ON l.node_id=ids.node_id AND l.side='left'
 LEFT JOIN n r ON r.node_id=ids.node_id AND r.side='right' ORDER BY difference,ids.node_id"""
COMPARISON = PAIR + """SELECT
 CASE WHEN l.graph IS NULL OR r.graph IS NULL THEN 'not comparable'
 WHEN json_extract(l.payload,'$.status')='passed' AND json_extract(r.payload,'$.status')='passed'
 AND json_extract(l.payload,'$.attempt_started')=1 AND json_extract(r.payload,'$.attempt_started')=1
 AND json_type(l.payload,'$.repository')='text' AND LENGTH(json_extract(l.payload,'$.repository'))>0
 AND json_type(r.payload,'$.repository')='text' AND LENGTH(json_extract(r.payload,'$.repository'))>0
 AND EXISTS (SELECT 1 FROM n WHERE executed>0)
 AND json_extract(l.payload,'$.comparison_key') IS NOT NULL AND
 json_extract(l.payload,'$.comparison_key')=json_extract(r.payload,'$.comparison_key')
 AND NOT EXISTS (SELECT node_id,definition,edges,selected,executed FROM n WHERE side='left'
 EXCEPT SELECT node_id,definition,edges,selected,executed FROM n WHERE side='right')
 AND NOT EXISTS (SELECT node_id,definition,edges,selected,executed FROM n WHERE side='right'
 EXCEPT SELECT node_id,definition,edges,selected,executed FROM n WHERE side='left') THEN 'comparable evidence'
 ELSE 'descriptive only' END AS comparison,
 json_extract(r.payload,'$.duration_seconds')-json_extract(l.payload,'$.duration_seconds') AS delta_seconds,
 '仅在范围、工具链、主机与构建状态均有匹配证据时可比较；差值本身不能证明原因。' AS limitation
 FROM roots l,roots r WHERE l.side='left' AND r.side='right'"""
SUMMARY = PAIR + """SELECT side,json_extract(payload,'$.profile') AS profile,
 json_extract(payload,'$.status') AS state,
 (SELECT SUM(selected) FROM n WHERE n.side=roots.side) AS selected,
 CASE WHEN graph IS NOT NULL THEN (SELECT COUNT(*) FROM n WHERE n.side=roots.side AND executed>0) END AS executed,
 CASE WHEN graph IS NOT NULL THEN (SELECT COUNT(*) FROM n WHERE n.side=roots.side AND state='cached') END AS reused,
 CASE WHEN graph IS NULL THEN 'unavailable' ELSE 'retained' END AS graph,
 json_extract(payload,'$.duration_seconds') AS seconds,run_id,quality,json_extract(payload,'$.build_state') AS build_state,
 json_extract(payload,'$.recovery_of') AS recovery_of,json_extract(payload,'$.reason') AS reason,
 (SELECT COUNT(*) FROM records q WHERE q.kind='request' AND q.quality='valid'
 AND json_extract(q.payload,'$.run_id')=roots.run_id AND json_extract(q.payload,'$.outcome')='reused') AS reuse_requests
 FROM roots"""


INTERVALS = PAIR + """, attempts AS (
 SELECT p.side,a.*,ROW_NUMBER() OVER (PARTITION BY p.side,a.node_id ORDER BY a.started_at,a.attempt_id) AS attempt_number
 FROM pair p JOIN node_attempts a USING(run_id)),
 timed AS (SELECT *,ROUND((julianday(started_at)-2440587.5)*86400,3) AS start,
 ROUND((julianday(ended_at)-2440587.5)*86400,3) AS end FROM attempts),
 waits AS (SELECT side,j.key,
 ROUND((julianday(json_extract(payload,'$.started_at'))-2440587.5)*86400,3)
 +json_extract(j.value,'$[0]')-json_extract(payload,'$.started_monotonic') AS start,
 ROUND((julianday(json_extract(payload,'$.started_at'))-2440587.5)*86400,3)
 +json_extract(j.value,'$[1]')-json_extract(payload,'$.started_monotonic') AS end
 FROM roots,json_each(payload,'$.blocked_intervals') j WHERE quality='valid')
"""
STEP = "CASE REPLACE(node_id,'check:','') " + ' '.join(f"WHEN '{key}' THEN '{value}'" for key,value in STEPS.items()) + ' ELSE node_id END'
TIMELINE = INTERVALS + f"""SELECT start,end,CASE side WHEN 'left' THEN '左' ELSE '右' END||' · '||{STEP}||' · '||
 attempt_number AS lane,result AS state
 FROM timed WHERE start IS NOT NULL AND end>=start
 UNION ALL SELECT start,end,CASE side WHEN 'left' THEN '左' ELSE '右' END||' · 已测量等待 · '||key,'measured wait'
 FROM waits WHERE start IS NOT NULL AND end>=start ORDER BY start,lane"""
ATTEMPTS = INTERVALS + """SELECT side,node_id,attempt_number,attempt_id,result,started_at AS start_UTC,ended_at AS end_UTC,
 duration_seconds AS seconds,
 CASE WHEN start IS NOT NULL AND end>=start THEN
 (SELECT COUNT(DISTINCT json_array(b.run_id,b.attempt_id)) FROM timed b WHERE (b.run_id!=a.run_id OR b.attempt_id!=a.attempt_id)
 AND b.start<b.end AND a.start<a.end AND b.start<a.end AND b.end>a.start) END AS overlaps,
 selection_reason,execution_scope FROM timed a ORDER BY start,side,node_id,attempt_id"""


def dashboard():
    query = """SELECT run AS __value,COALESCE(json_extract(payload,'$.profile'),'unknown')||' · '||
 COALESCE(json_extract(payload,'$.started_at'),'unknown time')||' · '||run AS __text
 FROM records WHERE kind='run' ORDER BY COALESCE(unixepoch(json_extract(payload,'$.started_at')),0) DESC,run"""
    panels = [panel(1, '比较条件 · 差值不等于提速原因', 0, 4, COMPARISON),
              panel(2, '两轮运行 · 选择 / 执行 / 复用', 4, 5, SUMMARY),
              panel(3, '范围差异 · 按稳定 ID 保留增删与定义变化', 9, 10, DIFFERENCE)]
    timeline = panel(4, '实际执行时间线 · UTC · 每次尝试独立一行', 19, 12, TIMELINE)
    timeline['type'] = 'state-timeline'
    timeline['targets'][0]['timeColumns'] = ['start', 'end']
    timeline['transformations'] = [{'id': 'partitionByValues', 'options': {'fields': ['lane'], 'keepFields': False}}]
    timeline['fieldConfig']['defaults'].update(displayName='${__field.labels.lane}',
        color={'mode': 'fixed', 'fixedColor': 'blue'})
    timeline['options'] = {'mergeValues': False, 'rowHeight': 0.8, 'showValue': 'auto',
        'alignValue': 'left', 'axisWidth': 240, 'legend': {'showLegend': False}, 'tooltip': {'mode': 'single'}, 'perPage': 12}
    panels.extend([timeline, panel(5, '尝试详情 · 重叠数仅包含已知区间', 31, 7, ATTEMPTS)])
    panels.append({'id': 6, 'title': '如何阅读', 'type': 'text', 'gridPos': {'x': 0, 'y': 38, 'w': 24, 'h': 4},
        'options': {'mode': 'markdown', 'content': '按稳定 ID 比较；改名保留为删除与新增。选择、执行次数与复用分开显示。'
        '时间线只画有实际起止时间的尝试；缺失文件计时与未结束尝试保持未知，详见尝试详情。'
        '同一步骤的多次尝试各占一行，重叠数包括父子嵌套，不是额外费用。等待仅来自显式记录的区间，按运行 UTC 与单调时钟锚点换算；没有记录不表示零。'
        '历史范围不可用时不推测差异。耗时差为右减左；不同模式、主机、工具链或未知构建状态仅作描述。'}})
    labels = {'side': '侧', 'comparison': '可比性', 'delta_seconds': '右减左（秒）', 'limitation': '判定边界',
        'difference': '范围差异', 'left_selected': '左选择', 'right_selected': '右选择',
        'left_attempts': '左执行次数', 'right_attempts': '右执行次数', 'left_state': '左状态', 'right_state': '右状态',
        'left_reuse': '左复用来源', 'right_reuse': '右复用来源', 'left_definition': '左定义', 'right_definition': '右定义',
        'left_edges': '左依赖与归属', 'right_edges': '右依赖与归属', 'reused': '已复用', 'recovery': '恢复验证', 'quality': '记录质量',
        'build_state': '构建状态', 'recovery_of': '恢复自', 'reason': '原因', 'reuse_requests': '复用请求（无新增执行）', 'overlaps': '重叠次数', 'attempt_number': '尝试序号'}
    values = {'left': '左', 'right': '右', 'common': '相同定义', 'only-left': '仅左侧', 'only-right': '仅右侧',
        'changed-definition': '定义变化', 'descriptive only': '仅作描述', 'not comparable': '不可比较',
        'recovery': '恢复验证', 'comparable evidence': '可比证据齐全', 'valid': '有效', 'legacy': '历史', 'measured wait': '已测量等待'}
    for item in panels:
        if item['type']=='text':
            continue
        item['fieldConfig']['defaults']['mappings'][0]['options'].update({key: {'text': value} for key,value in values.items()})
        item['fieldConfig']['overrides'].extend({'matcher': {'id': 'byName', 'options': name},
            'properties': [{'id': 'displayName', 'value': label}]} for name,label in labels.items())
        if item['type']=='table':
            item['fieldConfig']['defaults']['custom'].update(filterable=True, minWidth=90)
            item['fieldConfig']['overrides'].append({'matcher': {'id': 'byName', 'options': 'node_id'},
                'properties': [{'id': 'custom.width', 'value': 300}, {'id': 'custom.wrapText', 'value': True}]})
    timeline['fieldConfig']['overrides'] = []
    panels[1]['fieldConfig']['overrides'].append({'matcher': {'id': 'byName', 'options': 'run_id'},
        'properties': [{'id': 'links', 'value': [{'title': '查看完整运行 DAG',
            'url': '/d/storyos-run?theme=light&var-run=${__value.raw:percentencode}'}]}]})
    return {'uid': 'storyos-compare', 'title': 'StoryOS · 运行比较', 'schemaVersion': 39,
        'editable': False, 'timezone': 'utc', 'refresh': '5s', 'time': {'from': 'now-7d', 'to': 'now'},
        'templating': {'list': [variable('left', '左侧运行', query), variable('right', '右侧运行', query)]},
        'panels': panels}

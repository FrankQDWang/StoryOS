function analysisLinks(run,right='') {
    const id=encodeURIComponent(run);
    return '<p class="muted">完整图和时间线在新标签打开；关闭标签后返回当前筛选与阅读位置。</p><p><a target="_blank" rel="noopener" href="/d/storyos-run?var-run='+id+'&theme=light">打开可缩放执行图与时间线 ↗</a></p>'+
        (right?'<p><a target="_blank" rel="noopener" href="/d/storyos-compare?var-left='+id+'&var-right='+encodeURIComponent(right)+'&theme=light">打开完整两轮比较 ↗</a></p>':'');
}
function costView(model) {
    const bill=model.cost, root=bill?.root, issue=bill?.issue;
    if(!root)return '<p class="notice">此记录没有实际根启动或成本事实。被拒绝与复用不产生新根费用。</p>';
    const stageNames={concurrent:'并发区间',unclassified:'未归类区间'};
    const stages=rows=>rows.length?rows.map(row=>'<div class="stage"><strong>'+escapeText(stageNames[row.stage]||row.stage)+'</strong><span>'+duration(row.seconds)+'</span></div>').join(''):'<p class="muted">阶段耗时未知</p>';
    return '<p class="notice">成本是独占墙钟时间；父子阶段不相加。并发区间只计一次，未归类区间单列。</p><dl><dt>本轮根耗时</dt><dd>'+duration(root.seconds)+'</dd><dt>本轮阻塞等待</dt><dd>'+duration(root.blocked_seconds)+' · '+(root.blocked_seconds==null?'未测量':'显式测量')+'</dd></dl><h3>本轮独占阶段</h3>'+stages(bill.stages)+
        (issue?'<h3>任务 '+escapeText(issue.id)+' · 实际启动成本</h3>'+issue.profiles.map(row=>'<div class="stage"><strong>'+escapeText(profiles[row.profile]||row.profile||'历史记录')+' · '+escapeText(states[row.status]||row.status)+'</strong><span>'+row.attempts+' 次 · '+duration(row.seconds)+'</span></div>').join('')+
        '<h3>任务独占阶段</h3>'+stages(issue.stages)+'<h3>请求与复用</h3>'+issue.requests.map(row=>'<div class="stage"><strong>'+escapeText(row.outcome)+'</strong><span>'+row.requests+' 次请求</span></div>').join('')+
        '<p class="notice">任务阻塞等待：'+duration(issue.blocked_seconds)+' · '+(issue.blocked_seconds==null?'缺少共同阻塞时钟或区间；不得按模式行相加':'已按共同阻塞时钟取并集；不要按模式行相加')+'。</p>':'<p class="notice">未归属运行不合并为单个任务费用。</p>');
}
function graphView(model,run) {
    const graph=model.graph?.graph;
    if(!graph)return '<p class="notice">此历史记录没有保留图；范围和节点时间保持未知。</p>';
    const statesById=new Map(model.graph.states.map(row=>[row.node_id,row]));
    const attempted=new Set(model.attempts.map(row=>row.node_id));
    const nodes=graph.nodes.filter(node=>node.type!=='test-file'&&
        (model.graphFilter==='all'||model.graphFilter==='active'&&(statesById.get(node.id)?.selected||attempted.has(node.id))||
        model.graphFilter==='selected'&&statesById.get(node.id)?.selected||model.graphFilter==='attempted'&&attempted.has(node.id))&&
        (node.id+' '+(node.path||'')).toLowerCase().includes(model.graphSearch.toLowerCase()));
    return '<p class="notice">依赖边表示执行前提；包含和成员边仅表示归属。选择不等于实际执行。</p><div class="scope"><span>节点 '+graph.nodes.length+'</span><span>依赖 '+graph.dependencies.length+'</span><span>实际启动节点 '+attempted.size+'</span></div>'+analysisLinks(run)+
        '<h3>步骤与节点</h3><div class="toolbar wrap"><input aria-label="搜索图节点" placeholder="搜索步骤或节点 ID" data-graph-search value="'+escapeText(model.graphSearch)+'"><select aria-label="图节点范围" data-graph-filter>'+selectOptions([['active','已选择或已启动'],['selected','已选择'],['attempted','已启动'],['all','全部节点']],model.graphFilter)+'</select></div><p class="muted">'+nodes.length+' 个步骤 · 选择节点查看前提、成员与尝试</p>'+nodes.map(node=>{
            const state=statesById.get(node.id);
            return '<button class="file-row" data-node="'+escapeText(node.id)+'"><strong>'+escapeText(node.id.replace(/^check:/,''))+'</strong><span>'+escapeText(node.type)+' · '+escapeText(states[state?.state]||state?.state||'未知')+' · '+(state?.selected?'已选择':'未选择')+' · '+model.attempts.filter(a=>a.node_id===node.id).length+' 次启动</span></button>';
        }).join('');
}
function nodeView(model,nodeId) {
    const graph=model.graph?.graph, node=graph?.nodes.find(item=>item.id===nodeId);
    if(!node)return '<p class="notice">此节点没有保留定义。</p>';
    const state=model.graph.states.find(item=>item.node_id===nodeId);
    const attempts=model.attempts.filter(item=>item.node_id===nodeId);
    const prerequisites=graph.dependencies.filter(edge=>edge.to===nodeId).map(edge=>edge.from);
    const members=graph.relations.filter(edge=>edge.from===nodeId).map(edge=>edge.to);
    return '<dl><dt>节点</dt><dd>'+escapeText(node.id)+'</dd><dt>类型</dt><dd>'+escapeText(node.type)+'</dd><dt>状态</dt><dd>'+escapeText(states[state?.state]||state?.state||'未知')+'</dd><dt>选择</dt><dd>'+(state?.selected?'已选择':'未选择')+'</dd><dt>路径</dt><dd>'+escapeText(node.path)+'</dd><dt>复用来源</dt><dd>'+escapeText(state?.producer)+'</dd><dt>依赖前提</dt><dd>'+escapeText(prerequisites.join(' · ')||'无')+'</dd><dt>包含或成员</dt><dd>'+escapeText(members.join(' · ')||'无')+'</dd></dl><h3>实际尝试</h3>'+
        (attempts.length?attempts.map(row=>'<div class="stage"><strong>'+escapeText(row.attempt_id)+' · '+escapeText(states[row.result]||row.result)+'</strong><span>'+stamp(row.started_at)+' UTC · '+duration(row.duration_seconds)+'</span></div>').join(''):'<p class="notice">无逐节点尝试；不能推断已执行或零耗时。</p>');
}
function timelineView(model,run) {
    const attempts=[...model.attempts].sort((a,b)=>model.timelineSort==='latest'?String(b.started_at||'').localeCompare(String(a.started_at||'')):String(a.started_at||'').localeCompare(String(b.started_at||'')));
    return '<p class="notice">UTC 时间线只列实际尝试。结束未知的尝试保留在列表；重叠区间不是额外费用。</p>'+analysisLinks(run)+'<div class="toolbar"><select aria-label="时间线排序" data-timeline-sort>'+selectOptions([['start','最早开始'],['latest','最新开始']],model.timelineSort)+'</select></div>'+
        (attempts.length?attempts.map(row=>'<div class="timeline-row"><button data-node="'+escapeText(row.node_id)+'">'+escapeText(row.node_id)+' →</button><span>'+stamp(row.started_at)+' → '+stamp(row.ended_at)+' UTC</span><small>'+escapeText(states[row.result]||row.result)+' · '+duration(row.duration_seconds)+'</small></div>').join(''):'<p class="notice">没有保留实际节点尝试；时间未知。</p>');
}
function comparisonView(model,run,right) {
    const data=model.comparison, ready=right&&model.compareRight===right&&data;
    let content='<p class="notice">先选另一轮。范围、定义、执行和构建状态齐全之前，耗时差只作描述。</p><div class="toolbar"><input aria-label="查找另一轮运行" placeholder="搜索运行 ID、任务或类型" data-compare-query value="'+escapeText(model.compareSearch)+'"><button data-compare-search>查找</button></div>'+
        model.compareOptions.map(row=>'<button class="file-row" data-compare-run="'+escapeText(row.run)+'"><strong>'+escapeText(row.run)+'</strong><span>任务 '+escapeText(row.issue??'未归属')+' · '+escapeText(profiles[row.profile]||row.profile||'历史记录')+'</span></button>').join('');
    if(!ready)return content;
    const comparisonNames={'descriptive only':'仅作描述','not comparable':'范围不可比较','comparable evidence':'可比证据齐全'};
    const differenceNames={'common':'共同定义','only-left':'仅左侧','only-right':'仅右侧','changed-definition':'定义变化'};
    const quality=data.comparison.comparison, differences=data.differences.filter(row=>model.compareFilter==='all'||model.compareFilter==='changed'&&
        (row.difference!=='common'||row.left_selected!==row.right_selected||row.left_attempts!==row.right_attempts||row.left_state!==row.right_state)||row.difference===model.compareFilter);
    differences.sort((a,b)=>model.compareSort==='id'?a.node_id.localeCompare(b.node_id):a.difference.localeCompare(b.difference)||a.node_id.localeCompare(b.node_id));
    content+='<h3>比较条件先于耗时结论</h3><p><strong>'+escapeText(comparisonNames[quality]||quality)+'</strong></p>'+data.runs.map(row=>'<div class="stage"><strong>'+escapeText(row.side==='left'?'左':'右')+' · '+escapeText(row.run_id)+'</strong><span>选择 '+escapeText(row.selected)+' · 执行 '+escapeText(row.executed)+' · 构建 '+escapeText(row.build_state)+'</span></div>').join('')+'<p>描述性耗时差 · 右减左 '+duration(data.comparison.delta_seconds)+'</p><p class="notice">'+escapeText(data.comparison.limitation)+'</p>'+
        analysisLinks(run,right)+'<h3>定义、选择与执行差异</h3><div class="toolbar"><select aria-label="差异类别" data-compare-filter>'+selectOptions([['changed','有变化'],['all','全部节点'],['changed-definition','定义变化'],['only-left','仅左侧'],['only-right','仅右侧'],['common','共同定义']],model.compareFilter)+'</select><select aria-label="差异排序" data-compare-sort>'+selectOptions([['kind','类别排序'],['id','节点 ID 排序']],model.compareSort)+'</select></div><p class="muted">'+differences.length+' 个节点 · 改名按删除和新增显示</p>'+
        (differences.length?differences.map(row=>'<div class="difference"><strong>'+escapeText(row.node_id)+'</strong><small>'+escapeText(differenceNames[row.difference]||row.difference)+' · 左选择 '+escapeText(row.left_selected)+' / 右选择 '+escapeText(row.right_selected)+' · 左尝试 '+escapeText(row.left_attempts)+' / 右尝试 '+escapeText(row.right_attempts)+'</small><details><summary>定义与边</summary><pre>'+escapeText(JSON.stringify({left:row.left_definition,right:row.right_definition,left_edges:row.left_edges,right_edges:row.right_edges},null,2))+'</pre></details></div>').join(''):'<p class="notice">当前筛选没有变化节点；可查看全部节点或完整比较。</p>');
    return content;
}

const stamp = value => Number.isFinite(parseTime(value))?new Date(parseTime(value)).toISOString().slice(0,19).replace('T',' '):'未知';
const evidencePath = value => /^(?:[A-Za-z0-9][A-Za-z0-9_-]{0,127}\/report\.json|requests\/[A-Za-z0-9_-]+\.json)$/.test(value||'')?'target/verification/'+value:'未知';
function drawerView(model,view,heartbeat) {
    const titles={summary:'运行摘要',files:'文件范围',file:'文件证据',diagnostics:'诊断与证据'};
    const bar='<div class="detail-bar">'+(view.level==='summary'?'':'<button data-detail-back>'+(view.level==='file'?'返回文件列表':'返回运行摘要')+'</button>')+'<button data-close aria-label="关闭详情">关闭</button></div>';
    const content=model.root?drawerContent(model,view,heartbeat):'<p>读取保留证据…</p>';
    return bar+'<div class="detail-error" role="status" '+(model.error?'':'hidden')+'>'+escapeText(model.error||'')+'</div>'+
        '<div class="detail-update" '+(model.pending?'':'hidden')+'><button data-detail-update>证据有变化 · 更新范围</button></div><div class="detail-body"><div class="detail-title"><small>任务 '+escapeText(model.root?.record.issue??'未归属')+' / '+escapeText(profiles[model.root?.record.profile]||model.root?.record.profile||'历史记录')+
        '</small><h2 id="drawer-title">'+titles[view.level]+'</h2></div>'+content+'</div>';
}
function drawerContent(model,view,heartbeat) {
    const root=model.root, r=root.record, files=model.files, settings=model.settings;
    if(view.level==='summary') {
        const counts={all:files.length,selected:files.filter(f=>f.selected).length,started:files.filter(f=>model.fileFact(f).attempts.length).length,unknown:files.filter(f=>model.fileFact(f).state==='unknown').length};
        return '<div class="summary-state">'+badge(r,heartbeat)+'<strong>'+duration(elapsed(r,heartbeat))+'</strong></div><dl><dt>开始 · UTC</dt><dd>'+stamp(r.started_at)+'</dd><dt>结束 · UTC</dt><dd>'+stamp(r.ended_at)+'</dd><dt>触发原因 · 原文</dt><dd>'+escapeText(root.reason||'原记录未保留触发原因')+'</dd></dl><h3>范围摘要</h3><div class="scope">'+
            [['all','清单文件'],['selected','计划选择'],['started','有启动记录'],['unknown','状态未知']].map(([filter,label])=>'<button data-scope="'+filter+'"><b>'+escapeText(root.has_graph?counts[filter]:null)+'</b><span>'+label+'</span></button>').join('')+
            '</div><p class="notice">'+(root.has_graph?'缺少逐文件 attempt 时，实际执行与耗时保持未知。':'此历史记录未保留文件图，文件数量与范围未知。')+' 计划符合不等于选择最小。</p><h3>异常与不确定性</h3><p>'+escapeText(runState(r,heartbeat)==='stale'?'心跳已过期，实际进程存活未知。':r.status==='failed'?'本轮失败；展开诊断查看阶段证据。':'不能由阶段通过推断文件通过。请结合保留证据判断。')+'</p><button class="secondary" data-level="diagnostics">诊断与证据</button>';
    }
    if(view.level==='files') {
        if(!root.has_graph)return '<p class="notice">此历史记录未保留文件图，文件数量与选择范围未知。</p>';
        const candidates=files.filter(f=>f.path.toLowerCase().includes(settings.q.toLowerCase())&&
            (settings.filter==='all'||settings.filter==='selected'&&f.selected||settings.filter==='off'&&!f.selected||settings.filter==='started'&&model.fileFact(f).attempts.length||settings.filter===model.fileFact(f).state));
        candidates.sort((a,b)=>settings.sort==='duration'?(model.fileFact(b).seconds??-1)-(model.fileFact(a).seconds??-1)||a.path.localeCompare(b.path):a.path.localeCompare(b.path));
        if(model.visible==null)model.visible=candidates.map(f=>f.node_id);
        const visible=model.visible.map(id=>files.find(f=>f.node_id===id)).filter(Boolean);
        return '<div class="toolbar wrap"><input aria-label="搜索文件" placeholder="搜索文件路径" data-detail-setting="q" value="'+escapeText(settings.q)+'"><select aria-label="文件状态" data-detail-setting="filter">'+selectOptions([['all','全部文件'],['selected','计划选择'],['started','有启动记录'],['unknown','状态未知'],['off','未选择'],['failed','失败']],settings.filter)+
            '</select><select aria-label="文件排序" data-detail-setting="sort">'+selectOptions([['path','路径排序'],['duration','实测耗时最长']],settings.sort)+'</select></div><p class="muted">'+visible.length+' 个文件 · 点击查看保留证据</p>'+visible.map(f=>'<button class="file-row" data-file="'+escapeText(f.node_id)+'"><strong>'+escapeText(f.path)+'</strong><span>'+escapeText(f.node_id.split(':')[1])+' · '+(f.selected?'已选择':'未选择')+' · '+escapeText(states[model.fileFact(f).state]||model.fileFact(f).state)+' · '+duration(model.fileFact(f).seconds)+'</span></button>').join('');
    }
    if(view.level==='file') {
        const file=files.find(f=>f.node_id===view.file);
        if(!file)return '<p>此文件没有保留记录。</p>';
        const fact=model.fileFact(file);
        return '<h3 class="path">'+escapeText(file.path)+'</h3><dl><dt>计划</dt><dd>'+(file.selected?'已选择':'未选择')+'</dd><dt>实际状态</dt><dd>'+escapeText(states[fact.state]||fact.state)+'</dd><dt>实测耗时</dt><dd>'+duration(fact.seconds)+'</dd><dt>文件级启动</dt><dd>'+fact.attempts.length+' 条记录'+(!fact.attempts.length?'；实际是否执行未知':'')+'</dd><dt>复用来源</dt><dd>'+escapeText(file.producer||'未保留')+'</dd><dt>选择依据</dt><dd>'+escapeText(fact.attempts.map(a=>a.selection_reason).filter(Boolean).join('；')||'未保留逐文件选择理由，请结合原始报告核查。')+'</dd></dl><p class="notice">没有文件 attempt 不等于没有执行，也不等于通过或零耗时。阶段结果不可分摊到文件。</p>'+
            (fact.attempts.length?'<pre>'+escapeText(JSON.stringify(fact.attempts,null,2))+'</pre>':'');
    }
    const path='/runs/'+encodeURIComponent(view.run), stages=model.attempts.filter(a=>!files.some(f=>f.node_id===a.node_id&&f.graph_sha256===a.graph_sha256));
    return '<p class="muted">运行 ID：'+escapeText(view.run)+'</p><h3>完整诊断</h3><p><a target="_blank" rel="noopener" href="/d/storyos-run?var-run='+encodeURIComponent(view.run)+'&theme=light">打开完整详情 · DAG ↗</a></p><p><a target="_blank" rel="noopener" href="/d/storyos-compare?var-left='+encodeURIComponent(view.run)+'&theme=light">打开两轮比较 ↗</a></p><p><a target="_blank" rel="noopener" href="http://127.0.0.1:3754/api/v1'+path+'">Agent 接口 · 原始 JSON ↗</a></p><p class="notice">以上链接在新标签打开。关闭新标签即可返回，筛选与阅读位置保留。</p><h3>日志与来源</h3><p>仅展示允许的本地证据路径，不读取任意日志，不执行操作。</p><pre>'+escapeText(evidencePath(root.evidence))+'</pre><h3>阶段 attempt</h3>'+stages.map(a=>'<div class="stage"><strong>'+escapeText(a.node_id)+'</strong><span>'+escapeText(states[a.result]||a.result)+' · '+duration(a.duration_seconds)+'</span></div>').join('')+
        '<h3>原始请求</h3>'+model.requests.map(r=>'<p>'+escapeText(r.outcome)+' · '+escapeText(r.utc)+'</p><pre>'+escapeText(evidencePath(r.evidence))+'</pre>').join('');
}

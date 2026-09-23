const escapeText = value => String(value ?? '未知').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const duration = value => value == null ? '未知' : Math.abs(value) < 0.05 ? value.toFixed(3)+' 秒' : Math.abs(value) < 60 ? value.toFixed(1)+' 秒' : Math.abs(value) < 3600 ? (value/60).toFixed(1)+' 分钟' : (value/3600).toFixed(2)+' 小时';
const profiles = {complete:'完整验证',daily:'日常验证',targeted:'定向检查',recovery:'恢复检查'};
const states = {passed:'通过',failed:'失败',running:'运行中',stale:'心跳过期',interrupted:'中断',pending:'待执行',blocked:'阻塞',unknown:'未知',incomplete:'未完成','source-changed':'源已变化','not-selected':'未选择',cached:'复用'};
const ruleNames = {'daily-complete':'日常路径尝试完整验证','duplicate-candidate':'同一候选重复启动','unassigned':'启动未归属任务','resource-overlap':'检出资源重叠','runtime-regression':'可比运行耗时增长','stale-heartbeat':'心跳过期'};
const ruleHints = {'daily-complete':'区分被拒绝的请求与实际启动。','duplicate-candidate':'仅实际启动形成重复；核查恢复理由。','unassigned':'仅统计实际启动且缺少任务归属的根运行。','resource-overlap':'核查同仓库实际启动区间。','runtime-regression':'规则只使用达到可比条件的样本；差值不证明原因。','stale-heartbeat':'进程是否仍存活未知。'};
function badge(row, heartbeat) {
    const state = runState(row, heartbeat);
    return '<span class="badge '+escapeText(state)+'">'+escapeText(states[state] || state)+'</span>';
}
function elapsed(row, heartbeat) {
    const start = parseTime(row.actual_started_at || row.started_at);
    return runState(row,heartbeat)==='running' && Number.isFinite(start) ? Math.max(0,(Date.now()-start)/1000) : row.duration_seconds;
}
const runDuration = (row,heartbeat) => row.cache_status==='hit'?'结果复用 · 无新增执行':row.attempt_started===0?'未启动 · 无新增执行':duration(elapsed(row,heartbeat));
function runTable(rows, selected, heartbeat) {
    if (!rows.length) return '<div class="empty">没有符合条件的记录</div>';
    return '<div class="table"><div class="row heading"><span>运行</span><span>状态</span><span>开始时间 · UTC</span><span>耗时</span></div>'+rows.map(row => {
        const stamp = parseTime(row.started_at);
        return '<button class="row '+(row.run===selected?'selected':'')+'" data-run="'+escapeText(row.run)+'" aria-label="运行 '+escapeText(row.run)+'"><span><strong>任务 '+escapeText(row.issue ?? '未归属')+'</strong><small>'+escapeText(profiles[row.profile] || row.profile || '历史记录')+
            '</small></span><span data-status="'+escapeText(row.run)+'">'+badge(row,heartbeat)+'</span><span>'+escapeText(Number.isFinite(stamp)?new Date(stamp).toISOString().slice(0,19).replace('T',' '):null)+'</span><span data-duration="'+escapeText(row.run)+
            '">'+runDuration(row,heartbeat)+'</span></button>';
    }).join('')+'</div>';
}
function selectOptions(values, selected) {
    return values.map(([value,label]) => '<option value="'+value+'" '+(value===selected?'selected':'')+'>'+label+'</option>').join('');
}
function reviewSummary(findings) {
    return '<div class="rule-strip">'+Object.entries(ruleNames).map(([rule,label])=>'<span>'+label+' <b>'+findings.rows.filter(row=>row.rule===rule).length+'</b></span>').join('')+'</div><button class="text-link" data-page="review">查看全部规则与证据 →</button>';
}
function reviewView(findings) {
    const rules=Object.keys(ruleNames).filter(rule=>findings.rule==='all'||rule===findings.rule);
    return '<p class="muted">来自现有 violations 规则视图。被拒绝的请求不算执行；心跳过期表示存活未知。</p><div class="toolbar"><select aria-label="规则类别" data-review-setting="rule">'+selectOptions([['all','全部规则'],...Object.entries(ruleNames)],findings.rule)+'</select><select aria-label="规则排序" data-review-setting="sort">'+selectOptions([['newest','最新优先'],['oldest','最早优先']],findings.sort)+'</select></div>'+rules.map(rule=>{
        const items=findings.rows.filter(row=>row.rule===rule).sort((a,b)=>(findings.sort==='oldest'?1:-1)*((parseTime(a.started_at)||0)-(parseTime(b.started_at)||0)));
        return '<details class="rule-group" data-rule-section="'+rule+'" '+(findings.openRules.includes(rule)?'open':'')+'><summary>'+ruleNames[rule]+' · '+items.length+' 条</summary><p class="muted">'+ruleHints[rule]+'</p>'+(items.length?items.map(row=>'<div class="finding"><div><strong>任务 '+escapeText(row.issue??'未归属')+' · '+escapeText(row.run||'请求未绑定运行')+'</strong><small>'+escapeText(row.disposition==='prevented'?'请求已阻止 · 无新增执行':row.disposition==='unknown-liveness'?'存活未知':'实际启动')+' · '+escapeText(row.started_at||'时间未知')+'</small></div>'+(row.run?'<button data-run="'+escapeText(row.run)+'">查看运行 →</button>':'')+'<details><summary>规则来源与证据</summary><code>'+escapeText(rule)+' · '+escapeText(row.disposition)+' · '+escapeText(evidencePath(row.evidence))+'</code></details></div>').join(''):'<p class="empty">当前没有此类记录</p>')+'</details>';
    }).join('');
}
function overviewView(recent, current, overview, selected, findings) {
    const live=current.rows.filter(row=>row.group==='running');
    return '<div class="status-strip"><div><span>运行中 <b data-count="active">'+escapeText(overview.active)+'</b></span><span>未完成且存活未知 <b data-count="stale">'+escapeText(overview.unfinished==null?null:overview.unfinished-overview.active)+'</b></span></div><div>近24小时 <b data-count="starts">'+
        escapeText(overview.starts)+'</b> 次启动 · <b data-count="seconds">'+duration(overview.seconds)+'</b></div></div><h2>当前运行</h2>'+(live.length?runTable(live,selected,current.heartbeat):'<div class="empty"><strong>'+(current.total==null?'正在读取保留记录':'当前没有有效心跳的托管运行')+'</strong><small>采集状态请查看监控健康。普通 shell 与远程 CI 不在覆盖范围。</small></div>')+
        '<h2>需要核查</h2>'+reviewSummary(findings)+'<h2>最近运行 <span>最近 12 条</span></h2>'+runTable(recent.rows,selected,recent.heartbeat);
}
function historyView(model, selected) {
    return '<div class="toolbar"><input aria-label="搜索运行" placeholder="搜索任务、类型或运行 ID" data-setting="q" value="'+escapeText(model.q)+'"><select aria-label="运行状态" data-setting="status">'+selectOptions([['','全部状态'],['passed','通过'],['failed','失败'],['running','未完成（含心跳过期）'],['interrupted','中断']],model.status)+
        '</select><details class="display"><summary>显示选项</summary><label>排序<select aria-label="运行排序" data-setting="sort">'+selectOptions([['newest','最新优先'],['oldest','最早优先'],['duration','耗时最长']],model.sort)+'</select></label></details></div><p class="muted">'+
        escapeText(model.total)+' 条保留记录 · 排序在阅读时保持固定</p>'+runTable(model.rows,selected,model.heartbeat)+'<div class="pagination"><button data-offset="'+Math.max(0,model.offset-model.limit)+'" '+(model.offset?'':'disabled')+'>上一页</button><span>第 '+
        (Math.floor(model.offset/model.limit)+1)+' 页</span><button data-offset="'+model.next_offset+'" '+(model.next_offset==null?'disabled':'')+'>下一页</button></div>';
}

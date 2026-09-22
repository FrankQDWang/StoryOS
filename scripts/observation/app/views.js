const escapeText = value => String(value ?? '未知').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const duration = value => value == null ? '未知' : value < 60 ? Math.round(value)+' 秒' : value < 3600 ? (value/60).toFixed(1)+' 分钟' : (value/3600).toFixed(2)+' 小时';
const profiles = {complete:'完整验证',daily:'日常验证',targeted:'定向检查',recovery:'恢复检查'};
const states = {passed:'通过',failed:'失败',running:'运行中',stale:'心跳过期',interrupted:'中断',pending:'待执行',blocked:'阻塞',unknown:'未知',incomplete:'未完成','source-changed':'源已变化'};
function badge(row, heartbeat) {
    const state = runState(row, heartbeat);
    return '<span class="badge '+escapeText(state)+'">'+escapeText(states[state] || state)+'</span>';
}
function elapsed(row, heartbeat) {
    const start = parseTime(row.actual_started_at || row.started_at);
    return runState(row,heartbeat)==='running' && Number.isFinite(start) ? Math.max(0,(Date.now()-start)/1000) : row.duration_seconds;
}
function runTable(rows, selected, heartbeat) {
    if (!rows.length) return '<div class="empty">没有符合条件的记录</div>';
    return '<div class="table"><div class="row heading"><span>运行</span><span>状态</span><span>开始时间 · UTC</span><span>耗时</span></div>'+rows.map(row => {
        const stamp = parseTime(row.started_at);
        return '<button class="row '+(row.run===selected?'selected':'')+'" data-run="'+escapeText(row.run)+'" aria-label="运行 '+escapeText(row.run)+'"><span><strong>任务 '+escapeText(row.issue ?? '未归属')+'</strong><small>'+escapeText(profiles[row.profile] || row.profile || '历史记录')+
            '</small></span><span data-status="'+escapeText(row.run)+'">'+badge(row,heartbeat)+'</span><span>'+escapeText(Number.isFinite(stamp)?new Date(stamp).toISOString().slice(0,19).replace('T',' '):null)+'</span><span data-duration="'+escapeText(row.run)+
            '">'+duration(elapsed(row,heartbeat))+'</span></button>';
    }).join('')+'</div>';
}
function selectOptions(values, selected) {
    return values.map(([value,label]) => '<option value="'+value+'" '+(value===selected?'selected':'')+'>'+label+'</option>').join('');
}
function overviewView(recent, current, overview, selected) {
    const live=current.rows.filter(row=>row.group==='running'), stale=current.rows.filter(row=>row.group==='stale');
    return '<div class="status-strip"><div><span>运行中 <b data-count="active">'+escapeText(overview.active)+'</b></span><span>待核查 <b data-count="stale">'+escapeText(overview.unfinished==null?null:overview.unfinished-overview.active)+'</b></span></div><div>近24小时 <b data-count="starts">'+
        escapeText(overview.starts)+'</b> 次启动 · <b data-count="seconds">'+duration(overview.seconds)+'</b></div></div><h2>当前运行</h2>'+(live.length?runTable(live,selected,current.heartbeat):'<div class="empty"><strong>'+(current.total==null?'正在读取保留记录':'当前没有有效心跳的托管运行')+'</strong><small>采集状态请查看监控健康。普通 shell 与远程 CI 不在覆盖范围。</small></div>')+
        '<h2>需要核查</h2><p class="muted">心跳过期不代表仍在执行。打开记录核查最后证据。</p>'+runTable(stale,selected,current.heartbeat)+(current.next_offset!=null?'<p class="muted">当前页未列出全部未完成记录，请到运行历史筛选运行中并翻页。</p>':'')+'<h2>最近运行 <span>最近 12 条</span></h2>'+runTable(recent.rows,selected,recent.heartbeat);
}
function historyView(model, selected) {
    return '<div class="toolbar"><input aria-label="搜索运行" placeholder="搜索任务、类型或运行 ID" data-setting="q" value="'+escapeText(model.q)+'"><select aria-label="运行状态" data-setting="status">'+selectOptions([['','全部状态'],['passed','通过'],['failed','失败'],['running','未完成（含心跳过期）'],['interrupted','中断']],model.status)+
        '</select><details class="display"><summary>显示选项</summary><label>排序<select aria-label="运行排序" data-setting="sort">'+selectOptions([['newest','最新优先'],['oldest','最早优先'],['duration','耗时最长']],model.sort)+'</select></label></details></div><p class="muted">'+
        escapeText(model.total)+' 条保留记录 · 排序在阅读时保持固定</p>'+runTable(model.rows,selected,model.heartbeat)+'<div class="pagination"><button data-offset="'+Math.max(0,model.offset-model.limit)+'" '+(model.offset?'':'disabled')+'>上一页</button><span>第 '+
        (Math.floor(model.offset/model.limit)+1)+' 页</span><button data-offset="'+model.next_offset+'" '+(model.next_offset==null?'disabled':'')+'>下一页</button></div>';
}

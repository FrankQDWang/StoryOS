class HealthObservation {
    constructor(request,data={}) {this.request=request;this.data=data}
    async refresh() {this.data=await this.request('/health')}
    component(name,now=Date.now()) {
        const item=this.data[name]||{}, delay=(now-Date.parse(this.data.queried_at))/1000;
        const age=Number.isFinite(item.age_seconds)&&Number.isFinite(delay)?item.age_seconds+Math.max(0,delay):null;
        const limit=this.data.stale_after_seconds;
        const stale=Number.isFinite(age)&&Number.isFinite(limit)&&(age>limit||item.age_seconds<0);
        const status=age==null||!Number.isFinite(limit)?'unavailable':stale?'stale':item.status;
        return {...item,age_seconds:age,status:['ok','stale','unavailable'].includes(status)?status:'unavailable'};
    }
}
function healthView(model,api) {
    const data=model.data, collector=model.component('collector');
    const labels={ok:'正常',stale:'观察过期',unavailable:'未知 / 异常'};
    const components=[['collector','采集链路'],['query','SQLite 查询'],['grafana','Grafana']];
    const rows=[['探针时间 · UTC',data.probe_checked_at],['待采集记录',collector.pending],['已采集记录',collector.records],['最近采集耗时',duration(collector.seconds)],...(collector.error?[['采集错误',collector.error]]:[])];
    return '<div class="health-strip">'+components.map(([name,label])=>{const item=model.component(name);return '<span>'+label+' <b class="'+(item.status==='ok'?'green':'amber')+'">'+labels[item.status]+'</b></span>'}).join('')+
        '</div><h2>观察记录</h2><dl class="health-facts">'+components.map(([name,label])=>{const item=model.component(name);return '<dt>'+label+' · UTC</dt><dd>'+escapeText(item.checked_at)+' <span class="muted">· 距今 '+duration(item.age_seconds==null?null:Math.max(0,item.age_seconds))+'</span></dd>'}).join('')+
        rows.map(([label,value])=>'<dt>'+label+'</dt><dd>'+escapeText(value)+'</dd>').join('')+'</dl>'+
        (data.storage==='unavailable'?'<p class="health-warning">健康记录无法写入；当前读数仅来自进程内观察。</p>':'')+
        '<h2>Agent 只读诊断</h2><p>每 10 秒加查询耗时更新探针。每项观察超过 '+escapeText(data.stale_after_seconds)+' 秒即过期；HTTP 成功不代表所有组件正常。</p>'+
        '<p><a target="_blank" rel="noopener" href="'+api+'/health">健康 JSON ↗</a>　<a target="_blank" rel="noopener" href="'+api+'/runs">最近运行 JSON ↗</a></p>'+
        '<p class="muted">在新标签打开证据；关闭该标签即可返回本页。向 Agent 提供观察时间、运行 ID 和诊断页中的证据路径。</p>'+
        '<p class="muted">探针不执行诊断或修复。接口不接受命令、任意 SQL 或任意文件路径。</p>';
}

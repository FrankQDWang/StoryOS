function mount(host) {
    const api='http://127.0.0.1:3754/api/v1', key='storyos-supervision-v2';
    let retained={};
    try { retained=JSON.parse(sessionStorage.getItem(key)||'{}'); } catch {}
    const request=async path => {
        const response=await fetch(api+path,{cache:'no-store',signal:AbortSignal.timeout(8000)});
        if (!response.ok) throw Object.assign(Error('只读接口不可用（'+response.status+'）'),{status:response.status});
        return response.json();
    };
    const health=new HealthObservation(request,retained.health), shown=new WeakSet();
    let healthBusy=false, healthError='', healthScroll=retained.healthScroll||0;
    let lists={home:new RunList(request,{limit:12,...retained.home}), current:new RunList(request,{status:'running',limit:100,...retained.current}), history:new RunList(request,retained.history)};
    const findings=new ReviewFindings(request,retained.findings);let reviewSeen=false;
    let overview=retained.overview||{}, last=retained.last||'', error='', disposed=false, busy=false, detailBusy=false, editTimer;
    const evidence=new Map(Object.entries(retained.evidence||{}).map(([id,value])=>[id,new RunEvidence(request,id,value)]));
    const readRoute=() => {
        const route=new URLSearchParams(location.hash.slice(1));
        return {page:['home','review','history','health'].includes(route.get('page'))?route.get('page'):'home',run:route.get('run')||'',level:['summary','files','file','graph','node','timeline','compare','cost','diagnostics'].includes(route.get('level'))?route.get('level'):'summary',file:route.get('file')||'',node:route.get('node')||'',origin:['graph','timeline'].includes(route.get('origin'))?route.get('origin'):'',right:route.get('right')||''};
    };
    let view=readRoute();
    function save() {
        const body=host.querySelector('.body');
        if (body && lists[view.page]) lists[view.page].scroll=body.scrollTop;
        if(body&&view.page==='review')findings.scroll=body.scrollTop;
        if(body&&view.page==='health')healthScroll=body.scrollTop;
        const detail=host.querySelector('.detail-body');
        if(detail&&evidence.has(view.run))evidence.get(view.run).scrolls[view.level+':'+view.file]=detail.scrollTop;
        try { sessionStorage.setItem(key,JSON.stringify({home:lists.home.saved(),current:lists.current.saved(),history:lists.history.saved(),findings:findings.saved(),overview,last,health:health.data,healthScroll,evidence:Object.fromEntries([...evidence].slice(-4).map(([id,model])=>[id,model.saved()]))})); } catch {}
    }
    function render() {
        const focused=host.contains(document.activeElement)?document.activeElement:null, caret=focused?.selectionStart;
        const names={home:'总览',review:'需要核查',history:'运行历史',health:'监控健康'};
        const pending=view.page==='home'?lists.home.pending||lists.current.pending||findings.pending:view.page==='review'?findings.pending:view.page==='history'&&lists.history.pending;
        host.innerHTML='<div class="supervision"><aside class="nav"><div class="brand">StoryOS<small>仓库监督</small></div><div class="nav-label">工作空间</div>'+Object.entries(names).map(([page,label])=>'<button data-page="'+page+'" class="'+(page===view.page?'active':'')+'" '+(page===view.page?'aria-current="page"':'')+'>'+label+'</button>').join('')+
            '<div class="nav-bottom"><b>只读观察</b><p>本机 · Grafana App</p><small>仅受管理的本地验证<br>不含远程 CI 与 shell 绕行</small></div></aside><div class="workspace"><header><div><small>StoryOS / 本地验证</small><h1>'+names[view.page]+'</h1></div><span class="sync" data-sync></span></header><div class="update" '+
            (pending?'':'hidden')+'><button data-update>观察有变化 · 更新列表</button><span>分组与顺序已保留，点击后应用</span></div><div class="error" role="status" hidden></div><main class="body '+view.page+'">'+(view.page==='home'?overviewView(lists.home,lists.current,overview,view.run,findings):view.page==='review'?reviewView(findings):view.page==='history'?historyView(lists.history,view.run):healthView(health,api))+
            '<footer>只读监督 · 缺失证据保持未知 · 统计不证明计划已最小化</footer></main></div></div>';
        host.querySelector('.body').scrollTop=view.page==='health'?healthScroll:view.page==='review'?findings.scroll:lists[view.page]?.scroll||0;
        feedback();
        renderDetail();
        keepFocus(host,focused,caret);
    }
    function renderDetail() {
        let panel=host.querySelector('.peek');
        if(!view.run){panel?.remove();return}
        if(!evidence.has(view.run))evidence.set(view.run,new RunEvidence(request,view.run));
        if(!panel){panel=document.createElement('section');panel.className='peek';panel.setAttribute('role','dialog');panel.setAttribute('aria-labelledby','drawer-title');host.querySelector('.supervision').append(panel)}
        const focused=panel.contains(document.activeElement)?document.activeElement:null, caret=focused?.selectionStart;
        const model=evidence.get(view.run);
        panel.innerHTML=drawerView(model,view,lists[view.page]?.heartbeat||120);
        panel.querySelector('.detail-body').scrollTop=model.scrolls[view.level+':'+view.file]||0;
        keepFocus(panel,focused,caret);
    }
    function keepFocus(scope,previous,caret) {
        if(!previous)return;
        const marker=['data-page','data-setting','data-review-setting','data-detail-setting','data-graph-search','data-graph-filter','data-compare-query','data-compare-filter','data-compare-sort','data-timeline-sort','data-close','data-detail-back','data-file','data-node','data-level','data-scope','data-detail-update','data-update','data-run','href'].find(name=>previous.hasAttribute(name));
        const control=marker&&[...scope.querySelectorAll('button,a,input,select')].find(node=>node.getAttribute(marker)===previous.getAttribute(marker));
        const target=control||scope.querySelector('[data-close]');
        if(target){target.focus({preventScroll:true});if(target.setSelectionRange&&caret!=null)target.setSelectionRange(caret,caret)}
    }
    async function refreshDetail() {
        if(detailBusy||disposed||!view.run)return;
        const run=view.run, model=evidence.get(run);
        if(!model)return;
        detailBusy=true;
        try {await model.refresh(view.level==='compare'?view.right:'');model.error=''}
        catch(failure){model.error=failure.message+'；保留上次证据，最近读取 '+(model.root?.queried_at||'未知')}
        finally {detailBusy=false;if(!disposed&&view.run===run){save();renderDetail()}else if(!disposed)refreshDetail()}
    }
    function feedback() {
        const box=host.querySelector('.error');
        const currentError=view.page==='health'?healthError:error;
        box.hidden=!currentError;
        box.textContent=currentError;
        host.querySelector('[data-sync]').textContent=view.page==='health'?'最近读取 · '+(health.data.queried_at||'连接中'):error?'连接失败 · 上次成功 '+(last||'未知'):'每 10 秒读取 · '+(last||'连接中');
    }
    function navigate(patch) {
        const previous=view.run;
        save(); view={...view,...patch};
        history.pushState(null,'','/a/storyos-supervision-app?theme=light#'+new URLSearchParams(view));
        render(); poll(); refreshDetail(); restoreFocus(previous);
    }
    function restoreFocus(previous) {
        if(previous&&!view.run)host.querySelectorAll('[data-run]').forEach(button=>{if(button.dataset.run===previous)button.focus({preventScroll:true})});
        else if(view.run)host.querySelector('[data-detail-back],[data-close]')?.focus({preventScroll:true});
    }
    async function poll() {
        if(disposed)return;
        if(view.page==='health'){
            if(healthBusy)return;
            healthBusy=true;
            try {await health.refresh();healthError=''}
            catch{healthError='只读接口连接失败；保留上次观察，年龄继续增长。'}
            finally {healthBusy=false;if(!disposed&&view.page==='health'){save();render()}}
            return;
        }
        if(view.page==='review'){
            if(busy)return;busy=true;
            try {await findings.refresh();error='';last=new Date().toLocaleTimeString('zh-CN',{hour12:false});if(!disposed&&view.page==='review'){save();if(!reviewSeen){render();reviewSeen=true}else{host.querySelector('.update').hidden=!findings.pending;feedback()}}}
            catch(failure){error=failure.message+'；保留上次规则记录。';if(!disposed&&view.page==='review')feedback()}
            finally {busy=false;if(view.page!=='review')poll()}
            return;
        }
        if (busy) return;
        busy=true;
        const page=view.page, current=lists[page], initial=!shown.has(current);
        try {
            const summary=await request('/overview');
            current.heartbeat=summary.heartbeat_seconds;
            await current.refresh();
            if (page==='home') { lists.current.heartbeat=summary.heartbeat_seconds; await lists.current.refresh(); await findings.refresh(); }
            if (disposed || lists[page]!==current || view.page!==page) return;
            overview=summary; error=''; last=new Date().toLocaleTimeString('zh-CN',{hour12:false});
            save();
            if (initial) { render(); shown.add(current); }
            else {
                const facts=new Map([...current.rows,...(page==='home'?lists.current.rows:[])].map(row=>[row.run,row]));
                host.querySelectorAll('[data-status]').forEach(node=>{const row=facts.get(node.dataset.status);if(row)node.innerHTML=badge(row,current.heartbeat)});
                host.querySelectorAll('[data-duration]').forEach(node=>{const row=facts.get(node.dataset.duration);if(row)node.textContent=runDuration(row,current.heartbeat)});
                const counts={...summary,stale:summary.unfinished-summary.active,seconds:duration(summary.seconds)};
                host.querySelectorAll('[data-count]').forEach(node=>node.textContent=counts[node.dataset.count]??'未知');
                host.querySelector('.update').hidden=!(current.pending||(page==='home'&&(lists.current.pending||findings.pending)));
                feedback();
            }
        } catch (failure) {
            if (!disposed && lists[page]===current && view.page===page) {
                error=failure.message+'；保留上次读数，请到监控健康核查。'; feedback();
            }
        } finally { busy=false; if(view.page!==page || lists[page]!==current)poll(); }
    }
    function click(event) {
        const button=event.target.closest('button');
        if (!button || button.disabled) return;
        if(button.hasAttribute('data-close'))navigate({run:'',level:'summary',file:''});
        else if(button.hasAttribute('data-detail-back'))navigate({level:view.level==='file'?'files':view.level==='node'?view.origin||'graph':'summary',file:'',node:'',origin:''});
        else if(button.hasAttribute('data-level'))navigate({level:button.dataset.level,file:'',node:''});
        else if(button.hasAttribute('data-file'))navigate({level:'file',file:button.dataset.file});
        else if(button.hasAttribute('data-node'))navigate({level:'node',node:button.dataset.node,origin:view.level});
        else if(button.hasAttribute('data-compare-run'))navigate({level:'compare',right:button.dataset.compareRun});
        else if(button.hasAttribute('data-compare-search')){const model=evidence.get(view.run);model.searchComparison().then(()=>{renderDetail();save()}).catch(failure=>{model.error=failure.message;renderDetail()})}
        else if(button.hasAttribute('data-scope')){const model=evidence.get(view.run);model.settings.filter=button.dataset.scope;model.visible=null;navigate({level:'files',file:''})}
        else if(button.hasAttribute('data-detail-update')){save();evidence.get(view.run).apply();renderDetail();save()}
        else if (button.hasAttribute('data-page')) navigate({page:button.dataset.page,run:'',level:'summary',file:'',node:'',right:''});
        else if (button.hasAttribute('data-run')) navigate({run:button.dataset.run,level:'summary',file:'',node:'',right:''});
        else if (button.hasAttribute('data-update')) {
            save();if(view.page==='review')findings.apply();else{lists[view.page].apply();if(view.page==='home'){lists.current.apply();findings.apply()}}render();save();
        } else if (button.hasAttribute('data-offset')) change('offset',Number(button.dataset.offset));
    }
    function change(name,value) {
        save();
        const old=lists.history;
        lists.history=new RunList(request,{q:old.q,status:old.status,activity:old.activity,sort:old.sort,[name]:value,offset:name==='offset'?value:0});
        render(); save(); poll();
        const input=host.querySelector('[data-setting="'+name+'"]');
        if(input){input.focus({preventScroll:true});if(input.setSelectionRange)input.setSelectionRange(input.value.length,input.value.length)}
    }
    function input(event) {
        if(event.target.dataset.reviewSetting){findings[event.target.dataset.reviewSetting]=event.target.value;render();save();return}
        if(event.target.hasAttribute('data-graph-search')){evidence.get(view.run).graphSearch=event.target.value;renderDetail();save();return}
        if(event.target.hasAttribute('data-graph-filter')){evidence.get(view.run).graphFilter=event.target.value;renderDetail();save();return}
        if(event.target.hasAttribute('data-compare-query')){evidence.get(view.run).compareSearch=event.target.value;save();return}
        if(event.target.hasAttribute('data-compare-filter')||event.target.hasAttribute('data-compare-sort')||event.target.hasAttribute('data-timeline-sort')){
            const model=evidence.get(view.run);model[event.target.hasAttribute('data-compare-filter')?'compareFilter':event.target.hasAttribute('data-compare-sort')?'compareSort':'timelineSort']=event.target.value;renderDetail();save();return;
        }
        const detailSetting=event.target.dataset.detailSetting;
        if(detailSetting){const model=evidence.get(view.run);model.settings[detailSetting]=event.target.value;model.visible=null;model.scrolls['files:']=0;renderDetail();save();return}
        const name=event.target.dataset.setting;
        if(!name)return;
        clearTimeout(editTimer);
        const value=event.target.value;
        editTimer=setTimeout(()=>change(name,value),name==='q'?300:0);
    }
    function toggle(event) {
        if(!event.target.hasAttribute('data-rule-section'))return;
        const rule=event.target.dataset.ruleSection;
        findings.openRules=event.target.open?[...new Set([...findings.openRules,rule])]:findings.openRules.filter(item=>item!==rule);
        save();
    }
    function pop() {const previous=view.run;save();view=readRoute();render();poll();refreshDetail();restoreFocus(previous)}
    function escape(event) {if(event.key==='Escape'&&view.run){event.preventDefault();navigate({run:'',level:'summary',file:''})}}
    host.addEventListener('click',click); host.addEventListener('input',input);host.addEventListener('toggle',toggle,true);
    window.addEventListener('popstate',pop); window.addEventListener('beforeunload',save);window.addEventListener('keydown',escape,true);
    const css=document.createElement('link'); css.rel='stylesheet'; css.href='/public/plugins/storyos-supervision-app/style.css?v=__STYLE_DIGEST__'; css.onload=()=>{if(!disposed){render();poll();refreshDetail()}}; document.head.append(css);
    const timer=setInterval(()=>{poll();refreshDetail()},10000);
    return () => { disposed=true; save(); clearInterval(timer); clearTimeout(editTimer); css.remove();host.removeEventListener('click',click);host.removeEventListener('input',input);host.removeEventListener('toggle',toggle,true);window.removeEventListener('popstate',pop);window.removeEventListener('beforeunload',save);window.removeEventListener('keydown',escape,true); };
}
function App() {
    const ref=React.useRef(null);
    React.useEffect(()=>mount(ref.current),[]);
    return React.createElement('div',{ref});
}

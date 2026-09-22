function mount(host) {
    const api='http://127.0.0.1:3754/api/v1', key='storyos-supervision-v2';
    let retained={};
    try { retained=JSON.parse(sessionStorage.getItem(key)||'{}'); } catch {}
    const request=async path => {
        const response=await fetch(api+path,{cache:'no-store',signal:AbortSignal.timeout(8000)});
        if (!response.ok) throw Object.assign(Error('只读接口不可用（'+response.status+'）'),{status:response.status});
        return response.json();
    };
    const shown=new WeakSet();
    let lists={home:new RunList(request,{limit:12,...retained.home}), current:new RunList(request,{status:'running',limit:100,...retained.current}), history:new RunList(request,retained.history)};
    let overview=retained.overview||{}, last=retained.last||'', error='', disposed=false, busy=false, editTimer;
    const readRoute=() => {
        const route=new URLSearchParams(location.hash.slice(1));
        return {page:['home','history','health'].includes(route.get('page'))?route.get('page'):'home',run:route.get('run')||'',level:route.get('level')||'summary',file:route.get('file')||''};
    };
    let view=readRoute();
    function save() {
        const body=host.querySelector('.body');
        if (body && lists[view.page]) lists[view.page].scroll=body.scrollTop;
        try { sessionStorage.setItem(key,JSON.stringify({home:lists.home.saved(),current:lists.current.saved(),history:lists.history.saved(),overview,last})); } catch {}
    }
    function render() {
        const focused=document.activeElement, setting=focused?.dataset.setting, caret=focused?.selectionStart;
        const names={home:'总览',history:'运行历史',health:'监控健康'};
        const pending=view.page==='home'?lists.home.pending||lists.current.pending:lists.history.pending;
        host.innerHTML='<div class="supervision"><aside class="nav"><div class="brand">StoryOS<small>仓库监督</small></div><div class="nav-label">工作空间</div>'+Object.entries(names).map(([page,label])=>'<button data-page="'+page+'" class="'+(page===view.page?'active':'')+'" '+(page===view.page?'aria-current="page"':'')+'>'+label+'</button>').join('')+
            '<div class="nav-bottom"><b>只读观察</b><p>本机 · Grafana App</p><small>仅受管理的本地验证<br>不含远程 CI 与 shell 绕行</small></div></aside><div class="workspace"><header><div><small>StoryOS / 本地验证</small><h1>'+names[view.page]+'</h1></div><span class="sync" data-sync></span></header><div class="update" '+
            (pending?'':'hidden')+'><button data-update>运行有变化 · 更新列表</button><span>分组与顺序已保留，点击后应用</span></div><div class="error" role="status" hidden></div><main class="body '+view.page+'">'+(view.page==='home'?overviewView(lists.home,lists.current,overview,view.run):view.page==='history'?historyView(lists.history,view.run):'<h2>监控健康</h2><p><a href="'+api+'/health" target="_blank" rel="noopener">查看带时间戳的健康数据</a></p><p class="muted">在新标签查看，关闭该标签即可返回。</p>')+
            '<footer>只读监督 · 缺失证据保持未知 · 统计不证明计划已最小化</footer></main></div></div>';
        host.querySelector('.body').scrollTop=lists[view.page]?.scroll||0;
        feedback();
        const input=setting&&host.querySelector('[data-setting="'+setting+'"]');
        if(input){input.focus({preventScroll:true});if(input.setSelectionRange&&caret!=null)input.setSelectionRange(caret,caret)}
    }
    function feedback() {
        const box=host.querySelector('.error');
        box.hidden=!error;
        box.textContent=error;
        host.querySelector('[data-sync]').textContent=error?'连接失败 · 上次成功 '+(last||'未知'):'每 10 秒读取 · '+(last||'连接中');
    }
    function navigate(patch) {
        save(); view={...view,...patch};
        history.pushState(null,'','/a/storyos-supervision-app?theme=light#'+new URLSearchParams(view));
        render(); poll();
    }
    async function poll() {
        if (busy || disposed || view.page==='health') return;
        busy=true;
        const page=view.page, current=lists[page], initial=!shown.has(current);
        try {
            const summary=await request('/overview');
            current.heartbeat=summary.heartbeat_seconds;
            await current.refresh();
            if (page==='home') { lists.current.heartbeat=summary.heartbeat_seconds; await lists.current.refresh(); }
            if (disposed || lists[page]!==current || view.page!==page) return;
            overview=summary; error=''; last=new Date().toLocaleTimeString('zh-CN',{hour12:false});
            save();
            if (initial) { render(); shown.add(current); }
            else {
                const facts=new Map([...current.rows,...(page==='home'?lists.current.rows:[])].map(row=>[row.run,row]));
                host.querySelectorAll('[data-status]').forEach(node=>{const row=facts.get(node.dataset.status);if(row)node.innerHTML=badge(row,current.heartbeat)});
                host.querySelectorAll('[data-duration]').forEach(node=>{const row=facts.get(node.dataset.duration);if(row)node.textContent=duration(elapsed(row,current.heartbeat))});
                const counts={...summary,stale:summary.unfinished-summary.active,seconds:duration(summary.seconds)};
                host.querySelectorAll('[data-count]').forEach(node=>node.textContent=counts[node.dataset.count]??'未知');
                host.querySelector('.update').hidden=!(current.pending||(page==='home'&&lists.current.pending));
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
        if (button.hasAttribute('data-page')) navigate({page:button.dataset.page,run:'',level:'summary',file:''});
        else if (button.hasAttribute('data-run')) navigate({run:button.dataset.run,level:'summary',file:''});
        else if (button.hasAttribute('data-update')) {
            save(); lists[view.page].apply(); if(view.page==='home')lists.current.apply(); render(); save();
        } else if (button.hasAttribute('data-offset')) change('offset',Number(button.dataset.offset));
    }
    function change(name,value) {
        save();
        const old=lists.history;
        lists.history=new RunList(request,{q:old.q,status:old.status,sort:old.sort,[name]:value,offset:name==='offset'?value:0});
        render(); save(); poll();
        const input=host.querySelector('[data-setting="'+name+'"]');
        if(input){input.focus({preventScroll:true});if(input.setSelectionRange)input.setSelectionRange(input.value.length,input.value.length)}
    }
    function input(event) {
        const name=event.target.dataset.setting;
        if(!name)return;
        clearTimeout(editTimer);
        const value=event.target.value;
        editTimer=setTimeout(()=>change(name,value),name==='q'?300:0);
    }
    function pop() { save(); view=readRoute(); render(); poll(); }
    host.addEventListener('click',click); host.addEventListener('input',input);
    window.addEventListener('popstate',pop); window.addEventListener('beforeunload',save);
    const css=document.createElement('link'); css.rel='stylesheet'; css.href='/public/plugins/storyos-supervision-app/style.css?v=__STYLE_DIGEST__'; css.onload=()=>{if(!disposed){render();poll()}}; document.head.append(css);
    const timer=setInterval(poll,10000);
    return () => { disposed=true; save(); clearInterval(timer); clearTimeout(editTimer); css.remove();host.removeEventListener('click',click);host.removeEventListener('input',input);window.removeEventListener('popstate',pop);window.removeEventListener('beforeunload',save); };
}
function App() {
    const ref=React.useRef(null);
    React.useEffect(()=>mount(ref.current),[]);
    return React.createElement('div',{ref});
}

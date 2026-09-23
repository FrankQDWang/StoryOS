class RunEvidence {
    constructor(request,run,saved={}) {
        this.request=request; this.run=run;
        Object.assign(this,{root:null,files:[],attempts:[],requests:[],graph:null,cost:null,comparison:null,compareRight:'',compareSearch:'',compareOptions:[],compareFilter:'changed',compareSort:'kind',graphSearch:'',graphFilter:'active',timelineSort:'start',settings:{q:'',filter:'all',sort:'path'},scrolls:{},visible:null,applied:''},saved);
        this.latest=null; this.pending=false;
    }
    async pages(path) {
        const items=[];
        let offset=0;
        do {
            const page=await this.request(path+(path.includes('?')?'&':'?')+'limit=100&offset='+offset);
            items.push(...page.items);
            if(items.length>10000)throw Error('证据超过本页读取上限，请使用 Agent 接口分页核查');
            if(page.next_offset!=null&&page.next_offset<=offset)throw Error('证据分页无进展');
            offset=page.next_offset;
        } while(offset!=null);
        return items;
    }
    async refresh(right='') {
        const path='/runs/'+encodeURIComponent(this.run), root=await this.request(path);
        const files=await this.pages(path+'/files'), attempts=await this.pages(path+'/attempts');
        const requests=await this.pages('/requests?run='+encodeURIComponent(this.run));
        const graph=await this.request(path+'/graph'), cost=await this.request(path+'/cost');
        const signature=JSON.stringify([files.map(f=>[f.node_id,f.graph_sha256,f.selected,f.state]),attempts.map(a=>[a.attempt_id,a.result,a.duration_seconds,a.ended_at]),requests.map(r=>r.evidence)]);
        this.root=root;this.graph=graph;this.cost=cost;this.latest={files,attempts,requests,signature};
        if(!this.applied)this.apply();
        else {
            this.pending=signature!==this.applied;
            const current=new Map(files.map(f=>[f.graph_sha256+f.node_id,f]));
            this.files=this.files.map(f=>current.has(f.graph_sha256+f.node_id)?{...current.get(f.graph_sha256+f.node_id),selected:f.selected}:{...f,state:'unknown',duration_seconds:null,missing:true});
            const actual=new Map(attempts.map(a=>[a.attempt_id,a]));
            this.attempts=this.attempts.map(a=>actual.get(a.attempt_id)||{...a,result:'unknown',duration_seconds:null});
        }
        if(right)await this.loadComparison(right);
    }
    async searchComparison() {
        const page=await this.request('/runs?q='+encodeURIComponent(this.compareSearch)+'&limit=50');
        this.compareOptions=page.items.filter(row=>row.run!==this.run);
    }
    async loadComparison(right) {
        if(!/^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$/.test(right))throw Error('运行 ID 无效');
        const path='/compare?left='+encodeURIComponent(this.run)+'&right='+encodeURIComponent(right);
        let offset=0, result, items=[];
        do {
            const page=await this.request(path+'&limit=500&offset='+offset);
            result=page;items.push(...page.differences.items);
            if(items.length>10000)throw Error('差异超过本页读取上限，请使用 Agent 接口分页核查');
            offset=page.differences.next_offset;
        } while(offset!=null);
        this.comparison={...result,differences:items};this.compareRight=right;
    }
    apply() {
        if(!this.latest)return;
        const {files,attempts,requests,signature}=this.latest;
        this.files=files;this.attempts=attempts;this.requests=requests;this.applied=signature;this.pending=false;this.visible=null;
    }
    fileFact(file) {
        const attempts=file.missing?[]:this.attempts.filter(a=>a.node_id===file.node_id&&a.graph_sha256===file.graph_sha256);
        return {state:attempts.length?file.state:'unknown',
            seconds:attempts.length&&file.state!=='unknown'&&attempts.every(a=>a.ended_at&&a.duration_seconds!=null)?attempts.reduce((n,a)=>n+a.duration_seconds,0):null,attempts};
    }
    saved() {
        const {root,files,attempts,requests,graph,cost,comparison,compareRight,compareSearch,compareOptions,compareFilter,compareSort,graphSearch,graphFilter,timelineSort,settings,scrolls,visible,applied}=this;
        return {root,files,attempts,requests,graph,cost,comparison,compareRight,compareSearch,compareOptions,compareFilter,compareSort,graphSearch,graphFilter,timelineSort,settings,scrolls,visible,applied};
    }
}

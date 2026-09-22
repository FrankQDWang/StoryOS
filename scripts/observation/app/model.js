const parseTime = value => Date.parse(String(value || '').replace(/^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z$/, '$1-$2-$3T$4:$5:$6Z'));
function runState(row, heartbeat = 120) {
    const age = (Date.now() - parseTime(row.heartbeat_at)) / 1000;
    return row.status === 'running' && (!Number.isFinite(age) || age < 0 || age > heartbeat) ? 'stale' : row.status;
}
class RunList {
    constructor(request, saved = {}) {
        this.request = request;
        Object.assign(this, {q:'', status:'', sort:'newest', offset:0, limit:50, scroll:0,
            rows:[], total:null, next_offset:null, applied:'', heartbeat:120}, saved);
        this.latest = null;
        this.pending = false;
    }
    signature(rows) {
        return JSON.stringify(rows.map(row => [row.run, runState(row, this.heartbeat)]));
    }
    async refresh() {
        const parameters = new URLSearchParams({q:this.q, status:this.status, sort:this.sort,
            offset:this.offset, limit:this.limit});
        const page = await this.request('/runs?' + parameters);
        const facts = new Map(page.items.map(row => [row.run,row]));
        for (const row of this.rows) {
            if (!facts.has(row.run)) {
                try { facts.set(row.run, (await this.request('/runs/' + encodeURIComponent(row.run))).record); }
                catch (error) {
                    if (error.status!==404) throw error;
                    facts.set(row.run,{...row,status:'unknown',duration_seconds:null});
                }
            }
        }
        this.latest = page;
        if (!this.applied) this.apply();
        else {
            this.pending = this.signature(page.items) !== this.applied || page.total !== this.total;
            this.rows = this.rows.map(row => ({...facts.get(row.run), group:row.group}));
        }
    }
    apply() {
        if (!this.latest) return;
        this.rows = this.latest.items.map(row => ({...row, group:runState(row,this.heartbeat)}));
        this.total = this.latest.total;
        this.next_offset = this.latest.next_offset;
        this.applied = this.signature(this.rows);
        this.pending = false;
    }
    saved() {
        const {q,status,sort,offset,limit,scroll,rows,total,next_offset,applied,heartbeat} = this;
        return {q,status,sort,offset,limit,scroll,rows,total,next_offset,applied,heartbeat};
    }
}

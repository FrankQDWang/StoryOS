"""Check list reading stability through the App state boundary."""

from pathlib import Path
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[1]


class AppTests(unittest.TestCase):
    def test_health_ages_independent_observations_after_a_failed_read(self):
        script = r'''
const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
vm.runInThisContext(fs.readFileSync('scripts/observation/app/health.js','utf8'));
(async()=>{
    const now=Date.parse('2026-09-22T12:00:00Z');
    const item={status:'ok',reported_status:'ok',checked_at:'2026-09-22T11:59:50Z',age_seconds:10};
    const data={queried_at:'2026-09-22T12:00:00Z',stale_after_seconds:30,collector:item,query:{...item,status:'unavailable',reported_status:'unavailable'},grafana:{...item,age_seconds:29}};
    const model=new HealthObservation(async()=>data);
    await model.refresh();
    assert.deepEqual(model.component('collector',now),{...item,status:'ok',age_seconds:10});
    assert.equal(model.component('query',now).status,'unavailable');
    assert.equal(model.component('collector',now-10).status,'ok');
    assert.equal(model.component('grafana',now+2000).status,'stale');
    model.request=async()=>{throw Error('offline')};
    await assert.rejects(model.refresh(),/offline/);
    assert.equal(model.component('collector',now+21000).status,'stale');
    const restored=new HealthObservation(async()=>data,JSON.parse(JSON.stringify(model.data)));
    assert.deepEqual(restored.component('collector',now+21000),model.component('collector',now+21000));
    assert.equal(new HealthObservation(async()=>({})).component('collector',now).status,'unavailable');
    model.data.collector={...item,age_seconds:-1};
    assert.equal(model.component('collector',now).status,'stale');
})().catch(e=>{console.error(e);process.exitCode=1});
'''
        result = subprocess.run(['node', '-e', script], cwd=ROOT, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_drawer_keeps_membership_and_requires_file_attempts(self):
        script = r'''
const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
vm.runInThisContext(fs.readFileSync('scripts/observation/app/drawer-model.js','utf8'));
(async()=>{
    const file={node_id:'file:a',graph_sha256:'g',path:'a.rs',selected:1,state:'unknown'};
    let files=[file], attempts=[{attempt_id:'stage',node_id:'check:a',graph_sha256:'g',result:'passed',duration_seconds:9}];
    let graph={graph:{nodes:[{id:'a'}]},states:[]},cost={root:{seconds:2},stages:[],issue:null};
    const request=async path=>path.includes('/files?')?{items:files,next_offset:null}:
        path.includes('/attempts?')?{items:attempts,next_offset:null}:
        path.includes('/requests?')?{items:[],next_offset:null}:path.endsWith('/graph')?graph:path.endsWith('/cost')?cost:{record:{run:'run',status:'running'},has_graph:true};
    const model=new RunEvidence(request,'run');
    await model.refresh();
    assert.deepEqual(model.fileFact(file),{state:'unknown',seconds:null,attempts:[]});
    const older=model.saved();delete older.graph;delete older.cost;
    const migrated=new RunEvidence(request,'run',older);await migrated.refresh();assert.equal(migrated.graph.graph.nodes.length,1);
    graph={graph:{nodes:[{id:'a'},{id:'b'}]},states:[]};cost={root:{seconds:3},stages:[],issue:null};
    await model.refresh();assert.equal(model.pending,true);assert.equal(model.graph.graph.nodes.length,1);assert.equal(model.cost.root.seconds,3);
    model.apply();assert.equal(model.graph.graph.nodes.length,2);assert.equal(model.cost.root.seconds,3);
    cost={...cost,root:{seconds:4}};await model.refresh();assert.equal(model.pending,false);assert.equal(model.cost.root.seconds,4);
    attempts=[...attempts,{attempt_id:'file',node_id:'file:a',graph_sha256:'g',result:'passed',duration_seconds:2,ended_at:'2026-09-22T00:00:02Z'}];
    files=[{...file,state:'passed'},{...file,node_id:'file:b',path:'b.rs'}];
    await model.refresh();
    assert.equal(model.pending,true); assert.equal(model.files.length,1);
    assert.equal(model.fileFact(file).seconds,null);
    model.apply(); assert.equal(model.files.length,2);
    assert.equal(model.fileFact(model.files[0]).seconds,2);
    let difference='common',seconds=0;model.request=async()=>({comparison:{comparison:'descriptive only',delta_seconds:seconds},runs:[{selected:seconds}],differences:{items:[{node_id:'a',difference,left_definition:difference}],next_offset:null}});
    await model.loadComparison('other');difference='changed-definition';seconds=3;await model.loadComparison('other');
    assert.equal(model.comparison.differences[0].difference,'common');
    assert.equal(model.comparison.differences[0].visibleChanged,false);assert.equal(model.comparison.differences[0].left_definition,'common');assert.equal(model.comparison.comparison.delta_seconds,3);assert.equal(model.pending,true);
    difference='common';await model.loadComparison('other');assert.equal(model.pending,false);
    difference='changed-definition';await model.loadComparison('other');
    model.apply();assert.equal(model.comparison.differences[0].difference,'changed-definition');model.request=request;
    attempts=[attempts[0],{...attempts[1],result:'running',ended_at:null,duration_seconds:null}];
    files=[file,files[1]];
    await model.refresh(); assert.equal(model.pending,true);
    assert.equal(model.fileFact(file).state,'unknown');
    model.apply();
    model.scrolls.files=212; model.settings.q='a';
    const restored=new RunEvidence(request,'run',JSON.parse(JSON.stringify(model.saved())));
    assert.equal(restored.scrolls.files,212); assert.equal(restored.settings.q,'a');
    restored.request=async()=>{throw Error('offline')};
    await assert.rejects(restored.refresh(),/offline/); assert.equal(restored.files.length,2);
    files=[]; model.request=request; await model.refresh();
    assert.equal(model.files[0].state,'unknown'); assert.equal(model.pending,true);
    assert.equal(model.fileFact(model.files[0]).seconds,null);
})().catch(e=>{console.error(e);process.exitCode=1});
'''
        result = subprocess.run(['node', '-e', script], cwd=ROOT, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_refresh_retains_reading_state_until_apply(self):
        script = r'''
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
vm.runInThisContext(fs.readFileSync('scripts/observation/app/model.js', 'utf8'));
vm.runInThisContext(fs.readFileSync('scripts/observation/app/views.js', 'utf8'));
(async () => {
    let rows = [{run:'a', status:'running'}, {run:'b', status:'passed'}];
    const calls = [];
    const request = async path => {
        calls.push(path);
        if (path.startsWith('/runs?')) return {items: rows, total:105, next_offset:50};
        return {record: {run:'b', status:'failed'}};
    };
    const model = new RunList(request, {q:'needle', sort:'oldest', offset:50, scroll:312});
    await model.refresh();
    assert.match(calls[0], /q=needle/);
    assert.match(calls[0], /offset=50/);
    rows = [{run:'new', status:'passed'}, {run:'a', status:'passed'}];
    await model.refresh();
    assert.deepEqual(model.rows.map(r => [r.run,r.status]), [['a','passed'],['b','failed']]);
    assert.equal(model.pending, true);
    assert.equal(model.scroll,312);
    const restored = new RunList(request, JSON.parse(JSON.stringify(model.saved())));
    assert.equal(restored.q,'needle');
    assert.equal(restored.offset,50);
    assert.deepEqual(restored.rows,model.rows);
    restored.apply();
    await restored.refresh();
    restored.apply();
    assert.deepEqual(restored.rows.map(r => r.run),['new','a']);
    assert.equal(restored.pending,false);
    model.request = async () => {throw Error('offline')};
    await assert.rejects(model.refresh(), /offline/);
    assert.equal(model.rows[0].run,'a');
    assert.match(runTable([{run:'reused',status:'passed',cache_status:'hit',attempt_started:0,duration_seconds:10}],null,120),/结果复用 · 无新增执行/);
    model.request = request;
    await model.refresh();
    assert.equal(model.pending,true);
    model.request = async path => {
        if(path.startsWith('/runs?')) return {items:rows,total:2,next_offset:null};
        throw Object.assign(Error('removed'),{status:404});
    };
    await model.refresh();
    assert.equal(model.rows[1].status,'unknown');
    assert.equal(model.pending,true);
    let findings=[{rule:'unassigned',run:'a',disposition:'executed',evidence:'a/report.json'}];
    const review=new ReviewFindings(async()=>({items:findings,next_offset:null}));
    await review.refresh();
    review.rule='unassigned';review.scroll=90;review.openRules=['unassigned'];
    findings=[{rule:'stale-heartbeat',run:'b',disposition:'unknown-liveness',evidence:'b/report.json'}];
    await review.refresh();
    assert.equal(review.pending,true);
    assert.equal(review.rows[0].run,'a');
    const reopened=new ReviewFindings(review.request,JSON.parse(JSON.stringify(review.saved())));
    assert.deepEqual([reopened.rule,reopened.scroll,reopened.openRules],['unassigned',90,['unassigned']]);
    reopened.apply();
    assert.equal(reopened.rows[0].run,'a');
    await reopened.refresh();reopened.apply();
    assert.equal(reopened.rows[0].run,'b');
})().catch(error => {console.error(error); process.exitCode=1});
'''
        result = subprocess.run(['node', '-e', script], cwd=ROOT, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

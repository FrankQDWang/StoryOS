"""Check list reading stability through the App state boundary."""

from pathlib import Path
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[1]


class AppTests(unittest.TestCase):
    def test_refresh_retains_reading_state_until_apply(self):
        script = r'''
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
vm.runInThisContext(fs.readFileSync('scripts/observation/app/model.js', 'utf8'));
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
})().catch(error => {console.error(error); process.exitCode=1});
'''
        result = subprocess.run(['node', '-e', script], cwd=ROOT, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

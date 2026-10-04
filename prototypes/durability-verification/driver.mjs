// One-time public-interface reliability probe. Run with scripts/dev-postgres.sh run.
import assert from 'node:assert/strict';
import { isDeepStrictEqual } from 'node:util';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { writeFileSync } from 'node:fs';
import * as api from '../../generated/typescript/storyos-public-release-1/client.mjs';

const USER = '018f0000-0000-7001-8000-000000000001';
const PROJECT = '018f0000-0000-7001-8000-000000000002';
const CHAPTER = '018f0000-0000-7001-8000-000000000003';
const scope = `owner_user_id='${USER}' AND project_id='${PROJECT}'`;
const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
assert.ok(container, 'Use scripts/dev-postgres.sh run');
const scenario = process.argv[2];
const events = [];
const record = (name, data) => { events.push({ name, data }); console.log(name, JSON.stringify(data));
  if (name === 'verdict' && data === 'fails') process.exitCode = 1; };
let serial = 10000;
const id = () => `018f0000-0000-7001-8000-${String(++serial).padStart(12, '0')}`;
const common = () => ({ client_contract_revision: 'storyos.web-client.release-1.v3',
  security_policy_revision: 'storyos.web-security-policy.release-1.v1', correlation_id: id() });

class SQL {
  constructor() {
    this.child = spawn('docker', ['exec', '-i', container, 'psql', '-XqAt', '-U', 'postgres', '-v', 'ON_ERROR_STOP=1']);
    this.lines = []; this.errors = '';
    createInterface({ input: this.child.stdout }).on('line', line => {
      if (line === this.marker) { this.done?.(this.lines); this.done = undefined; this.lines = []; }
      else this.lines.push(line);
    });
    this.child.stderr.on('data', b => { this.errors += b; });
    this.child.on('exit', code => this.fail?.(new Error(`psql exit ${code}: ${this.errors}`)));
  }
  async query(sql) {
    this.marker = `done_${++serial}`;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('SQL deadline: ' + sql)), 15000);
      this.done = value => { clearTimeout(timer); resolve(value); };
      this.fail = error => { clearTimeout(timer); reject(error); };
      this.child.stdin.write(`${sql};\n\\echo ${this.marker}\n`);
    });
  }
  async json(sql) { return JSON.parse((await this.query(sql))[0]); }
  async close() { const exit = once(this.child, 'exit'); this.child.stdin.end('ROLLBACK;\n\\q\n'); await exit; }
}
const monitor = new SQL();
const gate = new SQL();
const tailGate = new SQL();
let server, baseUrl, bind = '127.0.0.1:0';
async function start() {
  const database = new URL(process.env.STORYOS_TEST_DATABASE_URL);
  database.searchParams.set('application_name', 'durability_server');
  server = spawn('target/release-package/storyos-server', ['--bind', bind, '--web-root', 'target/release-package/web'], {
    env: { ...process.env, STORYOS_DATABASE_URL: database.href, STORYOS_WORKER: '0',
      STORYOS_BOOTSTRAP_SESSIONS: JSON.stringify({ 'session-a': USER, 'session-b': USER }),
      STORYOS_TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS: '1',
      STORYOS_CHALLENGE_SECRET: 'local-durability-probe-secret-never-used-outside-this-fixture' },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let errors = ''; server.stderr.on('data', b => { errors += b; });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('Server start deadline: ' + errors)), 15000);
    createInterface({ input: server.stdout }).on('line', line => {
      if (line.startsWith('STORYOS_SERVER_URL=')) {
        baseUrl = line.split('=')[1]; bind = new URL(baseUrl).host; clearTimeout(timer); resolve();
      }
    });
    server.once('exit', code => { clearTimeout(timer); reject(new Error(`Server exit ${code}: ${errors}`)); });
  });
}
async function stop() {
  if (server?.exitCode === null && server?.signalCode === null) {
    const exit = once(server, 'exit'); server.kill('SIGKILL'); await exit;
  }
}
const options = (session = 'a') => ({ baseUrl, projectId: PROJECT,
  fetchImpl: (url, init) => fetch(url, { ...init, signal: AbortSignal.timeout(20000),
    headers: { ...init?.headers, origin: baseUrl, cookie: `storyos_session=session-${session}` } }) });
async function command(kind, route, request, session = 'a', extra = {}, method = 'POST') {
  const key = id();
  const nonce = await api.createProjectCommandChallenge({ ...options(session), request: {
    method, route_template: `/api/v1/projects/{project_id}${route}`, command_schema: request.command_schema,
    canonical_command_digest: await api[`digest${kind[0].toUpperCase()}${kind.slice(1)}`](request), idempotency_key: key,
  } });
  return { key, request, session, send: (overrides = {}) => api[kind]({ ...options(session), ...extra, request,
    idempotencyKey: key, antiForgery: nonce.nonce, ...overrides }),
    outcome: () => api.getApplyAuthorEditOutcome({ ...options(session), idempotencyKey: key, antiForgery: nonce.nonce }) };
}
async function open(session = 'a') {
  const cmd = await command('createEditorSession', '/editor-sessions', {
    ...common(), command_schema: 'storyos.command.create-editor-session.request.v1' }, session);
  return { cmd, value: await cmd.send(), session };
}
const read = editor => api.getEditorSession({ ...options(editor.session), editorSessionId: editor.value.editor_session.editor_session_id });
const chapter = async () => { const { correlation_id, ...value } = await api.getChapter({ ...options(), chapterId: CHAPTER }); return value; };
async function edit(editor, text = '!') {
  const state = await read(editor), snap = state.base_snapshot, from = snap.materialized_revision.body.length;
  return command('applyAuthorEdit', '/manuscript/author-edits', {
    ...common(), command_schema: 'storyos.command.apply-author-edit.request.v1',
    editor_session_id: state.editor_session.editor_session_id, writer_generation: state.writer.writer_generation,
    chapter_id: CHAPTER, expected_authoritative_revision_id: snap.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: snap.proposal_head_revision_ids, target_refs: snap.target_refs,
    observed_ownership_partition: snap.observed_ownership_partition, editor_contract_revision: 'storyos.editor-contract.release-1.v3',
    undo_group_id: id(), completed_intent_record_id: id(), local_intent_sequence: String(serial),
    author_edit_units: [{ normalized_primitives: [{ kind: 'replace_selection', from, to: from, text }],
      selection_snapshot: { coordinate_profile: 'storyos.editor.utf16-code-unit.v1', from, to: from } }],
  }, editor.session);
}
async function takeover(editor, generation) {
  return command('takeOverProjectWriter', '/editor-sessions/{editor_session_id}/takeovers', {
    ...common(), command_schema: 'storyos.command.take-over-project-writer.request.v1',
    editor_session_id: editor.value.editor_session.editor_session_id, observed_writer_generation: generation,
    editor_contract_revision: 'storyos.editor-contract.release-1.v3',
  }, editor.session, { editorSessionId: editor.value.editor_session.editor_session_id });
}
async function rename(session, title) {
  const revision = (await monitor.query(`SELECT revision::text FROM storyos.projects WHERE ${scope}`))[0];
  return command('updateProject', '', { command_schema: 'storyos.command.update-project.request.v1',
    update_project_input: { ...common(), title, expected_project_revision: revision } }, session, {}, 'PATCH');
}
async function counts() {
  return monitor.json(`SELECT json_build_object('commits',(SELECT count(*) FROM storyos.authoritative_commits WHERE ${scope}),
    'receipts',(SELECT count(*) FROM storyos.domain_receipts WHERE ${scope}),
    'admissions',(SELECT count(*) FROM storyos.author_command_admissions WHERE ${scope}),
    'writer',(SELECT max(writer_generation) FROM storyos.project_writer_generations WHERE ${scope}))`);
}
async function blocked(count, pattern = '') {
  const end = Date.now() + 12000;
  while (Date.now() < end) {
    const rows = await monitor.json(`SELECT coalesce(json_agg(json_build_object('pid',pid,'query',query,
      'wait',wait_event,'blockers',pg_blocking_pids(pid))), '[]') FROM pg_stat_activity
      WHERE application_name='durability_server' AND wait_event_type='Lock' AND query LIKE '%${pattern}%'`);
    if (rows.length >= count) { record('barrier', rows); return rows; }
  }
  record('activity_at_deadline', await monitor.json("SELECT coalesce(json_agg(json_build_object('app',application_name,'state',state,'wait',wait_event,'query',query)), '[]') FROM pg_stat_activity WHERE usename='storyos_runtime'"));
  throw new Error(`Barrier not reached: ${count} ${pattern}`);
}
const capture = async promise => {
  try { const value = await promise; record('request_settled', { ok: true }); return { ok: true, value }; }
  catch (e) { const result = { ok: false, status: e.status ?? 'transport', problem: e.responseBody ? JSON.parse(e.responseBody) : e.message }; record('request_settled', result); return result; }
};
async function hold(table, predicate = scope) { await gate.query(`BEGIN; SELECT 1 FROM storyos.${table} WHERE ${predicate} FOR UPDATE`); }
async function cut(mode, rows) {
  if (mode === 'server') await stop();
  else for (const row of rows) await monitor.query(`SELECT pg_terminate_backend(${row.pid})`);
}
async function smoke(editor) {
  const cmd = await edit(editor, '+'); const reply = await cmd.send();
  assert.equal(reply.effect.kind, 'authoritative_applied'); record('continued_writing', reply.effect.kind);
}

async function run() {
  await start();
  record('environment', await monitor.json(`SELECT json_build_object('postgres',version(),'isolation',current_setting('default_transaction_isolation'),
    'fsync',current_setting('fsync'),'synchronous_commit',current_setting('synchronous_commit'),'full_page_writes',current_setting('full_page_writes'))`));
  let writer = scenario.startsWith('concurrent-') ? undefined : await open();
  if (scenario === 'lost-ack') {
    const cmd = await edit(writer, 'lost'); const before = await counts();
    record('discarded_ack', await capture(cmd.send({ fetchImpl: async (url, init) => {
      const response = await options().fetchImpl(url, init); await response.arrayBuffer();
      await stop(); throw new Error('Acknowledgement discarded after HTTP delivery; Server killed');
    } })));
    await start(); const outcome = await cmd.outcome();
    assert.equal(outcome.outcome.outcome_kind, 'committed');
    assert.deepEqual(await cmd.send(), outcome.outcome.response);
    assert.equal((await counts()).commits, before.commits + 1); record('lost_ack_recovered', outcome);
    await smoke(writer);
  } else if (scenario === 'durable') {
    const cmd = await edit(writer); const ack = await cmd.send(), before = await counts(), body = await chapter();
    for (const mode of ['server', 'database']) {
      if (mode === 'server') { await stop(); await start(); }
      else {
        await monitor.query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE application_name='durability_server'");
        await stop(); await start();
      }
      assert.deepEqual(await chapter(), body); assert.deepEqual(await cmd.send(), ack); assert.deepEqual(await counts(), before);
      record('ack_survives_' + mode, { ack, counts: before });
    }
    await smoke(writer);
  } else if (scenario.startsWith('cut-')) {
    const [, phase, mode] = scenario.split('-');
    const seed = await edit(writer, 'seed'); await seed.send();
    const cmd = await edit(writer, 'cut'), before = await counts(), body = await chapter();
    await hold(phase === 'admission' ? 'command_idempotency' : 'authoritative_heads',
      phase === 'admission' ? `${scope} AND idempotency_key='${cmd.key}'` : `${scope} AND manuscript_object_id='${CHAPTER}'`);
    const pending = capture(cmd.send()); let rows = await blocked(1);
    if (phase === 'commit') {
      await tailGate.query(`BEGIN; SELECT 1 FROM storyos.command_idempotency WHERE ${scope} AND idempotency_key='${cmd.key}' FOR UPDATE`);
      await gate.query('ROLLBACK'); rows = await blocked(1, 'UPDATE storyos.command_idempotency');
    }
    record('at_cut', await counts());
    await cut(mode, rows); record('interrupted_reply', await pending); await gate.query('ROLLBACK'); await tailGate.query('ROLLBACK');
    if (mode === 'server') await start();
    assert.deepEqual(await chapter(), body); record('before_recovery', { before, after: await counts() });
    if (phase !== 'admission') {
      const outcome = await cmd.outcome(); assert.equal(outcome.outcome.outcome_kind, 'committed');
      const ack = await cmd.send(); assert.deepEqual(ack, outcome.outcome.response);
      assert.equal((await counts()).commits, before.commits + 1); record('recovered_once', ack);
    } else {
      assert.deepEqual(await counts(), before); const ack = await cmd.send(); assert.deepEqual(await cmd.send(), ack); record('retried_once', ack);
    }
    await smoke(writer);
  } else if (scenario === 'concurrent-rename' || scenario === 'concurrent-retry' || scenario === 'concurrent-rename-restart') {
    if (scenario.endsWith('-restart')) { await stop(); await start(); }
    const first = await rename('a', 'First'), second = scenario === 'concurrent-retry' ? first : await rename('b', 'Second');
    await hold('projects'); const p1 = capture(first.send()); await blocked(1);
    const p2 = capture(second.send()); await blocked(2); await gate.query('ROLLBACK');
    const replies = await Promise.all([p1, p2]); record('concurrent_results', replies);
    const ack = await first.send(); assert.deepEqual(await first.send(), ack);
    record('settled_counts', await counts()); writer = await open(); await smoke(writer);
    record('verdict', replies.every(x => x.ok) ? 'holds' : 'fails');
  } else if (scenario === 'concurrent-author') {
    writer = await open(); const seed = await edit(writer, 'SEED'); const ack = await seed.send();
    const before = await counts(), cmd = await edit(writer, 'NEXT'), ren = await rename('b', 'Concurrent');
    await hold('authoritative_heads', `${scope} AND manuscript_object_id='${CHAPTER}'`);
    const pending = capture(cmd.send()); await blocked(1); const renamed = await ren.send();
    await gate.query('ROLLBACK'); record('concurrent_author_reply', await pending);
    record('concurrent_rename_reply', renamed); record('reconcile_author', await capture(cmd.outcome()));
    assert.deepEqual(await seed.send(), ack);
    assert.ok((await chapter()).chapter.current_revision.body.startsWith(ack.effect.authoritative_revision.body));
    record('acknowledged_seed_preserved', { ack, before, after: await counts() }); await smoke(writer);
  } else if (scenario === 'session-replay') {
    const before = writer.value;
    await smoke(writer); const observer = await open('b'), take = await takeover(observer, before.writer.writer_generation);
    await take.send(); const authority = await counts();
    for (const mode of ['concurrent-history', 'restart']) {
      if (mode === 'restart') { await stop(); await start(); }
      const replay = await writer.cmd.send(); assert.deepEqual(await counts(), authority);
      record('session_replay_' + mode, { original: before, replay, equal: isDeepStrictEqual(before, replay) });
    }
    record('verdict', isDeepStrictEqual(before, await writer.cmd.send()) ? 'holds' : 'fails');
    await smoke(observer);
  } else if (scenario.startsWith('takeover-')) {
    const mode = scenario.slice('takeover-'.length), observer = await open('b');
    const old = await edit(writer, 'OLD'), take = await takeover(observer, writer.value.writer.writer_generation);
    const before = await counts(); await hold('authoritative_heads', `${scope} AND manuscript_object_id='${CHAPTER}'`);
    const pending = capture(old.send()); const rows = await blocked(1);
    if (mode !== 'concurrent') { await cut(mode, rows); record('interrupted_old', await pending); }
    if (mode === 'server') await start();
    const won = await take.send(); assert.equal(won.result.kind, 'takeover_applied');
    record('takeover_ack', won); record('post_takeover', await counts()); await gate.query('ROLLBACK');
    if (mode === 'concurrent') record('old_reply', await pending);
    const outcome = await capture(old.outcome()); record('old_outcome', outcome);
    const after = await counts(); record('authority_after_old', { before, after, chapter: await chapter() });
    const stale = await command('applyAuthorEdit', '/manuscript/author-edits', {
      ...old.request, correlation_id: id(), completed_intent_record_id: id(), local_intent_sequence: String(serial) });
    record('fresh_stale_request', await capture(stale.send()));
    record('verdict', after.commits === before.commits ? 'holds' : 'fails');
    const next = await edit(observer, '+'); record('winner_next_edit', await next.send());
    const recovered = await open('b'); await (await takeover(recovered, String(after.writer))).send(); await smoke(recovered);
  } else throw new Error('Unknown scenario: ' + scenario);
}
try { await run(); }
catch (e) { record('blocked_or_probe_error', { message: e.message, stack: e.stack }); process.exitCode = 2; }
finally {
  await stop(); await gate.close(); await tailGate.close(); await monitor.close();
  const logs = spawnSync('docker', ['logs', container], { encoding: 'utf8' });
  const errors = (logs.stdout + logs.stderr).split('\n').filter(line => /ERROR:|DETAIL:.*(Process|Reason)|HINT:.*retried/.test(line));
  record('postgres_errors', errors);
  if (process.argv[3]) writeFileSync(process.argv[3], JSON.stringify({ scenario, events }, null, 2) + '\n');
}

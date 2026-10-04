import { execFileSync, spawnSync } from 'node:child_process';
import { randomUUID, createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync, appendFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as api from '../../generated/typescript/storyos-public-release-1/client.mjs';
import { startStoryOSServer, stopStoryOSServer, sessionFetch } from '../../apps/web/test/support/node-integration.ts';
import { RELEASE_1_PROTOCOL_PROFILE as profile } from '../../generated/typescript/storyos-public-release-1/release-profile.mjs';

const root = process.cwd(), scale = Number(process.env.SCALE ?? 30000);
const lab = resolve('prototypes/chinese-long-form-baseline');
const out = resolve(lab, 'out', String(scale));
mkdirSync(out, { recursive: true });
const corpus = JSON.parse(readFileSync(resolve(lab, 'out', `corpus-${scale}.json`)));
const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
const user = '018f0000-0000-7001-8000-000000000001';
const binding = { client_contract_revision: profile.release_identity.web_client_contract_revision, security_policy_revision: 'storyos.web-security-policy.release-1.v1' };
const id = () => { const t = Date.now().toString(16).padStart(12, '0'), r = randomUUID(); return `${t.slice(0, 8)}-${t.slice(8)}-7${r.slice(15, 18)}-${r.slice(19)}`; };
const sql = (query) => execFileSync('docker', ['exec', container, 'psql', '-XAt', '-v', 'ON_ERROR_STOP=1', '-U', 'postgres', '-c', query], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }).trim();
sql("ALTER SYSTEM SET shared_preload_libraries = 'pg_stat_statements'");
execFileSync('docker', ['restart', container]);
execFileSync('docker', ['exec', container, 'sh', '-c', 'until pg_isready -U postgres >/dev/null 2>&1; do sleep 0.1; done'], { timeout: 60000 });
const postgresPort = execFileSync('docker', ['port', container, '5432/tcp'], { encoding: 'utf8' }).trim().split(':').at(-1);
process.env.STORYOS_TEST_DATABASE_URL = `postgres://storyos_runtime:runtime@127.0.0.1:${postgresPort}/postgres`;
sql("CREATE EXTENSION pg_stat_statements; ALTER ROLE storyos_runtime SET session_preload_libraries = 'auto_explain'; ALTER ROLE storyos_runtime SET auto_explain.log_analyze = on; ALTER ROLE storyos_runtime SET auto_explain.log_buffers = on; ALTER ROLE storyos_runtime SET auto_explain.log_timing = off; ALTER ROLE storyos_runtime SET auto_explain.log_format = 'json'; ALTER ROLE storyos_runtime SET auto_explain.log_min_duration = -1;");
let started, baseUrl, options, projectId, session, treeRevision = '1', seq = 0;
let wire = [];
async function start(bind = '127.0.0.1:0') {
  started = await startStoryOSServer({ repositoryRoot: root, serverBinary: resolve('target/release-package/storyos-server'), bind, sessions: { 'baseline-session': user } });
  baseUrl = started.baseUrl;
  options = { baseUrl, fetchImpl: async (input, init) => {
    const response = await sessionFetch(baseUrl, 'baseline-session')(input, init);
    const bytes = Buffer.byteLength(await response.clone().text());
    wire.push({ method: init?.method ?? 'GET', path: new URL(input).pathname, status: response.status, request_bytes: Buffer.byteLength(init?.body ?? ''), response_bytes: bytes });
    return response;
  } };
}
const routes = {
  CreateVolume: ['POST', '/volumes'], CreateChapter: ['POST', '/volumes/{volume_id}/chapters'],
  UpdateVolume: ['PATCH', '/volumes/{volume_id}'], DeleteVolume: ['DELETE', '/volumes/{volume_id}'],
  UpdateChapter: ['PATCH', '/chapters/{chapter_id}'], DeleteChapter: ['DELETE', '/chapters/{chapter_id}'],
  CreateEditorSession: ['POST', '/editor-sessions'], SetCurrentChapter: ['PUT', '/current-chapter'],
  ApplyAuthorEdit: ['POST', '/manuscript/author-edits'], UndoLatestAuthorAction: ['POST', '/author-actions/undo'],
  ExportHumanReadableManuscript: ['POST', '/manuscript/exports'], UpdateProjectAssistance: ['PUT', '/assistance'],
  CreateAgentRun: ['POST', '/agent-runs'], AcceptProposal: ['POST', '/proposals/{proposal_id}/acceptances'],
};
async function command(name, input, pathIds = {}, flat = false) {
  const kebab = name.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase();
  const request = { command_schema: `storyos.command.${kebab}.request.${name === 'CreateAgentRun' ? 'v2' : 'v1'}`,
    ...(flat ? { ...binding, correlation_id: id(), ...input } : { [kebab.replaceAll('-', '_') + '_input']: { ...binding, correlation_id: id(), ...input } }) };
  const key = id(), [method, route] = routes[name];
  const challenge = await api.createProjectCommandChallenge({ ...options, projectId, request: { method, route_template: '/api/v1/projects/{project_id}' + route, command_schema: request.command_schema, canonical_command_digest: await api['digest' + name](request), idempotency_key: key } });
  const response = await api[name[0].toLowerCase() + name.slice(1)]({ ...options, projectId, ...pathIds, request, idempotencyKey: key, antiForgery: challenge.nonce });
  if (response.effect && !['authoritative_applied', 'applied', 'admitted', 'current_chapter_set', 'updated', 'compensated'].includes(response.effect.kind)) console.log(name, JSON.stringify(response.effect));
  if (response.effect?.tree_revision) treeRevision = response.effect.tree_revision;
  return response;
}
function budget() {
  sql(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0 WHERE project_id = '${projectId}'`);
}
async function edit(chapter, paragraphs) {
  let block = chapter.current_revision.blocks[0].manuscript_block_id;
  const primitives = [{ kind: 'replace_block_selection', manuscript_block_id: block, from: 0, to: chapter.current_revision.blocks[0].text.length, text: paragraphs[0] }];
  for (let index = 1; index < paragraphs.length; index++) {
    const next = id();
    primitives.push({ kind: 'split_block', manuscript_block_id: block, offset: paragraphs[index - 1].length, new_manuscript_block_id: next }, { kind: 'replace_block_selection', manuscript_block_id: next, from: 0, to: 0, text: paragraphs[index] });
    block = next;
  }
  return command('ApplyAuthorEdit', {
    editor_session_id: session.editor_session.editor_session_id, writer_generation: session.writer.writer_generation,
    chapter_id: chapter.chapter_id, expected_authoritative_revision_id: chapter.current_revision.revision_id,
    expected_proposal_head_revision_ids: [], target_refs: [`manuscript:${chapter.chapter_id}`], observed_ownership_partition: 'authoritative',
    editor_contract_revision: 'storyos.editor-contract.release-1.v3', undo_group_id: id(), completed_intent_record_id: id(), local_intent_sequence: String(++seq),
    author_edit_units: [{ normalized_primitives: primitives, selection_snapshot: { coordinate_profile: 'storyos.editor.utf16-code-unit.v1', from: 0, to: 0 } }],
  }, {}, true);
}
const query = (name, extra = {}) => api[name]({ ...options, projectId, ...extra });
const statQuery = `SELECT coalesce(json_agg(t), '[]') FROM (SELECT query, calls, rows, shared_blks_hit, shared_blks_read, shared_blks_dirtied, temp_blks_read, temp_blks_written, wal_bytes, total_exec_time FROM pg_stat_statements WHERE userid = (SELECT oid FROM pg_roles WHERE rolname = 'storyos_runtime') ORDER BY query) t`;
async function measure(name, action) {
  budget(); wire = []; sql('SELECT pg_stat_statements_reset()');
  sql(`DO $$ BEGIN RAISE LOG 'BASELINE_START ${scale}:${name}'; END $$`);
  const begin = performance.now();
  let result, error;
  try { result = await action(); } catch (e) { error = { message: String(e), response: e.responseBody }; }
  const elapsed_ms = performance.now() - begin;
  const statements = JSON.parse(sql(statQuery));
  sql(`DO $$ BEGIN RAISE LOG 'BASELINE_END ${scale}:${name}'; END $$`);
  const row = { scale, operation: name, elapsed_ms, error, outcome: result?.effect?.kind ?? result?.status ?? 'query', wire, sql_calls: statements.reduce((n, s) => n + s.calls, 0), sql_rows: statements.reduce((n, s) => n + s.rows, 0), shared_hit: statements.reduce((n, s) => n + s.shared_blks_hit, 0), shared_read: statements.reduce((n, s) => n + s.shared_blks_read, 0), statements };
  writeFileSync(resolve(out, `${name}.json`), JSON.stringify(row, null, 2));
  console.log(JSON.stringify({ scale, operation: name, sql_calls: row.sql_calls, response_bytes: wire.reduce((n, r) => n + r.response_bytes, 0), error, elapsed_ms }));
  return result;
}

try {
  await start();
  const createKey = id(), createInput = { title: `Chinese baseline ${scale}`, ...binding, correlation_id: id() };
  const challenge = await api.createProjectChallenge({ ...options, request: { command_schema: 'storyos.command.create-project.request.v1', create_project_input: createInput, idempotency_key: createKey } });
  projectId = challenge.prospective_project_id;
  await api.createProject({ ...options, idempotencyKey: createKey, antiForgery: challenge.nonce, request: { command_schema: 'storyos.command.create-project.request.v1', prospective_project_id: projectId, create_project_input: createInput } });
  const volumes = [], chapters = [];
  const importStart = performance.now();
  for (let i = 0; i < corpus.chapters.length; i++) {
    budget();
    if (i % 50 === 0) {
      const v = await command('CreateVolume', { title: `第${volumes.length + 1}卷`, expected_tree_revision: treeRevision });
      volumes.push(v.effect.volume_id);
    }
    const created = await command('CreateChapter', { title: corpus.chapters[i].title, expected_tree_revision: treeRevision }, { volumeId: volumes.at(-1) });
    const chapterId = created.effect.chapter_id;
    chapters.push(chapterId);
    if (!session) session = await command('CreateEditorSession', {}, {}, true);
    const before = await query('getChapter', { chapterId });
    if (i > 0) await command('SetCurrentChapter', { chapter_id: chapterId, expected_current_chapter_id: chapters[i - 1], expected_target_revision_id: before.chapter.current_revision.revision_id, editor_session_id: session.editor_session.editor_session_id });
    const result = await edit(before.chapter, corpus.chapters[i].paragraphs);
    if (result.effect.kind !== 'authoritative_applied') throw new Error(JSON.stringify(result));
    if ((i + 1) % 25 === 0 || i === 0) console.log(`Imported ${i + 1}/${corpus.chapters.length} chapters`);
  }
  const loaded = await query('getStatistics');
  writeFileSync(resolve(out, 'import.json'), JSON.stringify({ source: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), scale, projectId, chapters, volumes, session, elapsed_ms: performance.now() - importStart, statistics: loaded, wire_requests: wire.length, body_sha256: createHash('sha256').update(corpus.chapters.map(c => c.paragraphs.join('\n')).join('')).digest('hex') }, null, 2));
  if (Number(loaded.manuscript.character_count) !== scale) throw new Error('Imported scalar count differs');
  const bind = new URL(baseUrl).host;
  await stopStoryOSServer(started.server);
  sql('ALTER ROLE storyos_runtime SET auto_explain.log_min_duration = 0');
  await start(bind);
  await measure('open-project', () => query('getProject'));
  await measure('tree', () => query('getManuscriptTree'));
  await measure('read-chapter', () => query('getChapter', { chapterId: chapters.at(-1) }));
  await measure('statistics', () => query('getStatistics'));
  for (const selection of ['current_chapter', 'manuscript']) await measure(`search-${selection}`, () => query('searchManuscript', { request: { schema_id: 'storyos.query.manuscript-search.request.v1', selection, query_text: '不存在的紫色星河', required_watermark: null } }));
  await measure('session-read', () => query('getEditorSession', { editorSessionId: session.editor_session.editor_session_id }));
  const first = (await query('getChapter', { chapterId: chapters[0] })).chapter;
  await measure('switch-chapter', () => command('SetCurrentChapter', { chapter_id: chapters[0], expected_current_chapter_id: chapters.at(-1), expected_target_revision_id: first.current_revision.revision_id, editor_session_id: session.editor_session.editor_session_id }));
  const saved = await measure('save-input', () => edit(first, [first.current_revision.blocks[0].text + '新']));
  if (saved?.effect.kind === 'authoritative_applied') await measure('undo', () => command('UndoLatestAuthorAction', { expected_author_undo_frontier_sequence: saved.effect.author_action_sequence, expected_authoritative_revision_id: saved.effect.authoritative_revision.revision_id, editor_session_id: session.editor_session.editor_session_id }));
  const volume = await measure('create-volume', () => command('CreateVolume', { title: '测量临时卷', expected_tree_revision: treeRevision }));
  const volumeId = volume?.effect.volume_id;
  const chapter = await measure('create-chapter', () => command('CreateChapter', { title: '测量临时章', expected_tree_revision: treeRevision }, { volumeId }));
  await measure('reorder-chapter', () => command('UpdateChapter', { title: corpus.chapters[0].title, order: String(Math.min(50, chapters.length)), expected_tree_revision: treeRevision }, { chapterId: chapters[0] }));
  await measure('reorder-volume', () => command('UpdateVolume', { title: '测量临时卷', order: '1', expected_tree_revision: treeRevision }, { volumeId }));
  await measure('delete-chapter', () => command('DeleteChapter', { expected_tree_revision: treeRevision }, { chapterId: chapter?.effect.chapter_id }));
  await measure('delete-volume', () => command('DeleteVolume', { expected_tree_revision: treeRevision }, { volumeId }));
  const worker = () => execFileSync(resolve('target/release-package/storyos-worker'), ['--once'], { env: { ...process.env, STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL }, timeout: 120000, encoding: 'utf8' });
  await measure('export', async () => {
    const admitted = await command('ExportHumanReadableManuscript', {});
    worker();
    const result = await query('getHumanReadableManuscriptExport', { exportId: admitted.effect.export_id });
    writeFileSync(resolve(out, 'export-result.json'), JSON.stringify({ status: result.status, content_sha256: result.content_sha256, utf8_bytes: Buffer.byteLength(result.manuscript_utf8 ?? '') }));
    return result;
  });
  budget();
  await command('UpdateProjectAssistance', { availability: 'available', expected_assistance_revision: '0' });
  const run = await command('CreateAgentRun', { conversation: { kind: 'new' }, author_message: { text: 'Revise this passage: keep the voice.' }, working_target: { kind: 'current_chapter', chapter_id: chapters[0] }, instruction: { kind: 'absent' }, cause: { kind: 'author_request' } });
  worker();
  const settled = await query('getAgentRun', { runId: run.effect.run_id });
  writeFileSync(resolve(out, 'proposal-setup.json'), JSON.stringify(settled, null, 2));
  const proposalId = settled.decision?.opened_proposal?.proposal_id;
  if (proposalId) {
    const opened = await measure('proposal-open', () => query('getProposal', { proposalId }));
    const head = (await query('getChapter', { chapterId: chapters[0] })).chapter.current_revision.revision_id;
    await measure('proposal-accept', () => command('AcceptProposal', { proposal_revision_id: opened.proposal.revision_id, validation_receipt_id: opened.proposal.validation_receipt.validation_receipt_id, selected_operation_ids: [opened.proposal.operation_id], expected_authoritative_revision_id: head, editor_session_id: session.editor_session.editor_session_id }, { proposalId }));
  } else {
    for (const name of ['proposal-open', 'proposal-accept']) await measure(name, () => { throw new Error('Proposal setup did not produce an opened Proposal; inspect proposal-setup.json'); });
  }
  writeFileSync(resolve(out, 'context.json'), JSON.stringify({ projectId, chapters, volumes, treeRevision, session, baseUrl }, null, 2));
} finally {
  if (started) await stopStoryOSServer(started.server);
  const logs = spawnSync('docker', ['logs', container], { encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 });
  writeFileSync(resolve(out, 'postgres.log'), logs.stdout + logs.stderr);
}

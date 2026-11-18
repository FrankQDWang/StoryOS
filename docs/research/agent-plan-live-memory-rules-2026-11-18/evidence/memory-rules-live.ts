// The live run of #1700. It attaches to the packaged Server at its address, loads Chapter 1 of
// `fixtures/memory-calibration/` through the public commands of the Web client, sends the two author messages of the
// plan, and reads each measure with read-only SQL. Its only SQL write is the statement C1: it sets `due_at` of the
// capture row of Conversation B to the present, and it must change one row.
//
// STORYOS_LIVE_ROOT=<worktree> STORYOS_TEST_POSTGRES_CONTAINER=<container> \
//   node memory-rules-live.ts <Server URL> <Worker PID> <output directory>
import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = process.env.STORYOS_LIVE_ROOT ?? "";
assert.ok(root, "STORYOS_LIVE_ROOT is necessary");
const client = await import(`${root}/generated/typescript/storyos-public-release-1/client.mjs`);
const { authorEditRequest, runRequest } = await import(`${root}/apps/web/test/support/acceptance.ts`);
const { TestServer, createChapterInput, createProject, createVolumeInput } =
  await import(`${root}/apps/web/test/support/http-harness.ts`);
const { queryStoryOSPostgres } = await import(`${root}/apps/web/test/support/node-integration.ts`);

const [baseUrl = "", workerPid = "", outDir = ""] = process.argv.slice(2);
assert.ok(baseUrl && workerPid && outDir, "usage: memory-rules-live.ts <Server URL> <Worker PID> <output directory>");
const fixtures = `${root}/fixtures/memory-calibration/`;
const COMMANDS_PER_MINUTE = 8;
const LIMITS = { requests: 20, input: 180_000, background: 20_000, author: 40_000 };
mkdirSync(outDir, { recursive: true });
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const lines: string[] = [];
function note(line: string): void {
  const stamped = `${new Date().toISOString()} ${line}`;
  lines.push(stamped);
  console.log(stamped);
  writeFileSync(join(outDir, "loader.log"), `${lines.join("\n")}\n`);
}

const sent: number[] = [];
async function paced<T>(label: string, action: () => Promise<T>): Promise<T> {
  while (sent.length > 0 && Date.now() - (sent[0] ?? 0) >= 60_000) sent.shift();
  if (sent.length >= COMMANDS_PER_MINUTE) await sleep(60_000 - (Date.now() - (sent[0] ?? 0)) + 500);
  sent.push(Date.now());
  note(`command ${label}`);
  return action();
}

const sql = (query: string): Promise<string> => queryStoryOSPostgres(query);
async function rows<T>(query: string): Promise<T[]> {
  return JSON.parse(await sql(`SELECT coalesce(json_agg(row_to_json(q)), '[]'::json) FROM (${query}) AS q;`)) as T[];
}

// The statements of the plan. Each must change its planned row count; another count is a stop.
const statements: string[] = [];
async function makeDue(name: string, projectId: string, filter: string, expected: number): Promise<void> {
  const statement = `UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '${projectId}'::uuid`
    + ` AND ${filter}`;
  const changed = await sql(`WITH moved AS (${statement} RETURNING 1) SELECT count(*) FROM moved;`);
  statements.push(`-- ${name} ${new Date().toISOString()}: ${changed} rows, planned ${expected}\n${statement};`);
  writeFileSync(join(outDir, "due.sql"), `${statements.join("\n")}\n`);
  note(`statement ${name}: ${changed} rows, planned ${expected}`);
  if (Number(changed) !== expected) await stop(`statement ${name} changed ${changed} rows, planned ${expected}`);
}

async function stop(reason: string): Promise<never> {
  process.kill(Number(workerPid), "SIGTERM");
  note(`STOP: ${reason}`);
  if (projectIdForMeasures) await measures(projectIdForMeasures).catch((error) => note(`measures: ${error}`));
  throw new Error(`STOP: ${reason}`);
}
let projectIdForMeasures = "";

// The stops of the plan, read only. The record has each check.
const checks: unknown[] = [];
async function checkLimits(projectId: string, label: string, show = true): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const [usage] = await rows<{ requests: number; input: number | null; background_max: number | null;
    author_max: number | null; failed: number; embedding: number }>(`
    SELECT count(*)::int AS requests, sum((a.payload #>> '{usage,input_tokens}')::int)::int AS input,
      max((a.payload #>> '{usage,input_tokens}')::int) FILTER (WHERE r.run_kind <> 'author') AS background_max,
      max((a.payload #>> '{usage,input_tokens}')::int) FILTER (WHERE r.run_kind = 'author') AS author_max,
      ((SELECT count(*) FROM storyos.memory_captures WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.memory_consolidation_passes WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.agent_runs WHERE ${project} AND run_kind = 'author'
            AND status IN ('refused', 'cancelled')))::int AS failed,
      (SELECT count(*) FROM storyos.embedding_attempts WHERE ${project})::int AS embedding
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project}`);
  assert.ok(usage);
  const over = usage.requests > LIMITS.requests || (usage.input ?? 0) > LIMITS.input
    || (usage.background_max ?? 0) > LIMITS.background || (usage.author_max ?? 0) > LIMITS.author || usage.failed > 0;
  if (show) {
    checks.push({ at: new Date().toISOString(), before: label, ...usage, result: over ? "stop" : "pass" });
    writeFileSync(join(outDir, "checks.json"), `${JSON.stringify(checks, null, 2)}\n`);
    note(`check before ${label}: ${JSON.stringify(usage)}`);
  }
  if (over) await stop(`a limit of the plan before ${label}: ${JSON.stringify(usage)}`);
}

async function until(projectId: string, label: string, check: () => Promise<boolean>, limitMs = 30 * 60_000) {
  const start = Date.now();
  let shown = start;
  for (;;) {
    await checkLimits(projectId, label, false);
    if (await check()) break;
    if (Date.now() - start > limitMs) await stop(`no end of ${label} in ${limitMs / 60_000} minutes`);
    if (Date.now() - shown > 60_000) { note(`wait ${label}`); shown = Date.now(); }
    await sleep(5_000);
  }
  note(`done ${label}`);
}

const count = async (query: string) => Number(await sql(query));
const settledPasses = (projectId: string) => count(`SELECT count(*) FROM storyos.memory_consolidation_passes
  WHERE project_id = '${projectId}'::uuid AND outcome IS NOT NULL;`);
const startedPasses = (projectId: string) => count(`SELECT count(*) FROM storyos.memory_consolidation_passes
  WHERE project_id = '${projectId}'::uuid;`);
const settledCaptures = (projectId: string) => count(`SELECT count(*) FROM storyos.memory_captures
  WHERE project_id = '${projectId}'::uuid AND outcome IS NOT NULL;`);
const startedCaptures = (projectId: string) => count(`SELECT count(*) FROM storyos.memory_captures
  WHERE project_id = '${projectId}'::uuid;`);
const waitingObservations = (projectId: string) => count(`SELECT count(*) FROM storyos.memory_observations
  WHERE project_id = '${projectId}'::uuid AND state = 'waiting';`);
const snapshots: Record<string, unknown> = {};
async function snapshot(projectId: string, label: string) {
  snapshots[label] = await rows(`SELECT DISTINCT ON (memory_entry_id) entry_number, version, state, entry_type,
      origin, title, length(text) AS length, text, flag_reason FROM storyos.memory_entry_versions
     WHERE project_id = '${projectId}'::uuid ORDER BY memory_entry_id, version DESC`);
  writeFileSync(join(outDir, "entries.json"), `${JSON.stringify(snapshots, null, 2)}\n`);
}

type Story = { projectId: string; baseUrl: string; fetchImpl: typeof fetch; key: () => string;
  binding: () => Record<string, unknown>; send: (command: string, body: unknown) => Promise<any>;
  chapterIds: string[]; editorSessionId: string; intent: number; current: string };

async function authorRun(story: Story, label: string, text: string, conversationId?: string): Promise<string> {
  await checkLimits(story.projectId, label);
  const conversation = conversationId === undefined ? { kind: "new" as const }
    : { kind: "existing" as const, conversation_id: conversationId };
  const created = await paced(`createAgentRun ${label}`, () => story.send("createAgentRun", {
    request: runRequest(story.current, story.key(), text, conversation) }));
  const effect = created.response.effect;
  if (effect.kind !== "admitted") await stop(`the Run of ${label} was not admitted: ${created.body}`);
  note(`run ${label} ${effect.run_id}`);
  const status = async () => (await client.getAgentRun({
    baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl, runId: effect.run_id })).status;
  await until(story.projectId, `Run ${label}`, async () => ["completed", "refused", "cancelled"]
    .includes(await status()));
  return effect.run_id;
}

async function edit(story: Story, label: string, primitives: unknown[]) {
  story.intent += 1;
  const ns = (0xe100 + story.intent).toString(16);
  const request = await authorEditRequest({ baseUrl: story.baseUrl }, { prepared: { projectId: story.projectId,
    fetchImpl: story.fetchImpl }, session: { editor_session: { editor_session_id: story.editorSessionId } } },
  primitives, { kind: "authoritative" }, String(story.intent), ns);
  const edited = await paced(`applyAuthorEdit ${label}`, () => story.send("applyAuthorEdit", { request }));
  if (edited.response.effect.kind !== "authoritative_applied") await stop(`${label}: ${edited.body}`);
  return edited.response.effect.authoritative_revision;
}

const paragraphs = (file: string) => readFileSync(join(fixtures, file), "utf8").split("\n")
  .filter((line) => line.trim().length > 0 && !line.startsWith("# "));

// The same steps as the loader of S7-D7: a new Chapter gets the whole text and one split for each paragraph; a
// changed Chapter gets one replacement for each changed paragraph.
async function writeChapter(story: Story, index: number, file: string): Promise<void> {
  const chapterId = story.chapterIds[index] ?? "";
  const session = await client.getEditorSession({ baseUrl: story.baseUrl, projectId: story.projectId,
    fetchImpl: story.fetchImpl, editorSessionId: story.editorSessionId });
  const target = (await client.getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    fetchImpl: story.fetchImpl, chapterId })).chapter;
  if (session.base_snapshot.chapter_id !== chapterId) {
    await paced("setCurrentChapter", () => story.send("setCurrentChapter", { request: {
      command_schema: "storyos.command.set-current-chapter.request.v1",
      set_current_chapter_input: { chapter_id: chapterId, expected_current_chapter_id: session.base_snapshot.chapter_id,
        expected_target_revision_id: target.current_revision.revision_id, editor_session_id: story.editorSessionId,
        ...story.binding() } } }));
  }
  story.current = chapterId;
  const texts = paragraphs(file);
  const blocks = target.current_revision.blocks.filter((block: { text: string }) => block.text.length > 0);
  if (blocks.length > 0) {
    assert.equal(blocks.length, texts.length, `${file} must keep the paragraphs of the Chapter`);
    const changes = texts.flatMap((text, position) => {
      const block = blocks[position];
      return block === undefined || block.text === text ? [] : [{ kind: "replace_block_selection",
        manuscript_block_id: block.manuscript_block_id, from: 0, to: block.text.length, text }];
    });
    await edit(story, file, changes);
    return;
  }
  const whole = await edit(story, `${file} text`, [{ kind: "replace_selection", from: 0, to: 0, text: texts.join("") }]);
  let blockId = whole.blocks[0]?.manuscript_block_id ?? "";
  const splits = texts.slice(0, -1).map((text) => {
    const split = { kind: "split_block", manuscript_block_id: blockId, offset: text.length,
      new_manuscript_block_id: story.key() };
    blockId = split.new_manuscript_block_id;
    return split;
  });
  const written = await edit(story, `${file} paragraphs`, splits);
  assert.deepEqual(written.blocks.map((block: { text: string }) => block.text), texts, `the Blocks of ${file}`);
}

async function measures(projectId: string): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const ended = (run: string) => `(SELECT max(e.recorded_at) FROM storyos.agent_run_events AS e
    WHERE e.${project} AND e.run_id = ${run})`;
  const out = {
    statements,
    checks,
    captures: await rows(`SELECT c.capture_id, c.run_id, c.source_kind, c.chapter_id, c.conversation_id, c.started_at,
        ${ended("c.run_id")} AS ended_at, c.outcome, c.failure_reason,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = c.run_id) AS run_steps,
        (SELECT json_agg(json_build_object('function', t.function_name, 'state', t.state, 'refusal', t.refusal,
           'arguments', t.arguments) ORDER BY t.decision_position, t.call_position)
           FROM storyos.tool_calls AS t WHERE t.${project} AND t.run_id = c.run_id) AS tool_calls,
        (SELECT json_object_agg(g.k, g.n) FROM (SELECT o.kind || ':' || coalesce(o.entry_type, '-') AS k, count(*) AS n
           FROM storyos.memory_observations AS o WHERE o.${project} AND o.capture_id = c.capture_id GROUP BY 1) AS g)
          AS observations
      FROM storyos.memory_captures AS c WHERE c.${project} ORDER BY c.started_at`),
    observations: await rows(`SELECT capture_id, position, kind, entry_type, title, text, entry_number, reason, state
      FROM storyos.memory_observations WHERE ${project} ORDER BY capture_id, position`),
    passes: await rows(`SELECT p.pass_id, p.run_id, p.started_at, ${ended("p.run_id")} AS ended_at, p.outcome,
        p.failure_reason, p.operations, p.observations AS input_items
      FROM storyos.memory_consolidation_passes AS p WHERE p.${project} ORDER BY p.started_at`),
    requests: await rows(`SELECT r.run_kind, a.run_id, a.model_attempt_id, a.decision_position, a.dispatch_state,
        a.payload #>> '{response_reference}' AS response_reference,
        (a.payload #>> '{usage,input_tokens}')::int AS input_tokens,
        (a.payload #>> '{usage,output_tokens}')::int AS output_tokens, a.payload #>> '{decision,kind}' AS decision
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} ORDER BY a.run_id, a.decision_position`),
    author_runs: await rows(`SELECT r.run_id, r.conversation_id, r.status, r.author_message,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = r.run_id) AS run_steps,
        (SELECT json_agg(json_build_object('function', t.function_name, 'arguments', t.arguments, 'state', t.state,
           'refusal', t.refusal) ORDER BY t.decision_position, t.call_position) FROM storyos.tool_calls AS t
          WHERE t.${project} AND t.run_id = r.run_id) AS tool_calls
      FROM storyos.agent_runs AS r WHERE r.${project} AND r.run_kind = 'author' ORDER BY r.run_id`),
    answers: await rows(`SELECT DISTINCT ON (a.run_id) a.run_id, a.payload #>> '{decision,text}' AS answer
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} AND r.run_kind = 'author' ORDER BY a.run_id, a.decision_position DESC`),
    embedding: await rows(`SELECT cause, settlement, count(*)::int AS attempts FROM storyos.embedding_attempts
     WHERE ${project} GROUP BY 1, 2 ORDER BY 1, 2`),
    vectors: await rows(`SELECT chapter_id, count(*)::int AS vectors FROM storyos.passage_vectors WHERE ${project}
     GROUP BY 1 ORDER BY 1`),
    index_work: await rows(`SELECT chapter_id, chapter_order, mark_generation, completed_generation, not_before
      FROM storyos.passage_index_work WHERE ${project} ORDER BY chapter_order`),
    schedules: await rows(`SELECT work_kind, source_id, due_at, held FROM storyos.memory_work_schedules
     WHERE ${project} ORDER BY work_kind, source_id`),
  };
  writeFileSync(join(outDir, "measures.json"), `${JSON.stringify(out, null, 2)}\n`);
}

const PART_A = "请帮我记住一个伏笔：茶馆门板上那张写着“东主有事”的白纸，其实是赵德明替沈怀山写的，这件事后面才揭开。先别改正文。";
const PART_B = "第一章里，顾青禾是怎么跟知遥说爷爷走时的情形的？请引用正文里的原句回答，不要修改正文。"
  + "另外我定下一件事：沈明川最后不会卖掉茶馆，他会在第十章的镇上会议上当众撕掉那份意向书。";

// The Project, the Volume, Chapter 1, and its text.
const project = await paced("createProject", () => createProject(TestServer.at(baseUrl), { title: "临水茶馆",
  resetChallengeBudget: false }));
projectIdForMeasures = project.projectId;
note(`project ${project.projectId}`);
await paced("updateProjectAssistance", () => project.send("updateProjectAssistance", { request: {
  command_schema: "storyos.command.update-project-assistance.request.v1",
  update_project_assistance_input: { availability: "available", expected_assistance_revision: "0", ...project.binding() },
} }));
const volume = await paced("createVolume", async () => project.send("createVolume",
  createVolumeInput(project, "第一卷", await project.treeRevision())));
if (volume.response.effect.kind !== "authoritative_applied") await stop(volume.body);
const volumeId = volume.response.effect.volume_id;
const file = "chapter-01.md";
const title = readFileSync(join(fixtures, file), "utf8").split("\n")[0]?.replace(/^# /, "") ?? file;
const chapter = await paced(`createChapter ${title}`, async () => project.send("createChapter",
  createChapterInput(project, volumeId, title, await project.treeRevision())));
if (chapter.response.effect.kind !== "authoritative_applied") await stop(chapter.body);
const chapterIds = [chapter.response.effect.chapter_id];
const opened = await paced("createEditorSession", () => project.openEditorSession());
const story: Story = Object.assign(project, { chapterIds, editorSessionId: opened.editor_session.editor_session_id,
  intent: 0, current: opened.base_snapshot.chapter_id });
await writeChapter(story, 0, file);
const P = story.projectId;

// Part A: Conversation A asks the Agent to remember one foreshadowing whose subject is in Chapter 1.
await authorRun(story, "part A", PART_A);
await snapshot(P, "after part A");

// Part B: Conversation B has one question about Chapter 1 and one decision about the story.
const runB = await authorRun(story, "part B", PART_B);
const conversationB = await sql(`SELECT conversation_id FROM storyos.agent_runs WHERE run_id = '${runB}'::uuid;`);
await snapshot(P, "after part B");

// The capture of Conversation B. A capture with observations makes the pass due by itself.
await checkLimits(P, "statement C1");
await makeDue("C1", P, `work_kind = 'conversation_capture' AND source_id = '${conversationB}'::uuid`, 1);
await until(P, "capture of Conversation B", async () => (await settledCaptures(P)) >= 1);
const observations = () => count(`SELECT count(*) FROM storyos.memory_observations WHERE project_id = '${P}'::uuid;`);
await until(P, "pass", async () => ((await startedPasses(P)) >= 1 || (await observations()) === 0)
  && (await settledPasses(P)) === (await startedPasses(P)));
await snapshot(P, "after the pass");
await checkLimits(P, "the end");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
await measures(P);

// The raw evidence: each wire payload with its own query, the Model Attempts, the ToolCalls, and the guard log.
const attempts = await rows<{ model_attempt_id: string }>(`SELECT model_attempt_id FROM storyos.model_attempts
  WHERE project_id = '${P}'::uuid ORDER BY model_attempt_id`);
mkdirSync(join(outDir, "wire"), { recursive: true });
for (const [index, attempt] of attempts.entries()) {
  const text = await sql(`SELECT payload #>> '{wire,serialized_payload}' FROM storyos.model_attempts
    WHERE model_attempt_id = '${attempt.model_attempt_id}'::uuid;`);
  writeFileSync(join(outDir, "wire", `M${index + 1}.json`), `${JSON.stringify(JSON.parse(text), null, 2)}\n`);
}
const dump = async (name: string, query: string) => writeFileSync(join(outDir, name),
  `${JSON.stringify(await rows(query), null, 2)}\n`);
await dump("model-attempts.json", `SELECT a.model_attempt_id, a.run_id, r.run_kind, r.model_registration_revision,
  a.decision_position, a.attempt_role, a.dispatch_state, a.payload #>> '{response_reference}' AS response_reference,
  a.payload -> 'usage' AS usage FROM storyos.model_attempts AS a
  JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id) WHERE a.project_id = '${P}'::uuid
  ORDER BY a.model_attempt_id`);
await dump("tool-calls.json", `SELECT tool_call_id, run_id, decision_position, call_position, function_name, state,
  refusal, arguments FROM storyos.tool_calls WHERE project_id = '${P}'::uuid ORDER BY run_id, decision_position, call_position`);
await dump("guard-log.json", `SELECT at, kind, ref, role, position FROM live_guard.log ORDER BY at`);
note("measures written");

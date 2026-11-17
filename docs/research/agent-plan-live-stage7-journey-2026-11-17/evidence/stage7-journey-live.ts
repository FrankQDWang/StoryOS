// The live run of S7-E1b (#1676). It attaches to the packaged Server at its address, loads Chapters 1 to 3 of
// `fixtures/memory-calibration/` through the public commands of the Web client, sends the author messages of the
// plan, and reads each measure with read-only SQL. Its only SQL writes are the statements C1, A1, B1, A2, and B2:
// each sets `due_at` of schedule rows of the Project to the present, and each must change its planned row count.
//
// STORYOS_TEST_POSTGRES_CONTAINER=<container> node stage7-journey-live.ts <Server URL> <Worker PID> <output directory>
import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = "<worktree>";
const client = await import(`${root}/generated/typescript/storyos-public-release-1/client.mjs`);
const { authorEditRequest, runRequest } = await import(`${root}/apps/web/test/support/acceptance.ts`);
const { TestServer, createChapterInput, createProject, createVolumeInput } =
  await import(`${root}/apps/web/test/support/http-harness.ts`);
const { queryStoryOSPostgres } = await import(`${root}/apps/web/test/support/node-integration.ts`);

const [baseUrl = "", workerPid = "", outDir = ""] = process.argv.slice(2);
assert.ok(baseUrl && workerPid && outDir, "usage: stage7-journey-live.ts <Server URL> <Worker PID> <output directory>");
const fixtures = `${root}/fixtures/memory-calibration/`;
const COMMANDS_PER_MINUTE = 8;
const LIMITS = { requests: 45, input: 600_000, background: 60_000, author: 150_000 };
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

const STEP1 = "请帮我记住一个伏笔：柜台底下那只上了锁的樟木箱，钥匙其实一直在赵德明手里。后面第八章才会打开它，"
  + "里面的信和收据会证明顾青禾出了一半房钱。正文现在还没写到，先别改正文。";
const STEP3 = "知遥第一次看到那只上锁的箱子时是什么情形？钥匙后面会在谁手里？请引用正文里的原句回答，不要修改正文。";

// The Project, the Volume, Chapters 1 to 3, and their text.
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
const chapterIds: string[] = [];
const files = ["chapter-01.md", "chapter-02.md", "chapter-03.md"];
for (const file of files) {
  const title = readFileSync(join(fixtures, file), "utf8").split("\n")[0]?.replace(/^# /, "") ?? file;
  const chapter = await paced(`createChapter ${title}`, async () => project.send("createChapter",
    createChapterInput(project, volumeId, title, await project.treeRevision())));
  if (chapter.response.effect.kind !== "authoritative_applied") await stop(chapter.body);
  chapterIds.push(chapter.response.effect.chapter_id);
}
note(`chapters ${chapterIds.join(" ")}`);
const opened = await paced("createEditorSession", () => project.openEditorSession());
const story: Story = Object.assign(project, { chapterIds, editorSessionId: opened.editor_session.editor_session_id,
  intent: 0, current: opened.base_snapshot.chapter_id });
for (const [index, file] of files.entries()) await writeChapter(story, index, file);
const P = story.projectId;

// Step 1: Conversation A asks the Agent to remember a foreshadowing.
const run1 = await authorRun(story, "step 1", STEP1);
const conversationA = await sql(`SELECT conversation_id FROM storyos.agent_runs WHERE run_id = '${run1}'::uuid;`);
await snapshot(P, "after step 1");

// Captures of Conversation A and of Chapters 1 to 3. P1 starts by itself after the first capture with observations.
await checkLimits(P, "statement C1");
await makeDue("C1", P, `work_kind = 'conversation_capture' AND source_id = '${conversationA}'::uuid`, 1);
await until(P, "capture of Conversation A", async () => (await settledCaptures(P)) >= 1);
await checkLimits(P, "statement A1");
await makeDue("A1", P, `work_kind = 'chapter_capture' AND chapter_id IN (${chapterIds.map((id) => `'${id}'::uuid`)
  .join(", ")})`, 3);
await until(P, "captures of Chapters 1 to 3", async () => (await settledCaptures(P)) >= 4);
// With no observation, no pass starts (a finding, not a stop).
const observations = () => count(`SELECT count(*) FROM storyos.memory_observations WHERE project_id = '${P}'::uuid;`);
await until(P, "pass P1", async () => ((await startedPasses(P)) >= 1 || (await observations()) === 0)
  && (await settledPasses(P)) === (await startedPasses(P)));
await snapshot(P, "after P1");
// B1 only after P1 settled, and only for observations that wait.
if (await waitingObservations(P) > 0) {
  await checkLimits(P, "statement B1");
  const passes = await startedPasses(P);
  await makeDue("B1", P, `work_kind = 'consolidation'`, 1);
  await until(P, "pass P2", async () => (await settledPasses(P)) > passes);
  await snapshot(P, "after P2");
} else note("B1 not sent: no observation waits after P1");

// The change of Chapter 3, its capture (A2), and its pass (B2).
const before3 = await startedCaptures(P);
await writeChapter(story, 2, "chapter-03-changed.md");
await checkLimits(P, "statement A2");
await makeDue("A2", P, `work_kind = 'chapter_capture' AND chapter_id = '${chapterIds[2]}'::uuid`, 1);
await until(P, "capture of the changed Chapter 3", async () => (await settledCaptures(P)) > before3);
if (await waitingObservations(P) > 0) {
  await until(P, "no pass in progress", async () => (await settledPasses(P)) === (await startedPasses(P)));
  if (await waitingObservations(P) > 0) {
    await checkLimits(P, "statement B2");
    const passes = await startedPasses(P);
    await makeDue("B2", P, `work_kind = 'consolidation'`, 1);
    await until(P, "pass P3", async () => (await settledPasses(P)) > passes);
  }
} else note("B2 not sent: the capture of the changed Chapter 3 left no observation that waits");
await snapshot(P, "after the change");

// Step 3: the index of Chapters 1 and 2 is complete (its quiet time ends by itself), then Conversation B.
await until(P, "index of Chapters 1 and 2", async () => (await count(`SELECT count(*) FROM storyos.passage_index_work
  WHERE project_id = '${P}'::uuid AND chapter_id IN ('${chapterIds[0]}'::uuid, '${chapterIds[1]}'::uuid)
    AND completed_generation = mark_generation;`)) === 2, 20 * 60_000);
await authorRun(story, "step 3", STEP3);
await snapshot(P, "after step 3");
await checkLimits(P, "the end");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
await measures(P);
note("measures written");

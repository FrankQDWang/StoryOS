// The live check of #435 (ADR 0103 section 2). One Project gets the 10 Chapters of `fixtures/memory-calibration/`
// in Chapter order through the public commands. Then three Runs of the author follow, each in a new Project
// Conversation:
//   Run A, work with several steps, with Chapter 3 as the Current Chapter.
//   Run B, a simple question, with Chapter 1 as the Current Chapter.
//   Run C, between the two, with Chapter 2 as the Current Chapter.
// While each Run works, the script reads `getAgentRun` at each poll and keeps each new `plan`. The script makes no SQL
// write. It reads each measure with read-only SQL, and it stops the Worker after Run C, so no Memory work of the quiet
// times (30 minutes and 1 hour) starts.
//
// STORYOS_LIVE_ROOT=<checkout> node plan-live.ts <Server URL> <Worker PID> <output directory>
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
assert.ok(baseUrl && workerPid && outDir, "usage: plan-live.ts <Server URL> <Worker PID> <output directory>");
const fixtures = join(root, "fixtures", "memory-calibration");
const COMMANDS_PER_MINUTE = 8;
// The stop limits of the plan.
const RUN_REQUESTS: Record<string, number> = { A: 30, B: 4, C: 6 };
const LIMITS = { requests: 40, runs: 3, input: 2_000_000, request_input: 150_000 };
mkdirSync(outDir, { recursive: true });
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const lines: string[] = [];
function note(line: string): void {
  const stamped = `${new Date().toISOString()} ${line}`;
  lines.push(stamped);
  console.log(stamped);
  writeFileSync(join(outDir, "loader.log"), `${lines.join("\n")}\n`);
}

// At most 8 commands in one minute, so that no command needs a reset of the challenge rate window.
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

let projectId = "";
async function stop(reason: string): Promise<never> {
  process.kill(Number(workerPid), "SIGTERM");
  note(`STOP: ${reason}`);
  if (projectId) await rawEvidence().catch((error) => note(`raw evidence: ${error}`));
  throw new Error(`STOP: ${reason}`);
}

// The Run of each label, for the limit of the requests of one Run.
const runLabels = new Map<string, string>();

// The stops of the plan, read only: more requests of one Run than its limit, more than 40 requests or 3 AgentRuns in
// all, more than 2,000,000 reported input tokens in all, one request above 150,000 reported input tokens, a Run that
// is refused or cancelled, an Exa or embedding request, or a Run that is not a Run of the author.
async function checkLimits(label: string, show = true): Promise<void> {
  const [usage] = await rows<{ requests: number; input: number | null; request_max: number | null; runs: number;
    other_runs: number; failed: number; exa: number; embedding: number }>(`
    SELECT (SELECT count(*) FROM storyos.model_attempts)::int AS requests,
      (SELECT sum((payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts)::int AS input,
      (SELECT max((payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts)::int AS request_max,
      (SELECT count(*) FROM storyos.agent_runs)::int AS runs,
      (SELECT count(*) FROM storyos.agent_runs WHERE run_kind <> 'author')::int AS other_runs,
      (SELECT count(*) FROM storyos.agent_runs WHERE status IN ('refused', 'cancelled'))::int AS failed,
      (SELECT count(*) FROM storyos.tool_calls WHERE destination_attempt_id IS NOT NULL)::int AS exa,
      (SELECT count(*) FROM storyos.embedding_attempts)::int AS embedding`);
  assert.ok(usage);
  const perRun = await rows<{ run_id: string; n: number }>(`SELECT run_id, count(*)::int AS n
    FROM storyos.model_attempts GROUP BY run_id`);
  if (show) note(`check after ${label}: ${JSON.stringify(usage)} ${JSON.stringify(perRun.map((r) => r.n))}`);
  const overRun = perRun.some((run) => run.n > (RUN_REQUESTS[runLabels.get(run.run_id) ?? ""] ?? 0));
  const over = overRun || usage.requests > LIMITS.requests || usage.runs > LIMITS.runs
    || (usage.input ?? 0) > LIMITS.input || (usage.request_max ?? 0) > LIMITS.request_input
    || usage.other_runs > 0 || usage.failed > 0 || usage.exa > 0 || usage.embedding > 0;
  if (over) await stop(`a limit of the plan after ${label}: ${JSON.stringify(usage)} ${JSON.stringify(perRun)}`);
}

async function until(label: string, check: () => Promise<boolean>, limitMs: number) {
  const start = Date.now();
  for (;;) {
    await checkLimits(label, false);
    if (await check()) break;
    if (readFileSync(join(outDir, "worker.log"), "utf8").includes("live_guard")) {
      await stop(`the request guard refused a request of ${label}`);
    }
    if (Date.now() - start > limitMs) await stop(`no end of ${label} in ${limitMs / 60_000} minutes`);
    await sleep(3_000);
  }
  note(`done ${label}`);
}

type Story = { projectId: string; baseUrl: string; fetchImpl: typeof fetch; key: () => string;
  binding: () => Record<string, unknown>; send: (command: string, body: unknown) => Promise<any>;
  treeRevision: () => Promise<string>; chapterIds: string[]; editorSessionId: string; intent: number;
  current: string };

// Each new plan that `getAgentRun` gave while a Run worked, and the plan after its end.
type PlanRead = { at: string; status: string; plan: unknown };
const planReads: Record<string, PlanRead[]> = {};

// One Run of the author in a new Conversation, to its settlement.
async function authorRun(story: Story, label: string, text: string, limitMinutes: number): Promise<string> {
  await checkLimits(`before Run ${label}`);
  const request = runRequest(story.current, story.key(), text, { kind: "new" as const });
  const created = await paced(`createAgentRun ${label}`, () => story.send("createAgentRun", { request }));
  const effect = created.response.effect;
  if (effect.kind !== "admitted") await stop(`the Run of ${label} was not admitted: ${created.body}`);
  runLabels.set(effect.run_id, label);
  note(`run ${label} ${effect.run_id}`);
  const reads: PlanRead[] = [];
  planReads[label] = reads;
  const read = async () => {
    const run = await client.getAgentRun({ baseUrl: story.baseUrl, projectId: story.projectId,
      fetchImpl: story.fetchImpl, runId: effect.run_id });
    const plan = run.plan ?? null;
    const last = reads.at(-1);
    if (!last || JSON.stringify(last.plan) !== JSON.stringify(plan) || last.status !== run.status) {
      reads.push({ at: new Date().toISOString(), status: run.status, plan });
      note(`plan ${label} ${run.status} ${JSON.stringify(plan)}`);
    }
    return run.status as string;
  };
  await until(`Run ${label}`, async () => ["completed", "refused", "cancelled"].includes(await read()),
    limitMinutes * 60_000);
  await checkLimits(`Run ${label}`);
  return effect.run_id;
}

// One author edit of the Current Chapter of the editor session.
async function edit(story: Story, label: string, primitives: unknown[]) {
  story.intent += 1;
  const ns = (0xe400 + story.intent).toString(16);
  const request = await authorEditRequest({ baseUrl: story.baseUrl }, { prepared: { projectId: story.projectId,
    fetchImpl: story.fetchImpl }, session: { editor_session: { editor_session_id: story.editorSessionId } } },
  primitives, { kind: "authoritative" }, String(story.intent), ns);
  const edited = await paced(`applyAuthorEdit ${label}`, () => story.send("applyAuthorEdit", { request }));
  if (edited.response.effect.kind !== "authoritative_applied") await stop(`${label}: ${edited.body}`);
  return edited.response.effect.authoritative_revision;
}

const paragraphs = (file: string) => readFileSync(join(fixtures, file), "utf8").split("\n")
  .filter((line) => line.trim().length > 0 && !line.startsWith("# "));
const titleOf = (file: string) => readFileSync(join(fixtures, file), "utf8").split("\n")[0]?.replace(/^# /, "") ?? file;

// Makes the Chapter of `index` the Current Chapter of the editor session.
async function setCurrent(story: Story, index: number) {
  const chapterId = story.chapterIds[index] ?? "";
  const session = await client.getEditorSession({ baseUrl: story.baseUrl, projectId: story.projectId,
    fetchImpl: story.fetchImpl, editorSessionId: story.editorSessionId });
  const target = (await client.getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    fetchImpl: story.fetchImpl, chapterId })).chapter;
  if (session.base_snapshot.chapter_id !== chapterId) {
    await paced(`setCurrentChapter ${index + 1}`, () => story.send("setCurrentChapter", { request: {
      command_schema: "storyos.command.set-current-chapter.request.v1",
      set_current_chapter_input: { chapter_id: chapterId, expected_current_chapter_id: session.base_snapshot.chapter_id,
        expected_target_revision_id: target.current_revision.revision_id, editor_session_id: story.editorSessionId,
        ...story.binding() } } }));
  }
  story.current = chapterId;
  return target;
}

// Makes the empty Chapter the Current Chapter, and then writes the paragraphs of the file: the whole text, then one
// split for each paragraph (the loader of S7-D7).
async function writeChapter(story: Story, index: number, file: string): Promise<void> {
  const target = await setCurrent(story, index);
  const texts = paragraphs(file);
  const filled = target.current_revision.blocks.filter((block: { text: string }) => block.text.length > 0);
  if (filled.length > 0) await stop(`${file}: the Chapter is not empty`);
  const whole = await edit(story, `${file} text`, [{ kind: "replace_selection", from: 0, to: 0, text: texts.join("") }]);
  let blockId = whole.blocks[0]?.manuscript_block_id ?? "";
  const splits = texts.slice(0, -1).map((text) => {
    const split = { kind: "split_block", manuscript_block_id: blockId, offset: text.length,
      new_manuscript_block_id: story.key() };
    blockId = split.new_manuscript_block_id;
    return split;
  });
  if (splits.length === 0) return;
  const written = await edit(story, `${file} paragraphs`, splits);
  assert.deepEqual(written.blocks.map((block: { text: string }) => block.text), texts, `the Blocks of ${file}`);
}

async function createChapter(story: Story, volumeId: string, file: string): Promise<string> {
  const chapter = await paced(`createChapter ${titleOf(file)}`, async () => story.send("createChapter",
    createChapterInput(story, volumeId, titleOf(file), await story.treeRevision())));
  if (chapter.response.effect.kind !== "authoritative_applied") await stop(chapter.body);
  return chapter.response.effect.chapter_id;
}

async function rawEvidence(): Promise<void> {
  const attempts = await rows<{ model_attempt_id: string }>(`SELECT model_attempt_id FROM storyos.model_attempts
    ORDER BY model_attempt_id`);
  mkdirSync(join(outDir, "wire"), { recursive: true });
  for (const [index, attempt] of attempts.entries()) {
    const text = await sql(`SELECT payload #>> '{wire,serialized_payload}' FROM storyos.model_attempts
      WHERE model_attempt_id = '${attempt.model_attempt_id}'::uuid;`);
    const payload = text.trim() === "" ? null : JSON.parse(text);
    writeFileSync(join(outDir, "wire", `M${index + 1}.json`), `${JSON.stringify(payload, null, 2)}\n`);
  }
  const dump = async (name: string, query: string) => writeFileSync(join(outDir, name),
    `${JSON.stringify(await rows(query), null, 2)}\n`);
  await dump("model-attempts.json", `SELECT a.model_attempt_id, a.run_id, r.run_kind, r.model_registration_revision,
    a.decision_position, a.attempt_role, a.dispatch_state, a.payload #>> '{execution_profile,mapping_revision}' AS mapping_revision,
    a.payload #>> '{response_reference}' AS response_reference, a.payload -> 'usage' AS usage,
    a.payload -> 'decision' AS decision
    FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
    ORDER BY a.model_attempt_id`);
  await dump("tool-calls.json", `SELECT tool_call_id, run_id, decision_position, call_position, function_name, state,
    refusal, arguments, result_text FROM storyos.tool_calls ORDER BY run_id, decision_position, call_position`);
  await dump("guard-log.json", `SELECT at, kind, ref, role, position FROM live_guard.log ORDER BY at`);
  await dump("runs.json", `SELECT * FROM storyos.agent_runs ORDER BY run_id`);
  writeFileSync(join(outDir, "plan-reads.json"), `${JSON.stringify(planReads, null, 2)}\n`);
}

const RUN_A = "请帮我检查第一章到第五章：找出每一处沈知遥的年龄、或者两件事之间相隔的时间前后对不上的地方，"
  + "把这些地方一一列出来；然后把第三章的第一段改得读起来更顺一些。";
const RUN_B = "第一章的第一句话是什么？";
const RUN_C = "请用三句话概括第二章。";
const files = Array.from({ length: 10 }, (_, index) => `chapter-${String(index + 1).padStart(2, "0")}.md`);

const project = await paced("createProject", () => createProject(TestServer.at(baseUrl), { title: "临水茶馆",
  resetChallengeBudget: false }));
projectId = project.projectId;
note(`project ${projectId}`);
await paced("updateProjectAssistance", () => project.send("updateProjectAssistance", { request: {
  command_schema: "storyos.command.update-project-assistance.request.v1",
  update_project_assistance_input: { availability: "available", expected_assistance_revision: "0", ...project.binding() },
} }));
const volume = await paced("createVolume", async () => project.send("createVolume",
  createVolumeInput(project, "第一卷", await project.treeRevision())));
if (volume.response.effect.kind !== "authoritative_applied") await stop(volume.body);
const volumeId = volume.response.effect.volume_id;
const first = await paced(`createChapter ${titleOf(files[0] ?? "")}`, async () => project.send("createChapter",
  createChapterInput(project, volumeId, titleOf(files[0] ?? ""), await project.treeRevision())));
if (first.response.effect.kind !== "authoritative_applied") await stop(first.body);
const opened = await paced("createEditorSession", () => project.openEditorSession());
const story: Story = Object.assign(project, { chapterIds: [first.response.effect.chapter_id],
  editorSessionId: opened.editor_session.editor_session_id, intent: 0, current: opened.base_snapshot.chapter_id });

// Step 1: the 10 Chapters in order.
for (const [index, file] of files.entries()) {
  if (index > 0) story.chapterIds.push(await createChapter(story, volumeId, file));
  await writeChapter(story, index, file);
}
await checkLimits("the Chapters");

// Steps 2 to 4: the three Runs, each in a new Conversation.
await setCurrent(story, 2);
const runA = await authorRun(story, "A", RUN_A, 40);
await setCurrent(story, 0);
const runB = await authorRun(story, "B", RUN_B, 10);
await setCurrent(story, 1);
const runC = await authorRun(story, "C", RUN_C, 10);
await checkLimits("the end");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");

const runIds: Record<string, string> = { A: runA, B: runB, C: runC };
const measures = {
  runs: await rows(`SELECT r.run_id, r.status, r.model_registration_revision, r.author_message,
      (SELECT DISTINCT a.payload #>> '{execution_profile,mapping_revision}' FROM storyos.model_attempts AS a
        WHERE a.run_id = r.run_id AND a.payload #>> '{execution_profile,mapping_revision}' IS NOT NULL)
        AS mapping_revision,
      (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.run_id = r.run_id)::int AS requests,
      (SELECT sum((a.payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts AS a
        WHERE a.run_id = r.run_id)::int AS input_tokens,
      (SELECT sum((a.payload #>> '{usage,output_tokens}')::int) FROM storyos.model_attempts AS a
        WHERE a.run_id = r.run_id)::int AS output_tokens,
      (SELECT min(e.recorded_at) FROM storyos.agent_run_events AS e WHERE e.run_id = r.run_id) AS started_at,
      (SELECT max(e.recorded_at) FROM storyos.agent_run_events AS e WHERE e.run_id = r.run_id) AS ended_at
    FROM storyos.agent_runs AS r ORDER BY r.run_id`),
  requests: await rows(`SELECT a.run_id, a.model_attempt_id, a.decision_position, a.attempt_role, a.dispatch_state,
      (a.payload #>> '{usage,input_tokens}')::int AS input_tokens,
      (a.payload #>> '{usage,output_tokens}')::int AS output_tokens, a.payload #>> '{decision,kind}' AS decision
    FROM storyos.model_attempts AS a ORDER BY a.model_attempt_id`),
  calls: await rows(`SELECT run_id, decision_position, call_position, function_name, state, refusal
    FROM storyos.tool_calls ORDER BY run_id, decision_position, call_position`),
  plan_calls: await rows(`SELECT run_id, decision_position, call_position, state, refusal, arguments, result_text
    FROM storyos.tool_calls WHERE function_name = 'update_plan' ORDER BY run_id, decision_position, call_position`),
  answers: await rows(`SELECT DISTINCT ON (run_id) run_id, decision_position,
      payload #>> '{decision,text}' AS answer FROM storyos.model_attempts
     WHERE attempt_role = 'decision' ORDER BY run_id, decision_position DESC`),
  final_plans: Object.fromEntries(await Promise.all(Object.entries(runIds).map(async ([label, runId]) => {
    const run = await client.getAgentRun({ baseUrl: story.baseUrl, projectId: story.projectId,
      fetchImpl: story.fetchImpl, runId });
    return [label, { run_id: runId, status: run.status, plan: run.plan ?? null }];
  }))),
  plan_reads: planReads,
};
writeFileSync(join(outDir, "measures.json"), `${JSON.stringify(measures, null, 2)}\n`);
await rawEvidence();
note("measures written");

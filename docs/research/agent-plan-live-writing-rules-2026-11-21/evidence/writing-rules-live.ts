// The live run of #1762. It attaches to the packaged Server at its address, makes three Projects through the public
// commands of the Web client, sends the four author requests of the plan, and reads each measure with read-only SQL.
// It makes no SQL write.
//
// STORYOS_LIVE_ROOT=<worktree> STORYOS_TEST_POSTGRES_CONTAINER=<container> \
//   node writing-rules-live.ts <Server URL> <Worker PID> <output directory>
import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = process.env.STORYOS_LIVE_ROOT ?? "";
assert.ok(root, "STORYOS_LIVE_ROOT is necessary");
const client = await import(`${root}/generated/typescript/storyos-public-release-1/client.mjs`);
const { authorEditRequest, runRequest } = await import(`${root}/apps/web/test/support/acceptance.ts`);
const { TestServer, createChapterInput, createProject, createVolumeInput } =
  await import(`${root}/apps/web/test/support/http-harness.ts`);
const { queryStoryOSPostgres } = await import(`${root}/apps/web/test/support/node-integration.ts`);

const [baseUrl = "", workerPid = "", outDir = ""] = process.argv.slice(2);
assert.ok(baseUrl && workerPid && outDir, "usage: writing-rules-live.ts <Server URL> <Worker PID> <output directory>");
const inputs = join(dirname(fileURLToPath(import.meta.url)), "inputs");
const COMMANDS_PER_MINUTE = 8;
const LIMITS = { requests: 20, input: 150_000, request: 30_000 };
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

const projects: string[] = [];
const inProjects = (alias = "") => `${alias}project_id IN (${projects.map((id) => `'${id}'::uuid`).join(", ")})`;

async function stop(reason: string): Promise<never> {
  process.kill(Number(workerPid), "SIGTERM");
  note(`STOP: ${reason}`);
  if (projects.length > 0) await evidence().catch((error) => note(`evidence: ${error}`));
  throw new Error(`STOP: ${reason}`);
}

// The stops of the plan, read only. The record has each check.
const checks: unknown[] = [];
async function checkLimits(label: string, show = true): Promise<void> {
  if (projects.length === 0) return;
  const [usage] = await rows<{ requests: number; input: number | null; request_max: number | null;
    background: number; failed: number; embedding: number; exa: number }>(`
    SELECT count(*)::int AS requests, sum((a.payload #>> '{usage,input_tokens}')::int)::int AS input,
      max((a.payload #>> '{usage,input_tokens}')::int) AS request_max,
      (count(*) FILTER (WHERE r.run_kind <> 'author'))::int AS background,
      (SELECT count(*) FROM storyos.agent_runs WHERE ${inProjects()} AND run_kind = 'author'
        AND status IN ('refused', 'cancelled'))::int AS failed,
      (SELECT count(*) FROM storyos.embedding_attempts WHERE ${inProjects()})::int AS embedding,
      (SELECT used FROM live_guard.cap WHERE kind = 'exa') AS exa
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE ${inProjects("a.")}`);
  assert.ok(usage);
  const over = usage.requests > LIMITS.requests || (usage.input ?? 0) > LIMITS.input
    || (usage.request_max ?? 0) > LIMITS.request || usage.background > 0 || usage.failed > 0
    || usage.embedding > 0 || usage.exa > 0;
  if (show) {
    checks.push({ at: new Date().toISOString(), before: label, ...usage, result: over ? "stop" : "pass" });
    writeFileSync(join(outDir, "checks.json"), `${JSON.stringify(checks, null, 2)}\n`);
    note(`check before ${label}: ${JSON.stringify(usage)}`);
  }
  if (over) await stop(`a limit of the plan before ${label}: ${JSON.stringify(usage)}`);
}

async function until(label: string, check: () => Promise<boolean>, limitMs = 15 * 60_000) {
  const start = Date.now();
  let shown = start;
  for (;;) {
    await checkLimits(label, false);
    if (await check()) break;
    if (Date.now() - start > limitMs) await stop(`no end of ${label} in ${limitMs / 60_000} minutes`);
    if (Date.now() - shown > 60_000) { note(`wait ${label}`); shown = Date.now(); }
    await sleep(3_000);
  }
  note(`done ${label}`);
}

type Story = { projectId: string; baseUrl: string; fetchImpl: typeof fetch; key: () => string;
  binding: () => Record<string, unknown>; send: (command: string, body: unknown) => Promise<any>;
  treeRevision: () => Promise<string>; openEditorSession: () => Promise<any>;
  editorSessionId: string; intent: number; current: string };

const runs: { label: string; project: string; run_id: string; text: string }[] = [];
async function authorRun(story: Story, label: string, text: string): Promise<string> {
  await checkLimits(label);
  const created = await paced(`createAgentRun ${label}`, () => story.send("createAgentRun", {
    request: runRequest(story.current, story.key(), text, { kind: "new" }) }));
  const effect = created.response.effect;
  if (effect.kind !== "admitted") await stop(`the Run of ${label} was not admitted: ${created.body}`);
  note(`run ${label} ${effect.run_id}`);
  runs.push({ label, project: story.projectId, run_id: effect.run_id, text });
  const status = async () => (await client.getAgentRun({
    baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl, runId: effect.run_id })).status;
  await until(`Run ${label}`, async () => ["completed", "refused", "cancelled"].includes(await status()));
  return effect.run_id;
}

async function edit(story: Story, label: string, primitives: unknown[]) {
  story.intent += 1;
  const ns = (0xe200 + story.intent).toString(16);
  const request = await authorEditRequest({ baseUrl: story.baseUrl }, { prepared: { projectId: story.projectId,
    fetchImpl: story.fetchImpl }, session: { editor_session: { editor_session_id: story.editorSessionId } } },
  primitives, { kind: "authoritative" }, String(story.intent), ns);
  const edited = await paced(`applyAuthorEdit ${label}`, () => story.send("applyAuthorEdit", { request }));
  if (edited.response.effect.kind !== "authoritative_applied") await stop(`${label}: ${edited.body}`);
  return edited.response.effect.authoritative_revision;
}

const read = (file: string) => readFileSync(join(inputs, file), "utf8");
const paragraphs = (file: string) => read(file).split("\n").filter((line) => line.trim().length > 0
  && !line.startsWith("# "));

// A Project with one Volume and one Chapter that has the paragraphs of `file`, one Block for each paragraph.
async function story(title: string, file: string): Promise<Story> {
  const project = await paced(`createProject ${title}`, () => createProject(TestServer.at(baseUrl), { title,
    resetChallengeBudget: false }));
  projects.push(project.projectId);
  note(`project ${title} ${project.projectId}`);
  await paced("updateProjectAssistance", () => project.send("updateProjectAssistance", { request: {
    command_schema: "storyos.command.update-project-assistance.request.v1",
    update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
      ...project.binding() } } }));
  const volume = await paced("createVolume", async () => project.send("createVolume",
    createVolumeInput(project, "第一卷", await project.treeRevision())));
  if (volume.response.effect.kind !== "authoritative_applied") await stop(volume.body);
  const chapterTitle = read(file).split("\n")[0]?.replace(/^# /, "") ?? file;
  const chapter = await paced(`createChapter ${chapterTitle}`, async () => project.send("createChapter",
    createChapterInput(project, volume.response.effect.volume_id, chapterTitle, await project.treeRevision())));
  if (chapter.response.effect.kind !== "authoritative_applied") await stop(chapter.body);
  const opened = await paced("createEditorSession", () => project.openEditorSession());
  const made: Story = Object.assign(project, { editorSessionId: opened.editor_session.editor_session_id,
    intent: 0, current: opened.base_snapshot.chapter_id });
  assert.equal(made.current, chapter.response.effect.chapter_id);
  const texts = paragraphs(file);
  const whole = await edit(made, `${file} text`, [{ kind: "replace_selection", from: 0, to: 0, text: texts.join("") }]);
  let blockId = whole.blocks[0]?.manuscript_block_id ?? "";
  const splits = texts.slice(0, -1).map((text) => {
    const split = { kind: "split_block", manuscript_block_id: blockId, offset: text.length,
      new_manuscript_block_id: made.key() };
    blockId = split.new_manuscript_block_id;
    return split;
  });
  const written = splits.length === 0 ? whole : await edit(made, `${file} paragraphs`, splits);
  assert.deepEqual(written.blocks.map((block: { text: string }) => block.text), texts, `the Blocks of ${file}`);
  return made;
}

// The raw evidence and the measures of each Run.
async function evidence(): Promise<void> {
  const where = inProjects();
  const attempts = await rows<{ model_attempt_id: string; run_id: string; decision_position: number }>(`
    SELECT model_attempt_id, run_id, decision_position FROM storyos.model_attempts WHERE ${where}
     ORDER BY run_id, decision_position, model_attempt_id`);
  const block = read("writing-rules-block.txt");
  mkdirSync(join(outDir, "wire"), { recursive: true });
  const perRun: Record<string, unknown> = {};
  for (const run of runs) {
    const own = attempts.filter((attempt) => attempt.run_id === run.run_id);
    const wires: string[] = [];
    for (const [index, attempt] of own.entries()) {
      const text = await sql(`SELECT payload #>> '{wire,serialized_payload}' FROM storyos.model_attempts
        WHERE model_attempt_id = '${attempt.model_attempt_id}'::uuid;`);
      const name = `${run.label}-M${index + 1}.json`;
      writeFileSync(join(outDir, "wire", name), `${JSON.stringify(JSON.parse(text), null, 2)}\n`);
      wires.push(text);
    }
    const first = wires[0] === undefined ? undefined : JSON.parse(wires[0]);
    const instructions: string = first?.instructions ?? "";
    perRun[run.label] = {
      ...run,
      first_request_has_block: instructions.includes(block),
      block_count_in_first_request: instructions.split(block).length - 1,
      requests_with_block: wires.filter((wire) => (JSON.parse(wire).instructions ?? "").includes(block)).length,
      requests: wires.length,
      run: (await rows(`SELECT status, model_registration_revision FROM storyos.agent_runs
        WHERE run_id = '${run.run_id}'::uuid`))[0],
      mapping_revisions: await rows(`SELECT DISTINCT payload #>> '{execution_profile,mapping_revision}' AS mapping_revision
        FROM storyos.model_attempts WHERE run_id = '${run.run_id}'::uuid`),
      attempts: await rows(`SELECT decision_position, attempt_role, dispatch_state,
          payload #>> '{response_reference}' AS response_reference,
          (payload #>> '{usage,input_tokens}')::int AS input_tokens,
          (payload #>> '{usage,output_tokens}')::int AS output_tokens, payload #>> '{decision,kind}' AS decision
        FROM storyos.model_attempts WHERE run_id = '${run.run_id}'::uuid ORDER BY decision_position`),
      tool_calls: await rows(`SELECT decision_position, call_position, function_name, state, refusal, arguments
        FROM storyos.tool_calls WHERE run_id = '${run.run_id}'::uuid ORDER BY decision_position, call_position`),
      candidates: await rows(`SELECT p.proposal_id, o.manuscript_block_id, o.candidate_text
        FROM storyos.proposals AS p JOIN storyos.proposal_operations AS o
          USING (owner_user_id, project_id, proposal_id)
        WHERE p.source_run_id = '${run.run_id}'::uuid ORDER BY p.proposal_id, o.manuscript_block_id`),
      final_answer: (await rows<{ answer: string }>(`SELECT payload #>> '{decision,text}' AS answer
        FROM storyos.model_attempts WHERE run_id = '${run.run_id}'::uuid ORDER BY decision_position DESC
        LIMIT 1`))[0]?.answer ?? null,
    };
  }
  writeFileSync(join(outDir, "measures.json"), `${JSON.stringify({ checks, runs: perRun }, null, 2)}\n`);
  const dump = async (name: string, query: string) => writeFileSync(join(outDir, name),
    `${JSON.stringify(await rows(query), null, 2)}\n`);
  await dump("model-attempts.json", `SELECT a.model_attempt_id, a.run_id, r.run_kind, r.model_registration_revision,
    a.decision_position, a.attempt_role, a.dispatch_state, a.payload #>> '{response_reference}' AS response_reference,
    a.payload -> 'usage' AS usage FROM storyos.model_attempts AS a
    JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id) WHERE ${inProjects("a.")}
    ORDER BY a.run_id, a.decision_position`);
  await dump("tool-calls.json", `SELECT tool_call_id, run_id, decision_position, call_position, function_name, state,
    refusal, arguments FROM storyos.tool_calls WHERE ${where} ORDER BY run_id, decision_position, call_position`);
  await dump("guard-log.json", `SELECT at, kind, ref, role, position FROM live_guard.log ORDER BY at`);
  await dump("schedules.json", `SELECT project_id, work_kind, source_id, due_at, held FROM storyos.memory_work_schedules
    WHERE ${where} ORDER BY project_id, work_kind, source_id`);
  note("evidence written");
}

const NEXT_SCENE = "请接着这一章的结尾，写下一场。";
const REVISION = "请改写这一段，保留其中的情节。";
const QUESTION = "这一章里，林舟为什么一直不肯离开渡口？请直接回答，不用修改正文。";

const a = await story("渡口 A", "chapter-ferry.md");
const b = await story("渡口 B", "chapter-ferry.md");
const instruction = await paced("updateProjectInstruction", () => b.send("updateProjectInstruction", { request: {
  command_schema: "storyos.command.update-project-instruction.request.v1",
  update_project_instruction_input: { text: read("project-instruction.txt"), expected_instruction_revision: "0",
    ...b.binding() } } }));
if (instruction.response.effect.kind !== "authoritative_applied") await stop(instruction.body);
const c = await story("归来 C", "chapter-habits.md");

await authorRun(a, "R1", NEXT_SCENE);
await authorRun(b, "R2", NEXT_SCENE);
await authorRun(c, "R3", REVISION);
await authorRun(a, "R4", QUESTION);
await checkLimits("the end");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
await evidence();

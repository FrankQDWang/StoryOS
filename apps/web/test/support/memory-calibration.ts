// The live calibration run of S7-D7 (ADR 0094 section 8). It attaches to the packaged Server at its address, loads
// the sample of `fixtures/memory-calibration/` through the public commands of the Web client, sends the messages of
// the Conversation script, and reads the measures with read-only SQL. Its only SQL writes are the statements A, B,
// and C: each one sets the due time of one schedule row to the present, and it must change exactly 1 row.
//
// STORYOS_TEST_POSTGRES_CONTAINER=<container> node apps/web/test/support/memory-calibration.ts \
//   <Server URL> <Worker PID> <output directory>
import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import * as client from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AuthorEditPrimitive } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { authorEditRequest, runRequest } from "./acceptance.ts";
import { TestServer, createChapterInput, createProject, createVolumeInput } from "./http-harness.ts";
import type { TestProject } from "./http-harness.ts";
import { queryStoryOSPostgres } from "./node-integration.ts";

const [baseUrl = "", workerPid = "", outDir = ""] = process.argv.slice(2);
assert.ok(baseUrl && workerPid && outDir, "usage: memory-calibration.ts <Server URL> <Worker PID> <output directory>");
const fixtures = fileURLToPath(new URL("../../../../fixtures/memory-calibration/", import.meta.url));
const COMMANDS_PER_MINUTE = 8;
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

const sql = (query: string) => queryStoryOSPostgres(query);
async function rows<T>(query: string): Promise<T[]> {
  return JSON.parse(await sql(`SELECT coalesce(json_agg(row_to_json(q)), '[]'::json) FROM (${query}) AS q;`)) as T[];
}

// The statements A (a Chapter), B (the pass), and C (a Project Conversation).
const statements: string[] = [];
async function makeDue(kind: "chapter_capture" | "consolidation" | "conversation_capture", projectId: string,
  sourceId: string): Promise<void> {
  const statement = `UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '${projectId}'::uuid`
    + ` AND work_kind = '${kind}' AND source_id = '${sourceId}'::uuid`;
  const changed = await sql(`WITH moved AS (${statement} RETURNING 1) SELECT count(*) FROM moved;`);
  statements.push(`-- ${new Date().toISOString()}: ${changed} row\n${statement};`);
  writeFileSync(join(outDir, "due.sql"), `${statements.join("\n")}\n`);
  note(`due ${kind} ${sourceId}: ${changed} row`);
  assert.equal(changed, "1", `the statement must change exactly 1 row: ${statement}`);
}

// Each poll also checks the stops of the plan, so that no request follows a request above a limit.
async function until(projectId: string, label: string, check: () => Promise<boolean>, limitMs = 30 * 60_000) {
  const start = Date.now();
  let shown = start;
  for (;;) {
    await checkLimits(projectId, false);
    if (await check()) break;
    if (Date.now() - start > limitMs) throw new Error(`STOP: no end of ${label} in ${limitMs / 60_000} minutes`);
    if (Date.now() - shown > 60_000) { note(`wait ${label}`); shown = Date.now(); }
    await sleep(5_000);
  }
  note(`done ${label}`);
}

// The stops of the plan: more than 150 model requests, more than 2,500,000 reported input tokens in all, a capture
// or pass request above 60,000 reported input tokens, a request of a Run of the author above 150,000, or failed
// work: a capture or a pass with the outcome `failed`, or a Run of the author that is `refused` or `cancelled`.
async function checkLimits(projectId: string, show = true): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const [usage] = await rows<{ requests: number; input: number | null; background_max: number | null;
    author_max: number | null; failed: number }>(`
    SELECT count(*)::int AS requests, sum((a.payload #>> '{usage,input_tokens}')::int)::int AS input,
      max((a.payload #>> '{usage,input_tokens}')::int) FILTER (WHERE r.run_kind <> 'author') AS background_max,
      max((a.payload #>> '{usage,input_tokens}')::int) FILTER (WHERE r.run_kind = 'author') AS author_max,
      ((SELECT count(*) FROM storyos.memory_captures WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.memory_consolidation_passes WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.agent_runs WHERE ${project} AND run_kind = 'author'
            AND status IN ('refused', 'cancelled')))::int AS failed
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project}`);
  assert.ok(usage);
  if (show) note(`usage ${JSON.stringify(usage)}`);
  if (usage.requests > 150 || (usage.input ?? 0) > 2_500_000 || (usage.background_max ?? 0) > 60_000
    || (usage.author_max ?? 0) > 150_000 || usage.failed > 0) {
    process.kill(Number(workerPid), "SIGTERM");
    throw new Error(`STOP: a stop of the plan: ${JSON.stringify(usage)}`);
  }
}

const settledPasses = async (projectId: string) => Number(await sql(`SELECT count(*)
  FROM storyos.memory_consolidation_passes WHERE project_id = '${projectId}'::uuid AND outcome IS NOT NULL;`));
type Source = { kind: "chapter_capture" | "conversation_capture"; id: string };
const captures = async (projectId: string, source: Source, onlySettled: boolean) => Number(await sql(`SELECT count(*)
  FROM storyos.memory_captures WHERE project_id = '${projectId}'::uuid
   AND ${source.kind === "chapter_capture" ? "chapter_id" : "conversation_id"} = '${source.id}'::uuid
   ${onlySettled ? "AND outcome IS NOT NULL" : ""};`));

// One capture of the source, then one pass when the pass row has a due time. When the capture started by time
// before the statement, the run counts it and sends no statement. The entry set after the pass goes to
// `entries.json`.
async function captureThenPass(projectId: string, label: string, source: Source, before: number): Promise<void> {
  const passes = await settledPasses(projectId);
  if (await captures(projectId, source, false) > before) note(`capture ${label} started by time: no statement`);
  else await makeDue(source.kind, projectId, source.id);
  await until(projectId, `capture ${label}`, async () => (await captures(projectId, source, true)) > before);
  await checkLimits(projectId);
  const due = await sql(`SELECT coalesce((SELECT CASE WHEN due_at IS NULL THEN 'none' WHEN due_at > now()
      THEN 'later' ELSE 'now' END FROM storyos.memory_work_schedules WHERE project_id = '${projectId}'::uuid
      AND work_kind = 'consolidation'), 'none');`);
  const passStarted = Number(await sql(`SELECT count(*) FROM storyos.memory_consolidation_passes
    WHERE project_id = '${projectId}'::uuid;`)) > passes;
  if (due === "none" && !passStarted) { note(`no pass after ${label}: the pass row has no due time`); return; }
  if (due === "later") await makeDue("consolidation", projectId, projectId);
  await until(projectId, `pass ${label}`, async () => (await settledPasses(projectId)) > passes);
  await checkLimits(projectId);
  snapshots[label] = await rows(`SELECT DISTINCT ON (memory_entry_id) entry_number, version, state, entry_type,
      origin, title, length(text) AS length, text, flag_reason FROM storyos.memory_entry_versions
     WHERE project_id = '${projectId}'::uuid ORDER BY memory_entry_id, version DESC`);
  writeFileSync(join(outDir, "entries.json"), `${JSON.stringify(snapshots, null, 2)}\n`);
}
const snapshots: Record<string, unknown> = {};

type Story = TestProject & { chapterIds: string[]; editorSessionId: string; intent: number; current: string };

// One Run of the author, to its settlement.
async function authorRun(story: Story, text: string, conversationId?: string): Promise<string> {
  const conversation = conversationId === undefined ? { kind: "new" as const }
    : { kind: "existing" as const, conversation_id: conversationId };
  const created = await paced("createAgentRun", () => story.send("createAgentRun", {
    request: runRequest(story.current, story.key(), text, conversation) }));
  const effect = created.response.effect;
  if (effect.kind !== "admitted") throw new Error(`STOP: the Run was not admitted: ${created.body}`);
  const status = async () => (await client.getAgentRun({
    baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl, runId: effect.run_id })).status;
  await until(story.projectId, `Run ${effect.run_id}`, async () => ["completed", "failed", "cancelled"]
    .includes(await status()));
  await checkLimits(story.projectId);
  return effect.run_id;
}

// One author edit of the Current Chapter of the editor session.
async function edit(story: Story, label: string, primitives: AuthorEditPrimitive[]) {
  story.intent += 1;
  const ns = (0xc000 + story.intent).toString(16);
  const request = await authorEditRequest({ baseUrl: story.baseUrl }, { prepared: { projectId: story.projectId,
    fetchImpl: story.fetchImpl }, session: { editor_session: { editor_session_id: story.editorSessionId } } },
  primitives, { kind: "authoritative" }, String(story.intent), ns);
  const edited = await paced(`applyAuthorEdit ${label}`, () => story.send("applyAuthorEdit", { request }));
  if (edited.response.effect.kind !== "authoritative_applied") throw new Error(`STOP: ${label}: ${edited.body}`);
  return edited.response.effect.authoritative_revision;
}

const paragraphs = (file: string) => readFileSync(join(fixtures, file), "utf8").split("\n")
  .filter((line) => line.trim().length > 0 && !line.startsWith("# "));

// Makes the Chapter the Current Chapter, and then writes the paragraphs of the file. A new Chapter gets the whole
// text and one split for each paragraph. A changed Chapter gets one replacement for each changed paragraph.
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
  const blocks = target.current_revision.blocks.filter((block) => block.text.length > 0);
  if (blocks.length > 0) {
    assert.equal(blocks.length, texts.length, `${file} must keep the paragraphs of the Chapter`);
    const changes = texts.flatMap((text, position): AuthorEditPrimitive[] => {
      const block = blocks[position];
      return block === undefined || block.text === text ? [] : [{ kind: "replace_block_selection",
        manuscript_block_id: block.manuscript_block_id, from: 0, to: block.text.length, text }];
    });
    await edit(story, file, changes);
    return;
  }
  const whole = await edit(story, `${file} text`, [{ kind: "replace_selection", from: 0, to: 0, text: texts.join("") }]);
  let blockId = whole.blocks[0]?.manuscript_block_id ?? "";
  const splits = texts.slice(0, -1).map((text): AuthorEditPrimitive => {
    const split: AuthorEditPrimitive = { kind: "split_block", manuscript_block_id: blockId, offset: text.length,
      new_manuscript_block_id: story.key() };
    blockId = split.new_manuscript_block_id;
    return split;
  });
  const written = await edit(story, `${file} paragraphs`, splits);
  assert.deepEqual(written.blocks.map((block) => block.text), texts, `the Blocks of ${file}`);
}

// The measures of the record, read only.
async function measures(projectId: string): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const ended = (run: string) => `(SELECT max(e.recorded_at) FROM storyos.agent_run_events AS e
    WHERE e.${project} AND e.run_id = ${run})`;
  const out = {
    statements,
    captures: await rows(`SELECT c.capture_id, c.run_id, c.source_kind, c.chapter_id, c.started_at,
        ${ended("c.run_id")} AS ended_at, c.outcome, c.failure_reason,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = c.run_id) AS run_steps,
        (SELECT json_agg(json_build_object('function', t.function_name, 'state', t.state, 'refusal', t.refusal,
           'arguments', t.arguments) ORDER BY t.decision_position, t.call_position)
           FROM storyos.tool_calls AS t WHERE t.${project} AND t.run_id = c.run_id) AS tool_calls,
        (SELECT json_object_agg(g.k, g.n) FROM (SELECT o.kind || ':' || coalesce(o.entry_type, '-') AS k, count(*) AS n
           FROM storyos.memory_observations AS o WHERE o.${project} AND o.capture_id = c.capture_id GROUP BY 1) AS g)
          AS observations
      FROM storyos.memory_captures AS c WHERE c.${project} ORDER BY c.started_at`),
    passes: await rows(`SELECT p.pass_id, p.run_id, p.started_at, ${ended("p.run_id")} AS ended_at, p.outcome,
        p.failure_reason, p.operations, p.observations AS input_items,
        (SELECT bool_and(o.kind = 'conflict') FROM jsonb_array_elements(p.observations) AS i(item)
           JOIN storyos.memory_observations AS o ON o.${project} AND o.capture_id = (i.item ->> 0)::uuid
            AND o.position = (i.item ->> 1)::numeric) AS only_conflict
      FROM storyos.memory_consolidation_passes AS p WHERE p.${project} ORDER BY p.started_at`),
    requests: await rows(`SELECT r.run_kind, a.run_id, a.decision_position, a.dispatch_state,
        (a.payload #>> '{usage,input_tokens}')::int AS input_tokens,
        (a.payload #>> '{usage,output_tokens}')::int AS output_tokens, a.payload #>> '{decision,kind}' AS decision
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} ORDER BY a.run_id, a.decision_position`),
    context_items: await rows(`SELECT DISTINCT m.run_id, m.decision_position, item ->> 'source_class' AS source_class,
        (item ->> 'token_count')::int AS token_count
      FROM storyos.context_assembly_manifests AS m,
        jsonb_path_query(m.payload, '$.** ? (@.source_class == "memory_index" || @.source_class == "chapter_list")')
          AS item
     WHERE m.${project} AND item ? 'token_count' ORDER BY m.run_id, m.decision_position`),
    author_runs: await rows(`SELECT r.run_id, r.conversation_id, r.status, r.author_message,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = r.run_id) AS run_steps,
        (SELECT json_agg(json_build_object('function', t.function_name, 'arguments', t.arguments, 'state', t.state)
           ORDER BY t.decision_position, t.call_position) FROM storyos.tool_calls AS t
          WHERE t.${project} AND t.run_id = r.run_id) AS tool_calls
      FROM storyos.agent_runs AS r WHERE r.${project} AND r.run_kind = 'author' ORDER BY r.run_id`),
    answers: await rows(`SELECT DISTINCT ON (a.run_id) a.run_id, a.payload #>> '{decision,text}' AS answer
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} AND r.run_kind = 'author' ORDER BY a.run_id, a.decision_position DESC`),
    schedules: await rows(`SELECT work_kind, source_id, due_at, held FROM storyos.memory_work_schedules
     WHERE ${project} ORDER BY work_kind, source_id`),
  };
  writeFileSync(join(outDir, "measures.json"), `${JSON.stringify(out, null, 2)}\n`);
}

// The steps of the plan.
const script = JSON.parse(readFileSync(join(fixtures, "conversation.json"), "utf8")) as {
  opening: string; messages: string[]; questions: string[];
};
const project = await paced("createProject", () => createProject(TestServer.at(baseUrl), { title: "临水茶馆",
  resetChallengeBudget: false }));
note(`project ${project.projectId}`);
await paced("updateProjectAssistance", () => project.send("updateProjectAssistance", { request: {
  command_schema: "storyos.command.update-project-assistance.request.v1",
  update_project_assistance_input: { availability: "available", expected_assistance_revision: "0", ...project.binding() },
} }));
const volume = await paced("createVolume", async () => project.send("createVolume",
  createVolumeInput(project, "第一卷", await project.treeRevision())));
if (volume.response.effect.kind !== "authoritative_applied") throw new Error(`STOP: ${volume.body}`);
const volumeId = volume.response.effect.volume_id;
const chapterIds: string[] = [];
const files = Array.from({ length: 10 }, (_, index) => `chapter-${String(index + 1).padStart(2, "0")}.md`);
for (const file of files) {
  const title = readFileSync(join(fixtures, file), "utf8").split("\n")[0]?.replace(/^# /, "") ?? file;
  const chapter = await paced(`createChapter ${title}`, async () => project.send("createChapter",
    createChapterInput(project, volumeId, title, await project.treeRevision())));
  if (chapter.response.effect.kind !== "authoritative_applied") throw new Error(`STOP: ${chapter.body}`);
  chapterIds.push(chapter.response.effect.chapter_id);
}
const opened = await paced("createEditorSession", () => project.openEditorSession());
const story: Story = Object.assign(project, { chapterIds, editorSessionId: opened.editor_session.editor_session_id,
  intent: 0, current: opened.base_snapshot.chapter_id });
for (const [index, file] of files.entries()) await writeChapter(story, index, file);

// Step 1: one Run of the author in Conversation A, its capture first (statement C), then the Chapters in order.
const firstRun = await authorRun(story, script.opening);
const conversationA = await sql(`SELECT conversation_id FROM storyos.agent_runs WHERE run_id = '${firstRun}'::uuid;`);
const conversationSource: Source = { kind: "conversation_capture", id: conversationA };
await captureThenPass(story.projectId, "conversation A, step 1", conversationSource, 0);
for (const [index, chapterId] of chapterIds.entries()) {
  await captureThenPass(story.projectId, `chapter ${index + 1}`, { kind: "chapter_capture", id: chapterId }, 0);
}
// Step 3: the three messages in Conversation A, then one capture and one pass.
for (const message of script.messages) await authorRun(story, message, conversationA);
await captureThenPass(story.projectId, "conversation A, step 3", conversationSource,
  await captures(story.projectId, conversationSource, true));
// Step 4: the two changed Chapters.
for (const [index, file] of [[2, "chapter-03-changed.md"], [8, "chapter-09-changed.md"]] as const) {
  const source: Source = { kind: "chapter_capture", id: chapterIds[index] ?? "" };
  const before = await captures(story.projectId, source, false);
  await writeChapter(story, index, file);
  await captureThenPass(story.projectId, `changed chapter ${index + 1}`, source, before);
}
// Step 5: the three questions in Conversation B. The Worker stops 1 minute after the last answer.
const firstQuestion = await authorRun(story, script.questions[0] ?? "");
const conversationB = await sql(`SELECT conversation_id FROM storyos.agent_runs WHERE run_id = '${firstQuestion}'::uuid;`);
for (const question of script.questions.slice(1)) await authorRun(story, question, conversationB);
await sleep(60_000);
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
await measures(story.projectId);
note("measures written");

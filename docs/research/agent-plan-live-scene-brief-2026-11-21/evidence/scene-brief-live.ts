// The live run of #1765, from the run scripts of #1763 and #1764. It attaches to the packaged Server at its address,
// makes two Projects with the same two Chapters through the public commands of the Web client, and sends three
// author messages: in Project 1 the request for the next scene with the selected Skill scene-brief, then the answer
// of the author in the same Conversation; in Project 2 the same request with no selected Skill. It reads each measure
// with read-only SQL and writes no row with SQL. The planned numbers are 4 requests and 40,000 reported input tokens
// for each Run, and 12 requests and 120,000 reported input tokens in total: a count above one of them stops the run.
//
// STORYOS_LIVE_ROOT=<worktree> STORYOS_TEST_POSTGRES_CONTAINER=<container> \
//   node scene-brief-live.ts <Server URL> <Worker PID> <output directory>
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
assert.ok(baseUrl && workerPid && outDir, "usage: scene-brief-live.ts <Server URL> <Worker PID> <output directory>");
const fixtures = new URL(".", import.meta.url).pathname;
const COMMANDS_PER_MINUTE = 8;
const LIMITS = { runRequests: 4, runInput: 40_000, requests: 12, input: 120_000, background: 0, author: 40_000 };
const RUNS_OF_PROJECT: Record<string, number> = {};
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

async function stop(reason: string): Promise<never> {
  process.kill(Number(workerPid), "SIGTERM");
  note(`STOP: ${reason}`);
  if (projectIdForMeasures) await measures(projectIdForMeasures).catch((error) => note(`measures: ${error}`));
  throw new Error(`STOP: ${reason}`);
}
let projectIdForMeasures = "";
const blocksNow: Record<string, unknown[]> = {};

// The stops of the plan, read only. The record has each check.
const checks: unknown[] = [];
async function checkLimits(projectId: string, label: string, show = true): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const [usage] = await rows<{ requests: number; input: number | null; background_max: number | null;
    author_max: number | null; failed: number; embedding: number; author_runs: number; other_runs: number;
    run_requests_max: number | null; run_input_max: number | null; all_requests: number; all_input: number | null }>(`
    SELECT count(*)::int AS requests, sum((a.payload #>> '{usage,input_tokens}')::int)::int AS input,
      max((a.payload #>> '{usage,input_tokens}')::int) FILTER (WHERE r.run_kind <> 'author') AS background_max,
      max((a.payload #>> '{usage,input_tokens}')::int) FILTER (WHERE r.run_kind = 'author') AS author_max,
      ((SELECT count(*) FROM storyos.memory_captures WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.memory_consolidation_passes WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.agent_runs WHERE ${project} AND run_kind = 'author'
            AND status IN ('refused', 'cancelled')))::int AS failed,
      (SELECT count(*) FROM storyos.embedding_attempts WHERE ${project})::int AS embedding,
      (SELECT count(*) FROM storyos.agent_runs WHERE ${project} AND run_kind = 'author')::int AS author_runs,
      (SELECT count(*) FROM storyos.agent_runs WHERE ${project} AND run_kind <> 'author')::int AS other_runs,
      (SELECT max(n) FROM (SELECT count(*)::int AS n FROM storyos.model_attempts WHERE ${project}
         GROUP BY run_id) AS q)::int AS run_requests_max,
      (SELECT max(n) FROM (SELECT sum((payload #>> '{usage,input_tokens}')::int)::int AS n
         FROM storyos.model_attempts WHERE ${project} GROUP BY run_id) AS q)::int AS run_input_max,
      (SELECT count(*) FROM storyos.model_attempts)::int AS all_requests,
      (SELECT sum((payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts)::int AS all_input
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project}`);
  assert.ok(usage);
  const over = (usage.run_requests_max ?? 0) > LIMITS.runRequests || (usage.run_input_max ?? 0) > LIMITS.runInput
    || usage.all_requests > LIMITS.requests || (usage.all_input ?? 0) > LIMITS.input
    || (usage.background_max ?? 0) > LIMITS.background || (usage.author_max ?? 0) > LIMITS.author || usage.failed > 0
    || usage.author_runs > (RUNS_OF_PROJECT[projectId] ?? 0) || usage.other_runs > 0 || usage.embedding > 0;
  if (show) {
    checks.push({ at: new Date().toISOString(), before: label, ...usage, result: over ? "stop" : "pass" });
    writeFileSync(join(outDir, "checks.json"), `${JSON.stringify(checks, null, 2)}\n`);
    note(`check before ${label}: ${JSON.stringify(usage)}`);
  }
  if (over) await stop(`a limit of the plan before ${label}: ${JSON.stringify(usage)}`);
}

async function until(projectId: string, runId: string, label: string, check: () => Promise<boolean>,
  limitMs = 30 * 60_000) {
  const start = Date.now();
  let shown = start;
  let atPlan = 0;
  for (;;) {
    await checkLimits(projectId, label, false);
    if (await check()) break;
    const workerLog = readFileSync(join(outDir, "worker.log"), "utf8");
    if (workerLog.includes("live_guard")) await stop(`the request guard refused a request of ${label}`);
    const used = await count(`SELECT count(*) FROM storyos.model_attempts WHERE run_id = '${runId}'::uuid`);
    if (used >= LIMITS.runRequests && atPlan === 0) atPlan = Date.now();
    if (atPlan > 0 && Date.now() - atPlan > 90_000) {
      await stop(`${label} did not end 90 seconds after the planned ${LIMITS.runRequests} requests of the Run`);
    }
    if (Date.now() - start > limitMs) await stop(`no end of ${label} in ${limitMs / 60_000} minutes`);
    if (Date.now() - shown > 60_000) { note(`wait ${label}`); shown = Date.now(); }
    await sleep(5_000);
  }
  note(`done ${label}`);
}

const count = async (query: string) => Number(await sql(query));
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

async function authorRun(story: Story, label: string, text: string, skills: unknown[] = [],
  conversationId?: string): Promise<{ runId: string; conversationId: string }> {
  await checkLimits(story.projectId, label);
  RUNS_OF_PROJECT[story.projectId] = (RUNS_OF_PROJECT[story.projectId] ?? 0) + 1;
  const conversation = conversationId === undefined ? { kind: "new" as const }
    : { kind: "existing" as const, conversation_id: conversationId };
  const request = runRequest(story.current, story.key(), text, conversation);
  if (skills.length > 0) Object.assign(request.create_agent_run_input, { skill_selections: skills });
  const created = await paced(`createAgentRun ${label}`, () => story.send("createAgentRun", { request }));
  const effect = created.response.effect;
  if (effect.kind !== "admitted") await stop(`the Run of ${label} was not admitted: ${created.body}`);
  note(`run ${label} ${effect.run_id}`);
  const status = async () => (await client.getAgentRun({
    baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl, runId: effect.run_id })).status;
  await until(story.projectId, effect.run_id, `Run ${label}`, async () => ["completed", "refused", "cancelled"]
    .includes(await status()));
  return { runId: effect.run_id, conversationId: created.response.conversation_id };
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

async function measures(projectId: string, name = "measures.json"): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const out = {
    checks,
    requests: await rows(`SELECT r.run_kind, a.run_id, a.model_attempt_id, a.decision_position, a.attempt_role,
        a.dispatch_state, a.payload #>> '{response_reference}' AS response_reference,
        (a.payload #>> '{usage,input_tokens}')::int AS input_tokens,
        (a.payload #>> '{usage,output_tokens}')::int AS output_tokens, a.payload #>> '{decision,kind}' AS decision
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} ORDER BY a.run_id, a.decision_position, a.model_attempt_id`),
    runs: await rows(`SELECT r.run_id, r.run_kind, r.conversation_id, r.status, r.author_message,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = r.run_id) AS requests,
        (SELECT json_agg(json_build_object('function', t.function_name, 'arguments', t.arguments, 'state', t.state,
           'refusal', t.refusal) ORDER BY t.decision_position, t.call_position) FROM storyos.tool_calls AS t
          WHERE t.${project} AND t.run_id = r.run_id) AS tool_calls,
        (SELECT count(*) FROM storyos.proposals AS p WHERE p.${project} AND p.source_run_id = r.run_id) AS proposals
      FROM storyos.agent_runs AS r WHERE r.${project} ORDER BY r.run_id`),
    answers: await rows(`SELECT DISTINCT ON (a.run_id) a.run_id, a.payload #>> '{decision,text}' AS answer
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} AND r.run_kind = 'author' ORDER BY a.run_id, a.decision_position DESC`),
    proposals: await rows(`SELECT proposal_id, source_run_id, kind, manuscript_block_id FROM storyos.proposals
     WHERE ${project}`),
    proposal_operations: await rows(`SELECT o.proposal_id, o.operation_id, o.manuscript_block_id, o.resolution,
        o.candidate_text, o.candidate_blocks FROM storyos.proposal_operations AS o WHERE o.${project}
       ORDER BY o.proposal_id, o.operation_id`),
    blocks: blocksNow[projectId] ?? [],
    memory_entries: snapshots,
    skill_catalog: await rows(`SELECT * FROM storyos.agent_run_skill_catalog_entries WHERE ${project}`),
    embedding: await rows(`SELECT cause, settlement, count(*)::int AS attempts FROM storyos.embedding_attempts
     WHERE ${project} GROUP BY 1, 2 ORDER BY 1, 2`),
  };
  writeFileSync(join(outDir, name), `${JSON.stringify(out, null, 2)}\n`);
}

const REQUEST = "请接着这一章的结尾，写下一场：林舟坐老周的船过河，去省城找姐姐。";
const ANSWER = "思路可以。只改一处：视角换成老周。这一场从老周的眼睛写，林舟想什么，只能从他的话和动作里看出来。其余照你的思路写。";
const FILES = ["chapter-01.md", "chapter-02.md"];

// One Project with the Volume, the two Chapters, and their text. The second Chapter is the current Chapter.
async function setup(title: string): Promise<Story> {
  const project = await paced(`createProject ${title}`, () => createProject(TestServer.at(baseUrl), { title,
    resetChallengeBudget: false }));
  note(`project ${title} ${project.projectId}`);
  await paced("updateProjectAssistance", () => project.send("updateProjectAssistance", { request: {
    command_schema: "storyos.command.update-project-assistance.request.v1",
    update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
      ...project.binding() },
  } }));
  const volume = await paced("createVolume", async () => project.send("createVolume",
    createVolumeInput(project, "第一卷", await project.treeRevision())));
  if (volume.response.effect.kind !== "authoritative_applied") await stop(volume.body);
  const chapterIds: string[] = [];
  for (const file of FILES) {
    const chapterTitle = readFileSync(join(fixtures, file), "utf8").split("\n")[0]?.replace(/^# /, "") ?? file;
    const chapter = await paced(`createChapter ${chapterTitle}`, async () => project.send("createChapter",
      createChapterInput(project, volume.response.effect.volume_id, chapterTitle, await project.treeRevision())));
    if (chapter.response.effect.kind !== "authoritative_applied") await stop(chapter.body);
    chapterIds.push(chapter.response.effect.chapter_id);
  }
  const opened = await paced("createEditorSession", () => project.openEditorSession());
  const story: Story = Object.assign(project, { chapterIds, editorSessionId: opened.editor_session.editor_session_id,
    intent: 0, current: opened.base_snapshot.chapter_id });
  await writeChapter(story, 0, FILES[0] ?? "");
  await writeChapter(story, 1, FILES[1] ?? "");
  return story;
}

async function readBlocks(story: Story): Promise<void> {
  const chapter = (await client.getChapter({ baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl,
    chapterId: story.chapterIds[1] ?? "" })).chapter;
  blocksNow[story.projectId] = chapter.current_revision.blocks.map((block: { manuscript_block_id: string;
    text: string }, index: number) => ({ block: index + 1, manuscript_block_id: block.manuscript_block_id,
    text: block.text }));
}

const one = await setup("渡口 1");
const two = await setup("渡口 2");
const { skills } = await client.listSkills({ baseUrl, fetchImpl: one.fetchImpl, projectId: one.projectId });
note(`skills ${JSON.stringify(skills)}`);
const names = skills.map((skill: { reference: { name: string } }) => skill.reference.name).join(",");
if (names !== "conflict-check,local-revision,scene-brief") await stop(`the Skill directory gives ${names}`);
const sceneBrief = skills.find((skill: { reference: { name: string } }) => skill.reference.name === "scene-brief");

// Run 1: the selected Skill scene-brief with the request for the next scene.
projectIdForMeasures = one.projectId;
const first = await authorRun(one, "run 1 brief", REQUEST, [sceneBrief.reference]);
await checkLimits(one.projectId, "run 2 answer");
// Run 2: the answer of the author, in the same Conversation, with one correction of the viewpoint.
await authorRun(one, "run 2 answer", ANSWER, [], first.conversationId);
await checkLimits(one.projectId, "the end of Project 1");
// Run 3: in the second Project, a new Conversation with no selected Skill and the same request.
projectIdForMeasures = two.projectId;
await authorRun(two, "run 3 plain", REQUEST, []);
await checkLimits(two.projectId, "the end of Project 2");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
await readBlocks(one);
await readBlocks(two);
await measures(one.projectId, "measures-1.json");
await measures(two.projectId, "measures-2.json");

// The raw evidence: each wire payload, the Model Attempts, the ToolCalls, and the guard log.
const projects = `('${one.projectId}'::uuid, '${two.projectId}'::uuid)`;
const attempts = await rows<{ model_attempt_id: string }>(`SELECT model_attempt_id FROM storyos.model_attempts
  WHERE project_id IN ${projects} ORDER BY model_attempt_id`);
mkdirSync(join(outDir, "wire"), { recursive: true });
for (const [index, attempt] of attempts.entries()) {
  const text = await sql(`SELECT payload #>> '{wire,serialized_payload}' FROM storyos.model_attempts
    WHERE model_attempt_id = '${attempt.model_attempt_id}'::uuid;`);
  writeFileSync(join(outDir, "wire", `M${index + 1}.json`), `${JSON.stringify(JSON.parse(text), null, 2)}\n`);
}
const dump = async (name: string, query: string) => writeFileSync(join(outDir, name),
  `${JSON.stringify(await rows(query), null, 2)}\n`);
await dump("model-attempts.json", `SELECT a.model_attempt_id, a.project_id, a.run_id, r.run_kind,
  r.model_registration_revision, a.decision_position, a.attempt_role, a.dispatch_state,
  a.payload #>> '{response_reference}' AS response_reference, a.payload -> 'usage' AS usage
  FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
  WHERE a.project_id IN ${projects} ORDER BY a.model_attempt_id`);
await dump("tool-calls.json", `SELECT tool_call_id, project_id, run_id, decision_position, call_position,
  function_name, state, refusal, arguments FROM storyos.tool_calls WHERE project_id IN ${projects}
  ORDER BY run_id, decision_position, call_position`);
await dump("guard-log.json", `SELECT at, kind, ref, role, position FROM live_guard.log ORDER BY at`);
note("measures written");

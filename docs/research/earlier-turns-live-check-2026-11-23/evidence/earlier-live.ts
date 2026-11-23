// The live check of #1826 (ADR 0102). One Project gets Chapter 1 of `fixtures/memory-calibration/` through the
// public commands. Then three Runs of the author go into one Project Conversation:
//   Run 1 asks one short question about the Chapter.
//   Run 2 starts with `Submit the complete current request.` (the mapping `Full`), and asks the assistant to repeat
//   the first sentence of its last answer exactly.
//   Run 3 asks one more short question with no prefix.
// The script makes no SQL write. It reads each measure with read-only SQL, and it stops the Worker after Run 3, so no
// Memory work of the quiet times (30 minutes and 1 hour) starts.
//
// STORYOS_LIVE_ROOT=<checkout> node earlier-live.ts <Server URL> <Worker PID> <output directory>
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
assert.ok(baseUrl && workerPid && outDir, "usage: earlier-live.ts <Server URL> <Worker PID> <output directory>");
const fixtures = join(root, "fixtures", "memory-calibration");
// The stop limits of the plan.
const RUN_REQUESTS = 6;
const LIMITS = { requests: 20, runs: 3, request_input: 60_000 };
const HEADING = "Earlier turns of this conversation, oldest first.";
mkdirSync(outDir, { recursive: true });
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const lines: string[] = [];
function note(line: string): void {
  const stamped = `${new Date().toISOString()} ${line}`;
  lines.push(stamped);
  console.log(stamped);
  writeFileSync(join(outDir, "loader.log"), `${lines.join("\n")}\n`);
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

// The stops of the plan, read only: more than 6 requests of one Run, more than 20 requests or 3 AgentRuns in all, one
// request above 60,000 reported input tokens, a Run that is refused or cancelled, an Exa or embedding request, or
// a Run that is not a Run of the author.
async function checkLimits(label: string, show = true): Promise<void> {
  const [usage] = await rows<{ requests: number; run_max: number | null; request_max: number | null; runs: number;
    other_runs: number; failed: number; exa: number; embedding: number }>(`
    SELECT (SELECT count(*) FROM storyos.model_attempts)::int AS requests,
      (SELECT max(n) FROM (SELECT count(*) AS n FROM storyos.model_attempts GROUP BY run_id) AS c)::int AS run_max,
      (SELECT max((payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts)::int AS request_max,
      (SELECT count(*) FROM storyos.agent_runs)::int AS runs,
      (SELECT count(*) FROM storyos.agent_runs WHERE run_kind <> 'author')::int AS other_runs,
      (SELECT count(*) FROM storyos.agent_runs WHERE status IN ('refused', 'cancelled'))::int AS failed,
      (SELECT count(*) FROM storyos.tool_calls WHERE destination_attempt_id IS NOT NULL)::int AS exa,
      (SELECT count(*) FROM storyos.embedding_attempts)::int AS embedding`);
  assert.ok(usage);
  if (show) note(`check after ${label}: ${JSON.stringify(usage)}`);
  const over = usage.requests > LIMITS.requests || (usage.run_max ?? 0) > RUN_REQUESTS || usage.runs > LIMITS.runs
    || (usage.request_max ?? 0) > LIMITS.request_input || usage.other_runs > 0 || usage.failed > 0
    || usage.exa > 0 || usage.embedding > 0;
  if (over) await stop(`a limit of the plan after ${label}: ${JSON.stringify(usage)}`);
}

async function until(label: string, check: () => Promise<boolean>, limitMs = 10 * 60_000) {
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
  treeRevision: () => Promise<string>; editorSessionId: string; current: string };

// One Run of the author in the Conversation, to its settlement.
async function authorRun(story: Story, label: string, text: string, conversation: unknown): Promise<string> {
  await checkLimits(`before Run ${label}`);
  const request = runRequest(story.current, story.key(), text, conversation);
  note(`command createAgentRun ${label}`);
  const created = await story.send("createAgentRun", { request });
  const effect = created.response.effect;
  if (effect.kind !== "admitted") await stop(`the Run of ${label} was not admitted: ${created.body}`);
  note(`run ${label} ${effect.run_id}`);
  const status = async () => (await client.getAgentRun({
    baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl, runId: effect.run_id })).status;
  await until(`Run ${label}`, async () => ["completed", "refused", "cancelled"].includes(await status()));
  await checkLimits(`Run ${label}`);
  return effect.run_id;
}

async function edit(story: Story, label: string, primitives: unknown[], intent: number) {
  const ns = (0xe300 + intent).toString(16);
  const request = await authorEditRequest({ baseUrl: story.baseUrl }, { prepared: { projectId: story.projectId,
    fetchImpl: story.fetchImpl }, session: { editor_session: { editor_session_id: story.editorSessionId } } },
  primitives, { kind: "authoritative" }, String(intent), ns);
  note(`command applyAuthorEdit ${label}`);
  const edited = await story.send("applyAuthorEdit", { request });
  if (edited.response.effect.kind !== "authoritative_applied") await stop(`${label}: ${edited.body}`);
  return edited.response.effect.authoritative_revision;
}

const file = "chapter-01.md";
const source = readFileSync(join(fixtures, file), "utf8").split("\n");
const title = source[0]?.replace(/^# /, "") ?? file;
const paragraphs = source.filter((line) => line.trim().length > 0 && !line.startsWith("# "));

// The first request of each Run: whether it has `previous_response_id`, and the section of the earlier turns.
async function firstRequests(runIds: string[]) {
  const out = [];
  for (const runId of runIds) {
    const [first] = await rows<{ model_attempt_id: string; payload: string | null }>(`SELECT model_attempt_id,
        payload #>> '{wire,serialized_payload}' AS payload FROM storyos.model_attempts
       WHERE run_id = '${runId}'::uuid ORDER BY model_attempt_id LIMIT 1`);
    assert.ok(first?.payload, `the first request of ${runId}`);
    const body = JSON.parse(first.payload);
    const text = (body.input ?? []).flatMap((item: any) => item.content ?? [])
      .map((part: any) => part.text ?? "").join("\n");
    const start = text.indexOf(HEADING);
    const end = text.indexOf("Author request:");
    out.push({ run_id: runId, model_attempt_id: first.model_attempt_id,
      previous_response_id: "previous_response_id" in body, section: start >= 0,
      section_text: start >= 0 ? text.slice(start, end >= 0 ? end : undefined) : null });
  }
  return out;
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
    refusal, arguments FROM storyos.tool_calls ORDER BY run_id, decision_position, call_position`);
  await dump("guard-log.json", `SELECT at, kind, ref, role, position FROM live_guard.log ORDER BY at`);
  await dump("runs.json", `SELECT * FROM storyos.agent_runs ORDER BY run_id`);
  await dump("earlier-turns.json", `SELECT run_id, decision_position,
    payload #> '{operation_requirement,earlier_turns}' AS earlier_turns FROM storyos.context_assembly_manifests
    WHERE manifest_role = 'decision' ORDER BY run_id, decision_position`);
}

const RUN_1 = "第一章里，沈知遥为什么回到青溪镇？用一两句话回答。";
const RUN_2 = "Submit the complete current request. 请一字不差地重复你上一个回答的第一句话，只输出那一句。";
const RUN_3 = "她回到青溪站那天的天气怎么样？用一句话回答。";

const project = await createProject(TestServer.at(baseUrl), { title: "临水茶馆", resetChallengeBudget: false });
projectId = project.projectId;
note(`project ${projectId}`);
await project.send("updateProjectAssistance", { request: {
  command_schema: "storyos.command.update-project-assistance.request.v1",
  update_project_assistance_input: { availability: "available", expected_assistance_revision: "0", ...project.binding() },
} });
const volume = await project.send("createVolume", createVolumeInput(project, "第一卷", await project.treeRevision()));
if (volume.response.effect.kind !== "authoritative_applied") await stop(volume.body);
const chapter = await project.send("createChapter",
  createChapterInput(project, volume.response.effect.volume_id, title, await project.treeRevision()));
if (chapter.response.effect.kind !== "authoritative_applied") await stop(chapter.body);
const opened = await project.openEditorSession();
const story: Story = Object.assign(project, { editorSessionId: opened.editor_session.editor_session_id,
  current: opened.base_snapshot.chapter_id });
assert.equal(story.current, chapter.response.effect.chapter_id, "the new Chapter is the Current Chapter");

// Chapter 1: the whole text, then one split for each paragraph (the loader of S7-D7).
const whole = await edit(story, `${file} text`, [{ kind: "replace_selection", from: 0, to: 0,
  text: paragraphs.join("") }], 1);
let blockId = whole.blocks[0]?.manuscript_block_id ?? "";
const splits = paragraphs.slice(0, -1).map((text) => {
  const split = { kind: "split_block", manuscript_block_id: blockId, offset: text.length,
    new_manuscript_block_id: story.key() };
  blockId = split.new_manuscript_block_id;
  return split;
});
const written = await edit(story, `${file} paragraphs`, splits, 2);
assert.deepEqual(written.blocks.map((block: { text: string }) => block.text), paragraphs, `the Blocks of ${file}`);

const first = await authorRun(story, "1", RUN_1, { kind: "new" });
const conversationId = await sql(`SELECT conversation_id FROM storyos.agent_runs WHERE run_id = '${first}'::uuid;`);
const existing = { kind: "existing", conversation_id: conversationId.trim() };
const second = await authorRun(story, "2", RUN_2, existing);
const third = await authorRun(story, "3", RUN_3, existing);
await checkLimits("the end");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
const answers = await rows(`SELECT DISTINCT ON (run_id) run_id, decision_position,
    payload #>> '{decision,text}' AS answer FROM storyos.model_attempts
   WHERE attempt_role = 'decision' ORDER BY run_id, decision_position DESC`);
const measures = {
  conversation_id: existing.conversation_id,
  runs: await rows(`SELECT r.run_id, r.status, r.model_registration_revision, r.author_message,
      (SELECT DISTINCT a.payload #>> '{execution_profile,mapping_revision}' FROM storyos.model_attempts AS a
        WHERE a.run_id = r.run_id AND a.payload #>> '{execution_profile,mapping_revision}' IS NOT NULL)
        AS mapping_revision,
      (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.run_id = r.run_id)::int AS requests,
      (SELECT sum((a.payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts AS a
        WHERE a.run_id = r.run_id)::int AS input_tokens,
      (SELECT sum((a.payload #>> '{usage,output_tokens}')::int) FROM storyos.model_attempts AS a
        WHERE a.run_id = r.run_id)::int AS output_tokens,
      (SELECT min(e.recorded_at) FROM storyos.agent_run_events AS e WHERE e.run_id = r.run_id) AS started_at
    FROM storyos.agent_runs AS r ORDER BY r.run_id`),
  requests: await rows(`SELECT a.run_id, a.model_attempt_id, a.decision_position, a.attempt_role, a.dispatch_state,
      (a.payload #>> '{usage,input_tokens}')::int AS input_tokens,
      (a.payload #>> '{usage,output_tokens}')::int AS output_tokens, a.payload #>> '{decision,kind}' AS decision
    FROM storyos.model_attempts AS a ORDER BY a.model_attempt_id`),
  first_requests: await firstRequests([first, second, third]),
  answers,
};
writeFileSync(join(outDir, "measures.json"), `${JSON.stringify(measures, null, 2)}\n`);
await rawEvidence();
note("measures written");

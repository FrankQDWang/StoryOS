// The live check of the F1 path of #1818 (S11-12, ADR 0101 section 6). It is the script of #1792 with Chapters 1 to 3
// of `fixtures/memory-calibration/` and one step. One Project gets the three Chapters in Chapter order through the
// public commands. After each Chapter, the capture of that Chapter and the consolidation pass run. The intent passes
// start only by the rules of the Worker. After Chapter 3, the script waits until the background work is idle. Then one
// Run of the author in a new Conversation gives one fact of `key.md` that 顾青禾 knows, and says that a later Chapter
// uses it. The script makes the capture of that Conversation due, and waits until the work is idle, which includes the
// intent pass of 顾青禾. The script reads each measure with read-only SQL. Its only SQL writes are the
// statements A (a Chapter capture), B (the consolidation pass), and C (a Conversation capture): each one sets the due
// time of one schedule row to the present, and it must change exactly 1 row. No statement touches the intent pass row.
//
// STORYOS_LIVE_ROOT=<checkout> node fact-live.ts <Server URL> <Worker PID> <output directory>
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
assert.ok(baseUrl && workerPid && outDir, "usage: fact-live.ts <Server URL> <Worker PID> <output directory>");
const fixtures = join(root, "fixtures", "memory-calibration");
const COMMANDS_PER_MINUTE = 8;
// The plan: the requests of one Run by its kind, and the limits in all.
const RUN_REQUESTS: Record<string, number> = { author: 6, memory_capture: 8, memory_consolidation: 1, intent_pass: 2 };
const LIMITS = { requests: 45, runs: 35, input: 1_000_000, request_input: 60_000, failed_intent_passes: 2 };
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
const count = async (query: string) => Number(await sql(query));

let projectId = "";
async function stop(reason: string): Promise<never> {
  process.kill(Number(workerPid), "SIGTERM");
  note(`STOP: ${reason}`);
  if (projectId) await measures().catch((error) => note(`measures: ${error}`));
  if (projectId) await rawEvidence().catch((error) => note(`raw evidence: ${error}`));
  throw new Error(`STOP: ${reason}`);
}

// The stops of the plan, read only: the requests of one Run above the number of its kind, more than 45 requests or
// 35 AgentRuns in all, more than 1,000,000 reported input tokens in all, one request above 60,000 reported input
// tokens, an Exa or embedding request, failed work (a capture or a consolidation pass with the outcome `failed`, or a
// Run of the author that is `refused` or `cancelled`), or 2 intent passes with the outcome `failed`.
const checks: unknown[] = [];
async function checkLimits(label: string, show = true): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const [usage] = await rows<{ requests: number; input: number | null; request_max: number | null; runs: number;
    failed: number; failed_intent: number; embedding: number; exa: number }>(`
    SELECT (SELECT count(*) FROM storyos.model_attempts)::int AS requests,
      (SELECT sum((payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts)::int AS input,
      (SELECT max((payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts)::int AS request_max,
      (SELECT count(*) FROM storyos.agent_runs)::int AS runs,
      ((SELECT count(*) FROM storyos.memory_captures WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.memory_consolidation_passes WHERE ${project} AND outcome = 'failed')
        + (SELECT count(*) FROM storyos.agent_runs WHERE ${project} AND run_kind = 'author'
            AND status IN ('refused', 'cancelled')))::int AS failed,
      (SELECT count(*) FROM storyos.intent_passes WHERE ${project} AND outcome = 'failed')::int AS failed_intent,
      (SELECT count(*) FROM storyos.embedding_attempts)::int AS embedding,
      (SELECT count(*) FROM storyos.tool_calls WHERE destination_attempt_id IS NOT NULL)::int AS exa`);
  assert.ok(usage);
  const perRun = await rows<{ run_id: string; run_kind: string; requests: number; input: number | null }>(`
    SELECT a.run_id, r.run_kind, count(*)::int AS requests, sum((a.payload #>> '{usage,input_tokens}')::int)::int AS input
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     GROUP BY a.run_id, r.run_kind`);
  const overRun = perRun.find((run) => run.requests > (RUN_REQUESTS[run.run_kind] ?? 0));
  const over = overRun !== undefined || usage.requests > LIMITS.requests || usage.runs > LIMITS.runs
    || (usage.input ?? 0) > LIMITS.input || (usage.request_max ?? 0) > LIMITS.request_input || usage.failed > 0
    || usage.failed_intent >= LIMITS.failed_intent_passes || usage.embedding > 0 || usage.exa > 0;
  if (show) {
    checks.push({ at: new Date().toISOString(), after: label, ...usage, per_run: perRun, result: over ? "stop" : "pass" });
    writeFileSync(join(outDir, "checks.json"), `${JSON.stringify(checks, null, 2)}\n`);
    note(`check after ${label}: ${JSON.stringify(usage)}`);
  }
  if (over) await stop(`a limit of the plan after ${label}: ${JSON.stringify({ ...usage, over_run: overRun })}`);
}

// Each poll also checks the stops of the plan and the log of the request guard.
async function until(label: string, check: () => Promise<boolean>, limitMs = 30 * 60_000) {
  const start = Date.now();
  let shown = start;
  for (;;) {
    await checkLimits(label, false);
    if (await check()) break;
    if (readFileSync(join(outDir, "worker.log"), "utf8").includes("live_guard")) {
      await stop(`the request guard refused a request of ${label}`);
    }
    if (Date.now() - start > limitMs) await stop(`no end of ${label} in ${limitMs / 60_000} minutes`);
    if (Date.now() - shown > 60_000) { note(`wait ${label}`); shown = Date.now(); }
    await sleep(5_000);
  }
  note(`done ${label}`);
}

// The statements A (a Chapter), B (the pass), and C (a Project Conversation).
const statements: string[] = [];
type WorkKind = "chapter_capture" | "consolidation" | "conversation_capture";
async function makeDue(kind: WorkKind, sourceId: string): Promise<void> {
  const statement = `UPDATE storyos.memory_work_schedules SET due_at = now() WHERE project_id = '${projectId}'::uuid`
    + ` AND work_kind = '${kind}' AND source_id = '${sourceId}'::uuid`;
  const changed = await sql(`WITH moved AS (${statement} RETURNING 1) SELECT count(*) FROM moved;`);
  statements.push(`-- ${new Date().toISOString()}: ${changed} row\n${statement};`);
  writeFileSync(join(outDir, "due.sql"), `${statements.join("\n")}\n`);
  note(`due ${kind} ${sourceId}: ${changed} row`);
  if (changed !== "1") await stop(`the statement must change exactly 1 row: ${statement}`);
}

const settledPasses = () => count(`SELECT count(*) FROM storyos.memory_consolidation_passes
  WHERE project_id = '${projectId}'::uuid AND outcome IS NOT NULL;`);
type Source = { kind: "chapter_capture" | "conversation_capture"; id: string };
const captures = (source: Source, onlySettled: boolean) => count(`SELECT count(*) FROM storyos.memory_captures
  WHERE project_id = '${projectId}'::uuid
   AND ${source.kind === "chapter_capture" ? "chapter_id" : "conversation_id"} = '${source.id}'::uuid
   ${onlySettled ? "AND outcome IS NOT NULL" : ""};`);

// The entries after a step: each current or deleted entry, with the part of a character entry.
const snapshots: Record<string, unknown> = {};
async function snapshot(label: string): Promise<void> {
  snapshots[label] = await rows(`SELECT DISTINCT ON (memory_entry_id) entry_number, version, memory_revision, state,
      entry_type, origin, entry_type = 'character' AND origin = 'author_request' AS author_part, title, description,
      char_length(text) AS length, text, flag_reason, cause ->> 'kind' AS cause, intent, intent_questions
      FROM storyos.memory_entry_versions
     WHERE project_id = '${projectId}'::uuid ORDER BY memory_entry_id, version DESC`);
  writeFileSync(join(outDir, "entries.json"), `${JSON.stringify(snapshots, null, 2)}\n`);
}

// One capture of the source, then one pass when the pass row has a due time. When the capture started by time
// before the statement, the run counts it and sends no statement.
async function captureThenPass(label: string, source: Source, before: number): Promise<void> {
  const passes = await settledPasses();
  if (await captures(source, false) > before) note(`capture ${label} started by time: no statement`);
  else await makeDue(source.kind, source.id);
  await until(`capture ${label}`, async () => (await captures(source, true)) > before);
  await checkLimits(`capture ${label}`);
  const due = await sql(`SELECT coalesce((SELECT CASE WHEN due_at IS NULL THEN 'none' WHEN due_at > now()
      THEN 'later' ELSE 'now' END FROM storyos.memory_work_schedules WHERE project_id = '${projectId}'::uuid
      AND work_kind = 'consolidation'), 'none');`);
  const passStarted = await count(`SELECT count(*) FROM storyos.memory_consolidation_passes
    WHERE project_id = '${projectId}'::uuid;`) > passes;
  if (due === "none" && !passStarted) note(`no pass after ${label}: the pass row has no due time`);
  else {
    if (due === "later") await makeDue("consolidation", projectId);
    await until(`pass ${label}`, async () => (await settledPasses()) > passes);
    await checkLimits(`pass ${label}`);
  }
  await snapshot(label);
}

// The background work is idle when no Run is queued, claimed, waiting, or paused, no capture, consolidation pass, or
// intent pass is open, no observation waits, and the intent pass row has no due time, at two polls in a row. When an
// observation waits and the consolidation row is due later, the script sends the statement B.
async function waitIdle(label: string): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  let idlePolls = 0;
  await until(`idle ${label}`, async () => {
    const [state] = await rows<{ active: number; open: number; waiting: number; intent_due: number;
      consolidation_due: string }>(`SELECT
        (SELECT count(*) FROM storyos.agent_runs WHERE ${project}
          AND status::text IN ('queued', 'claimed', 'waiting', 'paused'))::int AS active,
        ((SELECT count(*) FROM storyos.memory_captures WHERE ${project} AND outcome IS NULL)
          + (SELECT count(*) FROM storyos.memory_consolidation_passes WHERE ${project} AND outcome IS NULL)
          + (SELECT count(*) FROM storyos.intent_passes WHERE ${project} AND outcome IS NULL))::int AS open,
        (SELECT count(*) FROM storyos.memory_observations WHERE ${project} AND state = 'waiting'
          AND kind = 'observation')::int AS waiting,
        (SELECT count(*) FROM storyos.memory_work_schedules WHERE ${project} AND work_kind = 'intent_pass'
          AND due_at IS NOT NULL)::int AS intent_due,
        coalesce((SELECT CASE WHEN due_at IS NULL THEN 'none' WHEN due_at > now() THEN 'later' ELSE 'now' END
          FROM storyos.memory_work_schedules WHERE ${project} AND work_kind = 'consolidation'), 'none')
          AS consolidation_due`);
    assert.ok(state);
    if (state.waiting > 0 && state.open === 0 && state.consolidation_due === "later") {
      await makeDue("consolidation", projectId);
    }
    const idle = state.active === 0 && state.open === 0 && state.waiting === 0 && state.intent_due === 0;
    idlePolls = idle ? idlePolls + 1 : 0;
    return idlePolls >= 2;
  }, 45 * 60_000);
  await checkLimits(`idle ${label}`);
  await snapshot(`idle ${label}`);
}

const intentPasses = () => count(`SELECT count(*) FROM storyos.intent_passes WHERE project_id = '${projectId}'::uuid;`);

type Story = { projectId: string; baseUrl: string; fetchImpl: typeof fetch; key: () => string;
  binding: () => Record<string, unknown>; send: (command: string, body: unknown) => Promise<any>;
  treeRevision: () => Promise<string>; chapterIds: string[]; editorSessionId: string; intent: number;
  current: string };

// One Run of the author in a new Conversation, to its settlement.
async function authorRun(story: Story, label: string, text: string): Promise<{ runId: string; conversationId: string }> {
  await checkLimits(`before Run ${label}`);
  const request = runRequest(story.current, story.key(), text, { kind: "new" as const });
  const created = await paced(`createAgentRun ${label}`, () => story.send("createAgentRun", { request }));
  const effect = created.response.effect;
  if (effect.kind !== "admitted") await stop(`the Run of ${label} was not admitted: ${created.body}`);
  note(`run ${label} ${effect.run_id}`);
  const status = async () => (await client.getAgentRun({
    baseUrl: story.baseUrl, projectId: story.projectId, fetchImpl: story.fetchImpl, runId: effect.run_id })).status;
  await until(`Run ${label}`, async () => ["completed", "refused", "cancelled"].includes(await status()));
  await checkLimits(`Run ${label}`);
  const conversationId = await sql(`SELECT conversation_id FROM storyos.agent_runs WHERE run_id = '${effect.run_id}'::uuid;`);
  return { runId: effect.run_id, conversationId };
}

// One author edit of the Current Chapter of the editor session.
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

const paragraphs = (file: string) => readFileSync(join(fixtures, file), "utf8").split("\n")
  .filter((line) => line.trim().length > 0 && !line.startsWith("# "));
const titleOf = (file: string) => readFileSync(join(fixtures, file), "utf8").split("\n")[0]?.replace(/^# /, "") ?? file;

// Makes the empty Chapter the Current Chapter, and then writes the paragraphs of the file: the whole text, then one
// split for each paragraph (the loader of S7-D7).
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

// The measures of the record, read only.
async function measures(): Promise<void> {
  const project = `project_id = '${projectId}'::uuid`;
  const ended = (run: string) => `(SELECT max(e.recorded_at) FROM storyos.agent_run_events AS e
    WHERE e.${project} AND e.run_id = ${run})`;
  const out = {
    checks,
    statements,
    runs: await rows(`SELECT r.run_id, r.run_kind, r.status, r.model_registration_revision,
        (SELECT DISTINCT a.payload #>> '{execution_profile,mapping_revision}' FROM storyos.model_attempts AS a
          WHERE a.${project} AND a.run_id = r.run_id AND a.payload #>> '{execution_profile,mapping_revision}' IS NOT NULL)
          AS mapping_revision,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = r.run_id)::int AS requests,
        (SELECT sum((a.payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts AS a
          WHERE a.${project} AND a.run_id = r.run_id)::int AS input_tokens,
        (SELECT sum((a.payload #>> '{usage,output_tokens}')::int) FROM storyos.model_attempts AS a
          WHERE a.${project} AND a.run_id = r.run_id)::int AS output_tokens,
        (SELECT min(e.recorded_at) FROM storyos.agent_run_events AS e WHERE e.${project} AND e.run_id = r.run_id)
          AS started_at, ${ended("r.run_id")} AS ended_at, r.conversation_id, r.author_message,
        (SELECT json_agg(json_build_object('function', t.function_name, 'state', t.state, 'refusal', t.refusal,
           'arguments', t.arguments) ORDER BY t.decision_position, t.call_position)
           FROM storyos.tool_calls AS t WHERE t.${project} AND t.run_id = r.run_id) AS tool_calls
      FROM storyos.agent_runs AS r WHERE r.${project} ORDER BY started_at NULLS LAST, r.run_id`),
    captures: await rows(`SELECT c.capture_id, c.run_id, c.source_kind, c.chapter_id, c.conversation_id, c.started_at,
        c.outcome, c.failure_reason,
        (SELECT json_object_agg(g.k, g.n) FROM (SELECT o.kind || ':' || coalesce(o.entry_type, '-') AS k, count(*) AS n
           FROM storyos.memory_observations AS o WHERE o.${project} AND o.capture_id = c.capture_id GROUP BY 1) AS g)
          AS observations
      FROM storyos.memory_captures AS c WHERE c.${project} ORDER BY c.started_at`),
    observations: await rows(`SELECT * FROM storyos.memory_observations WHERE ${project} ORDER BY capture_id, position`),
    passes: await rows(`SELECT p.pass_id, p.run_id, p.started_at, ${ended("p.run_id")} AS ended_at, p.outcome,
        p.failure_reason, p.operations, p.observations AS input_items
      FROM storyos.memory_consolidation_passes AS p WHERE p.${project} ORDER BY p.started_at`),
    requests: await rows(`SELECT r.run_kind, a.run_id, a.model_attempt_id, a.decision_position, a.attempt_role,
        a.dispatch_state, (a.payload #>> '{usage,input_tokens}')::int AS input_tokens,
        (a.payload #>> '{usage,output_tokens}')::int AS output_tokens, a.payload #>> '{decision,kind}' AS decision
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} ORDER BY a.model_attempt_id`),
    answers: await rows(`SELECT DISTINCT ON (a.run_id) a.run_id, a.payload #>> '{decision,text}' AS answer
      FROM storyos.model_attempts AS a JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
     WHERE a.${project} AND r.run_kind = 'author' ORDER BY a.run_id, a.decision_position DESC`),
    character_versions: await rows(`SELECT entry_number, version, memory_revision, state, origin, title, cause,
        char_length(text) AS length, text, intent, intent_questions, recorded_at FROM storyos.memory_entry_versions
      WHERE ${project} AND entry_type = 'character' ORDER BY entry_number, version`),
    intent_passes: await rows(`SELECT p.pass_id, p.run_id, r.status, p.started_at, ${ended("p.run_id")} AS ended_at,
        p.outcome, p.failure_reason, e.entry_number, e.title, p.entry_version, ae.entry_number AS author_entry_number,
        p.author_version, p.chapter_id, r.model_registration_revision,
        (SELECT DISTINCT a.payload #>> '{execution_profile,mapping_revision}' FROM storyos.model_attempts AS a
          WHERE a.${project} AND a.run_id = p.run_id AND a.payload #>> '{execution_profile,mapping_revision}' IS NOT NULL)
          AS mapping_revision,
        (SELECT count(*) FROM storyos.model_attempts AS a WHERE a.${project} AND a.run_id = p.run_id)::int AS requests,
        (SELECT sum((a.payload #>> '{usage,input_tokens}')::int) FROM storyos.model_attempts AS a
          WHERE a.${project} AND a.run_id = p.run_id)::int AS input_tokens,
        (SELECT sum((a.payload #>> '{usage,output_tokens}')::int) FROM storyos.model_attempts AS a
          WHERE a.${project} AND a.run_id = p.run_id)::int AS output_tokens,
        v.version AS intent_version, v.memory_revision AS intent_revision, v.intent, v.intent_questions,
        strpos(v.intent, E'\\n') > 0 AS intent_line_break
      FROM storyos.intent_passes AS p
      JOIN storyos.agent_runs AS r USING (owner_user_id, project_id, run_id)
      JOIN storyos.memory_entry_versions AS e ON (e.owner_user_id, e.project_id, e.memory_entry_id, e.version)
        = (p.owner_user_id, p.project_id, p.memory_entry_id, p.entry_version)
      LEFT JOIN storyos.memory_entry_versions AS ae ON (ae.owner_user_id, ae.project_id, ae.memory_entry_id, ae.version)
        = (p.owner_user_id, p.project_id, p.author_entry_id, p.author_version)
      LEFT JOIN storyos.memory_entry_versions AS v ON v.owner_user_id = p.owner_user_id AND v.project_id = p.project_id
        AND v.cause ->> 'pass_id' = p.pass_id::text
     WHERE p.${project} ORDER BY p.started_at`),
    schedules: await rows(`SELECT work_kind, source_id, due_at, held FROM storyos.memory_work_schedules
     WHERE ${project} ORDER BY work_kind, source_id`),
    entries: snapshots,
  };
  writeFileSync(join(outDir, "measures.json"), `${JSON.stringify(out, null, 2)}\n`);
}

// The raw evidence: each wire payload, the Model Attempts, the ToolCalls, the Run rows, and the guard log.
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
}

// The steps of the plan.
const OPENING = "你好。我在写长篇小说《临水茶馆》，后面会一章一章写进来。现在先不用做任何事，回复一句“好”就行。";
const REMEMBER = "请帮我记住一件关于顾青禾的事：一九八三年房管所卖这栋房子，总价四千二百元，顾青禾出了其中的两千一百元，"
  + "所以茶馆的房子有她一半。这一点后面会用到。";
const files = Array.from({ length: 3 }, (_, index) => `chapter-${String(index + 1).padStart(2, "0")}.md`);

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

// Step 0: one Run of the author with no fact of the sample, so that the Project has a completed Run of the author, and
// the capture of its Conversation (statement C).
const opening = await authorRun(story, "opening", OPENING);
await captureThenPass("conversation opening", { kind: "conversation_capture", id: opening.conversationId }, 0);
// Step 1: each Chapter in order, its capture, and the pass. The intent passes start by the rules of the Worker. After
// Chapter 3, the script waits until the background work is idle.
for (const [index, file] of files.entries()) {
  if (index > 0) story.chapterIds.push(await createChapter(story, volumeId, file));
  await writeChapter(story, index, file);
  await captureThenPass(`chapter ${index + 1}`, { kind: "chapter_capture", id: story.chapterIds[index] ?? "" }, 0);
  note(`intent passes after chapter ${index + 1}: ${await intentPasses()}`);
}
await waitIdle("after chapter 3");
// Step 2: one Run of the author in a new Conversation, its capture, its pass, and the idle wait.
const steps: [string, string][] = [["step 2 remember 顾青禾", REMEMBER]];
for (const [label, text] of steps) {
  const before = await intentPasses();
  const run = await authorRun(story, label, text);
  note(`intent passes during the Run of ${label}: ${(await intentPasses()) - before}`);
  await snapshot(`${label} after the Run`);
  await captureThenPass(`conversation ${label}`, { kind: "conversation_capture", id: run.conversationId }, 0);
  await waitIdle(label);
}
await checkLimits("the end");
process.kill(Number(workerPid), "SIGTERM");
note("Worker stopped");
await measures();
await rawEvidence();
note("measures written");

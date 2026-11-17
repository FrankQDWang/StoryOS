import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import { USER_A, drainDueMemoryWork, inspectRun, prepare, settleOnce } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { addChapter } from "./production-memory-settings-command.ts";
import { exchanges, send } from "./production-project-conversations-command.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
/** The Server and the Worker have the fake embedding configuration, so the hybrid search is in the Tool Exposure. */
const EMBEDDING = { STORYOS_EMBEDDING_DESTINATION: "host_fake" };
const EDITOR = '[data-manuscript-editor][contenteditable="true"]';
const SAVED = '[data-save-state="saved"][data-unsettled-intent-count="0"]';
const SEND_ENABLED_TIMEOUT_MS = 30_000;
const INSTRUCTION = "每次回答都用中文，语气克制。";
const BELL = "铜铃是师父留下的，林舟从不离身。";
const CONFLICT_REASON = "第一章写林舟把铜铃丢进了河里。";
const CHANGED_BELL = "铜铃已经丢进了河里。";
const QUESTION = "Which wording should stay in this sentence?";

/** A `create` operation of a consolidation pass for the observations `observations` of its input. */
function create(type: string, title: string, observations: number[]) {
  return { action: "create", entry: null, entries: null, type, title, description: "述。", text: `${title}。`,
    observations, reason: null };
}

/** The capture script line that records `observations`. A fact item gets the null entry and the null reason. */
function capture(observations: Record<string, unknown>[]): string {
  const items = observations.map((item) => ({ kind: "observation", entry: null, reason: null, ...item }));
  return `SCRIPT:capture record_memory_observations ${JSON.stringify({ observations: items })}`;
}

/** A fact whose text holds the script of one `create` operation of the pass. */
function scripted(type: string, title: string, observation: number) {
  return { type, title, text: `${title}。\nSCRIPT:operation ${JSON.stringify(create(type, title, [observation]))}` };
}

/** Runs `storyos-worker --once` with the fake embedding configuration until `done` holds, at most 32 times. */
async function workUntil(what: string, done: () => Promise<boolean>) {
  for (let pass = 0; pass < 32; pass += 1) {
    if (await done()) return;
    await settleOnce(EMBEDDING);
  }
  assert.ok(await done(), `${what} did not settle`);
}

/** One value of the test administrator connection as text. */
const value = (query: string) => queryStoryOSPostgres(query);

/** The content of each Context item of `sourceClass` that the first decision request of the Run sent. */
async function sent(runId: string, sourceClass: string): Promise<string[]> {
  return JSON.parse(await value(`
    SELECT jsonb_path_query_array(wire, '$.context[*] ? (@.source_class == "${sourceClass}").content')::text
      FROM storyos.model_attempts,
           LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
     WHERE run_id = '${runId}'::uuid AND attempt_role = 'decision' AND decision_position = 0`)) as string[];
}

/**
 * ADR 0094, ADR 0095, and the author journey S7-JRN-001: one Project does each step in order, in the production
 * client, with the fake destination. The journey writes no product row directly. Each SQL statement of the test
 * administrator connection makes a row of the background work due, ends the quiet time of the index work, moves the
 * start time of the last Chapter capture back by the rate interval, resets the Command Challenge rate windows of the
 * Project, or parks the rows of the Project at the end. The store tests of S7-D5 prove the Chapter rate, so the
 * journey has no wait for a time of the product.
 */
export async function verifyProductionStage7Journey(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A }, extraEnv: EMBEDDING,
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server") });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    // The Worker claims the work of each Project in the order of the due times, so the work of earlier files goes first.
    await drainDueMemoryWork(EMBEDDING);
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000005e7a00", "Stage 7 journey", "5e7a1");
    const project = setup.projectId;
    const chapterB = await addChapter(server.baseUrl, setup, "5e7a1");
    const workspace = `${server.baseUrl}/projects/${project}`;
    const status = (runId: string) => value(`SELECT status FROM storyos.agent_runs WHERE run_id = '${runId}'::uuid`);
    const message = async (text: string) => {
      // The button is enabled only when the field has text and the panel has read the earlier Run.
      await page.locator('textarea[name="assistant-message"]').fill(text);
      await expect(page.locator(".composer .send-button")).toBeEnabled({ timeout: SEND_ENABLED_TIMEOUT_MS });
      const run = await send(page, text);
      await workUntil(`the Run ${run.runId}`, async () => await status(run.runId) === "completed");
      await page.locator(`[data-assistant-run-id="${run.runId}"][data-assistant-dispatch="completed"]`).waitFor();
      return run;
    };
    // The panel has the identity of its current Run, and the section of the current turn has its results.
    const turn = (runId: string) => page.locator(`[data-assistant-run-id="${runId}"]`);
    const current = page.locator("[data-assistant-current-run]");
    const typeInChapter = async (chapterId: string, lines: string[]) => {
      const button = page.locator(`button[data-chapter-id="${chapterId}"]`);
      // The editor shows the Chapter before the caret goes into its text.
      await button.click();
      await expect(button).toHaveAttribute("aria-current", "true");
      await expect(page.locator(".editor-panel h2")).toHaveText(await button.innerText());
      await page.locator(SAVED).waitFor();
      // The caret can be at the start of the text after the Chapter opens. An Enter before and after each line puts
      // each line in its own paragraph at either place.
      await page.locator(EDITOR).click();
      for (const line of lines) {
        await page.keyboard.press("Enter");
        await page.keyboard.insertText(line);
      }
      await page.keyboard.press("Enter");
      await page.locator(SAVED).waitFor();
    };
    // The commit of the last line also moves the due time of the row of the Chapter, so a due statement waits for it.
    // The marker is ASCII text of the capture line.
    const committed = (chapterId: string, marker: string) => expect.poll(() => value(`
      SELECT convert_from(payload.canonical_bytes, 'UTF8')
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
             (head.owner_user_id, head.project_id, head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
       WHERE head.project_id = '${project}'::uuid AND head.manuscript_object_id = '${chapterId}'::uuid`),
    { timeout: 10_000 }).toContain(marker);
    const settled = (table: string, where: string) => value(`SELECT count(*)::text FROM storyos.${table}
      WHERE project_id = '${project}'::uuid AND outcome IS NOT NULL AND ${where}`);
    // ADR 0038 permits 10 Challenges of the shared class in each minute for each Project. The journey asks for 16 in
    // less than one minute: 8 in steps 1 to 4 and 8 in steps 5 to 7. So it resets the windows at the start of step 5.
    const resetChallenges = () => value(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
      WHERE project_id = '${project}'::uuid`);
    const openSettings = async () => {
      await page.goto(`${workspace}/settings`);
      await page.locator("[data-memory-section] [data-memory-group], [data-memory-empty]").first().waitFor();
    };

    // 1. The author types in Chapter A, saves a Project Instruction with the gear, and sends a message. The Run binds
    // Revision 1, and its request has the text.
    await page.goto(workspace);
    await typeInChapter(setup.chapterId, ["雨下了一整夜，渡口没有船。"]);
    await page.locator('.tree-footer a[aria-label="项目设置"]').click();
    await page.waitForURL(`${workspace}/settings`);
    await page.locator("[data-project-instruction]").fill(INSTRUCTION);
    await page.locator("[data-project-instruction-save]").click();
    await expect(page.locator("[data-project-instruction-outcome]")).toHaveText("已保存。新的指令从下一次请求开始生效。");
    await page.locator("[data-return-to-writing]").click();
    await page.waitForURL(workspace);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const first = await message("开头这一句怎么样？");
    const firstRun = await inspectRun(server.baseUrl, setup, first.runId);
    assert.equal(firstRun.context.considered.find((item) => item.source_class === "instruction_binding")?.source_version,
      await value(`SELECT instruction_revision_id::text FROM storyos.project_instruction_revisions
        WHERE project_id = '${project}'::uuid AND revision = 1`));
    assert.deepEqual(await sent(first.runId, "instruction_binding"), [INSTRUCTION]);

    // 2. The author asks the assistant to remember a foreshadowing. The panel shows the line of the write, and the
    // Memory section shows the entry with the origin of a request of the author.
    const remember = await message(`记住这个伏笔。\nSCRIPT:write_memory create ${JSON.stringify({ type: "foreshadowing",
      title: "铜铃", description: "师父留下的铜铃。", text: BELL })}`);
    await expect(current.locator(".assistant-result").first()).toHaveText("已记住：铜铃");
    await openSettings();
    const bell = page.locator('[data-memory-group="foreshadowing"] [data-memory-entry="1"]');
    await expect(bell.locator('a[data-memory-entry-origin="conversation"]')).toHaveText("你让助手记住的");
    assert.equal(await bell.locator(".memory-entry-text").textContent(), BELL);

    // 3. A message of Conversation A and a line of Chapter B hold the capture scripts. Statement 1 makes the two rows
    // due, the Conversation first. Each capture records one fact, and the pass creates one entry for each fact.
    await page.goto(workspace);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const talk = await message(`整理一下这段对话。\n${capture([scripted("relationship", "林舟与阿禾", 1)])}`);
    await typeInChapter(chapterB, ["阿禾在渡口卖鱼，灯笼挂在船头。", capture([scripted("major_event", "渡口失火", 2)])]);
    await committed(chapterB, "major_event");
    const turns = (await exchanges(page)).length;
    await value(`UPDATE storyos.memory_work_schedules
        SET due_at = clock_timestamp() - CASE work_kind WHEN 'conversation_capture' THEN interval '2 minutes'
                                                       ELSE interval '1 minute' END
      WHERE project_id = '${project}'::uuid AND source_id IN ('${first.conversationId}'::uuid, '${chapterB}'::uuid)`);
    await workUntil("the captures and the pass of step 3", async () =>
      await settled("memory_consolidation_passes", "true") === "1");
    assert.equal(await value(`SELECT string_agg(source_kind || ' ' || outcome, ', ' ORDER BY capture_id)
      FROM storyos.memory_captures WHERE project_id = '${project}'::uuid`),
    "conversation observations_recorded, chapter observations_recorded");
    assert.equal(await value(`SELECT outcome FROM storyos.memory_consolidation_passes
      WHERE project_id = '${project}'::uuid`), "applied");
    await openSettings();
    const pair = page.locator('[data-memory-group="relationship"] [data-memory-entry="2"]');
    const fire = page.locator('[data-memory-group="major_event"] [data-memory-entry="3"]');
    await expect(pair.locator('a[data-memory-entry-origin="conversation"]')).toHaveText("来自对话");
    await expect(fire.locator('a[data-memory-entry-origin="chapter"]')).toHaveText("来自章节《Chapter B》");
    // The link of the Conversation origin opens Conversation A in the panel. The panel shows no turn for the work.
    await pair.locator('a[data-memory-entry-origin="conversation"]').click();
    await page.locator(`[data-assistant-run-id="${talk.runId}"][data-assistant-dispatch="completed"]`).waitFor();
    assert.equal((await exchanges(page)).length, turns);
    // The link of the Chapter origin opens Chapter B.
    await openSettings();
    await fire.locator('a[data-memory-entry-origin="chapter"]').click();
    await expect(page.locator(".editor-panel h2")).toHaveText("Chapter B");

    // 4. The author starts Conversation B, and then opens Conversation A again from the list and continues it.
    await page.locator(SAVED).waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator("[data-assistant-new-conversation]").click();
    const other = await message("第二段对话：阿禾的语气要不要更轻？");
    assert.notEqual(other.conversationId, first.conversationId);
    await page.locator("[data-assistant-conversation-history]").click();
    await page.locator(`[data-assistant-conversation-entry="${first.conversationId}"]`).click();
    await turn(talk.runId).waitFor();

    await resetChallenges();
    // 5. Statement 2 resets the Challenge rate windows. Run 1 of the step reads entry 2. Its request has the Chapter
    // list and the Memory Index, and the panel shows no line for the read. Run 2 searches Memory, and the panel shows
    // no line for the search. Statement 3 ends the quiet time of the index work. Run 3 searches with a keyword, reads
    // Chapter B, and searches by meaning, and the panel shows one line for each use.
    const read = await message("回到第一段对话。\nSCRIPT:read_memory_entry 2");
    assert.equal(read.request.create_agent_run_input.conversation.kind, "existing");
    assert.equal(read.conversationId, first.conversationId);
    assert.deepEqual([(await sent(read.runId, "chapter_list")).length, (await sent(read.runId, "memory_index")).length],
      [1, 1]);
    await expect(current.locator("[data-assistant-manuscript-uses]")).toHaveCount(0);
    const recalled = await message("铜铃是谁留下的？\nSCRIPT:search_memory 铜铃");
    assert.equal(await value(`SELECT string_agg(function_name || ' ' || state, ', ' ORDER BY call_id)
      FROM storyos.tool_calls WHERE run_id = '${recalled.runId}'::uuid AND function_name <> 'finish'`),
    "search_memory settled");
    await expect(current.locator("[data-assistant-manuscript-uses]")).toHaveCount(0);
    await value(`UPDATE storyos.passage_index_work SET not_before = clock_timestamp() - interval '1 second'
      WHERE project_id = '${project}'::uuid`);
    await workUntil("the index work", async () => await value(`SELECT (count(*) > 0
        AND bool_and(mark_generation = completed_generation))::text
      FROM storyos.passage_index_work WHERE project_id = '${project}'::uuid`) === "true");
    const searched = await message("SCRIPT:search_manuscript 阿禾\nSCRIPT:read_chapter 2\n"
      + "SCRIPT:hybrid_search_manuscript 谁在渡口卖鱼？");
    await expect(current.locator("[data-assistant-manuscript-uses] li")).toHaveText([
      "在全书中搜索了「阿禾」", "查阅了章节「Chapter B」", "按意思在全书中搜索了「谁在渡口卖鱼？」"]);
    assert.equal(await value(`SELECT string_agg(function_name || ' ' || state, ', ' ORDER BY call_id)
      FROM storyos.tool_calls WHERE run_id = '${searched.runId}'::uuid AND function_name <> 'finish'`),
    "hybrid_search_manuscript settled, read_chapter settled, search_manuscript settled");
    // The keyword search and the hybrid search each find the passage of Chapter B.
    assert.equal(await value(`SELECT bool_and(result_text LIKE '%阿禾在渡口卖鱼%')::text FROM storyos.tool_calls
      WHERE run_id = '${searched.runId}'::uuid AND function_name LIKE '%search_manuscript'`), "true");

    // 6. The Chapter rate lets the next Chapter capture start 2 minutes after the last start. Statement 4 moves that
    // start back by the rate interval, and the time stays before its settlement. The author writes a conflict in
    // Chapter A. Statement 5 makes its row due, and statement 6 makes the pass due before its rate. Chapter A has no
    // script line before it: the fake model calls the script lines of a capture input in order.
    assert.equal(await value(`WITH moved AS (UPDATE storyos.memory_captures SET started_at = started_at - interval '2 minutes'
      WHERE capture_id = (SELECT capture_id FROM storyos.memory_captures WHERE project_id = '${project}'::uuid
                            AND source_kind = 'chapter' ORDER BY started_at DESC LIMIT 1)
      RETURNING 1) SELECT count(*)::text FROM moved`), "1");
    await typeInChapter(setup.chapterId, ["林舟把铜铃丢进了河里。", capture([{ kind: "conflict", entry: 1,
      reason: CONFLICT_REASON, type: null, title: null, text: null }])]);
    await committed(setup.chapterId, "conflict");
    await value(`UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp()
      WHERE project_id = '${project}'::uuid AND work_kind = 'chapter_capture' AND source_id = '${setup.chapterId}'::uuid`);
    await workUntil("the conflict capture", async () => await settled("memory_captures", "source_kind = 'chapter'") === "2");
    await value(`UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp()
      WHERE project_id = '${project}'::uuid AND work_kind = 'consolidation'`);
    await workUntil("the pass of step 6", async () => await settled("memory_consolidation_passes", "true") === "2");
    await openSettings();
    const flag = bell.locator('[data-memory-entry-flag="flagged"]');
    await expect(flag.locator("[data-memory-flag-reason]")).toHaveText(CONFLICT_REASON);
    await expect(flag.locator('a[data-memory-flag-source="chapter"]')).toHaveText("来自章节《Chapter A》");
    assert.equal(await bell.locator(".memory-entry-text").textContent(), BELL);
    // The next Run has the flag in its Memory Index, reads the entry, and asks the author. It writes no entry.
    await page.goto(workspace);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const versions = () => value(`SELECT string_agg(version || ' ' || (cause ->> 'kind'), ', ' ORDER BY version)
      FROM storyos.memory_entry_versions WHERE project_id = '${project}'::uuid AND entry_number = 1`);
    const asked = await message("Which wording should I keep?\nSCRIPT:read_memory_entry 1");
    assert.match((await sent(asked.runId, "memory_index"))[0] ?? "", /\{"conflict_flag":true,"description":"师父留下的铜铃。","id":1,/u);
    await expect(current.locator(".assistant-result")).toHaveText([QUESTION]);
    assert.equal(await versions(), "1 author_request, 2 conflict_flag");
    // The answer of the author changes the entry, and the change removes the flag.
    const answered = await message(`以章节为准。\nSCRIPT:write_memory change ${JSON.stringify({ entry: 1,
      text: CHANGED_BELL })}`);
    await expect(current.locator(".assistant-result").first()).toHaveText("已记住：铜铃");
    assert.equal(await versions(), "1 author_request, 2 conflict_flag, 3 author_request");
    await openSettings();
    await expect(bell.locator('[data-memory-entry-flag="flagged"]')).toHaveCount(0);
    assert.equal(await bell.locator(".memory-entry-text").textContent(), CHANGED_BELL);

    // 7. The author deletes entry 2. A later capture asks for it again, statement 7 makes the Conversation row due,
    // and statement 8 makes the pass due. The pass refuses the create, and entry 2 stays deleted.
    await pair.locator("[data-memory-delete]").click();
    await pair.locator("[data-memory-delete-confirm]").click();
    await expect(page.locator('[data-memory-group="relationship"]')).toHaveCount(0);
    await page.goto(workspace);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await message(`再整理一下。\n${capture([scripted("relationship", "林舟与阿禾", 1)])}`);
    await value(`UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp()
      WHERE project_id = '${project}'::uuid AND source_id = '${first.conversationId}'::uuid`);
    await workUntil("the capture of step 7", async () =>
      await settled("memory_captures", "source_kind = 'conversation'") === "2");
    await value(`UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp()
      WHERE project_id = '${project}'::uuid AND work_kind = 'consolidation'`);
    await workUntil("the pass of step 7", async () => await settled("memory_consolidation_passes", "true") === "3");
    assert.equal(await value(`SELECT jsonb_agg(jsonb_build_array(item ->> 'result', item ->> 'refusal'))::text
      FROM (SELECT operations FROM storyos.memory_consolidation_passes WHERE project_id = '${project}'::uuid
             ORDER BY started_at DESC LIMIT 1) AS pass, jsonb_array_elements(pass.operations) AS item`),
    '[["refused", "deleted_by_author"]]');
    await openSettings();
    await expect(page.locator('[data-memory-group="relationship"]')).toHaveCount(0);
    assert.deepEqual(errors, []);

    // Statement 9 parks each row of the background work of the Project, so that no later file starts its work.
    await value(`UPDATE storyos.memory_work_schedules SET due_at = NULL WHERE project_id = '${project}'::uuid`);
    assert.equal(await value(`SELECT ((SELECT count(*) FROM storyos.memory_work_schedules
        WHERE project_id = '${project}'::uuid AND due_at IS NOT NULL)
      + (SELECT count(*) FROM storyos.agent_runs WHERE project_id = '${project}'::uuid
          AND status IN ('queued', 'claimed', 'waiting', 'paused')))::text`), "0");
    // The editor can send the typed lines in one or more Author Edits, so the count leaves out that kind.
    await assertProjectReceipts([project], { projects: 1, kinds: "listed", receipts: { createProject: 1,
      updateProjectAssistance: 1, createVolume: 1, createChapter: 2, updateProjectInstruction: 1, createAgentRun: 10,
      deleteMemoryEntry: 1 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import { USER_A, drainDueMemoryWork, prepare, settleOnce } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { addChapter } from "./production-memory-settings-command.ts";
import { send } from "./production-project-conversations-command.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const SERVER = join(repositoryRoot, "target/release-package/storyos-server");
const EDITOR = '[data-manuscript-editor][contenteditable="true"]';
const SAVED = '[data-save-state="saved"][data-unsettled-intent-count="0"]';
const SEND_ENABLED_TIMEOUT_MS = 30_000;
const NAME = "林舟";
// The arguments of the one `record_character_intent` call of each pass. The fake model reads the script line of the
// Character File, and the part of the author comes first in the file.
const FIRST = { intent: "他想等雨停了，再撑船过河。", questions: ["他和阿禾是什么关系？", "他为什么不肯下水？"] };
const SECOND = { intent: "他想先去河对岸找阿禾，问清那场火。", questions: ["那场火是谁放的？"] };
const MANUSCRIPT = `林舟是渡口的船夫，话不多。\nSCRIPT:intent ${JSON.stringify(FIRST)}`;
const AUTHOR = `林舟左手有一道旧疤，是那年失火时留下的。\nSCRIPT:intent ${JSON.stringify(SECOND)}`;
const EMPTY_LINE = "写作助手当前不可用。你仍可以直接写作。";
// The text answer of the fake model.
const ADVICE = "This passage is inspectable Host-fake advice. It is not Authoritative State.";

/** The capture script line of Chapter B: one fact whose text holds the `create` operation of the consolidation pass. */
const CAPTURE = `SCRIPT:capture record_memory_observations ${JSON.stringify({ observations: [{ kind: "observation",
  entry: null, reason: null, type: "character", title: NAME, text: `${NAME}。\nSCRIPT:operation ${JSON.stringify({
    action: "create", entry: null, entries: null, type: "character", title: NAME, description: "渡口的船夫。",
    text: MANUSCRIPT, observations: [1], reason: null })}` }] })}`;

/** One value of the test administrator connection as text. */
const value = (query: string) => queryStoryOSPostgres(query);

/** Runs `storyos-worker --once` until `done` holds, at most 32 times. */
async function workUntil(what: string, done: () => Promise<boolean>) {
  for (let pass = 0; pass < 32; pass += 1) {
    if (await done()) return;
    await settleOnce();
  }
  assert.ok(await done(), `${what} did not settle`);
}

/**
 * The writing workspace shows no intent and no question (ADR 0101 section 5). The messages of the author and the text
 * of Chapter B hold the script lines of the fake model, so the check leaves out the messages and the editor.
 */
async function assertNoIntent(page: Page) {
  await expect(page.locator("[data-character-intent], [data-character-questions]")).toHaveCount(0);
  const shown = await page.locator("body").evaluate((body) => {
    const copy = body.cloneNode(true) as HTMLElement;
    for (const typed of copy.querySelectorAll(".assistant-author-message, [data-manuscript-editor]")) typed.remove();
    return copy.textContent ?? "";
  });
  for (const text of ["接下来想做什么", "助手还不清楚的事", FIRST.intent, ...FIRST.questions, SECOND.intent,
    ...SECOND.questions]) assert.ok(!shown.includes(text), `the writing workspace shows ${text}`);
}

/**
 * ADR 0101 and the author journey S11-JRN-001 (S11-EVD-003): one Project does each step in order, in the production
 * client of the packaged Server, with the fake destination. The journey adds no product row and changes no text of a
 * row. Each SQL statement of the test administrator connection changes only a time or a count: it makes a row of the
 * background work due (`memory_work_schedules.due_at`), moves the start of the last intent pass and the due time of its
 * row back by the rate interval as the store tests of the pass do (`intent_passes.started_at` and `due_at`), resets the
 * Command Challenge rate windows of the Project (`issued_count`), or parks the rows of the Project at the end
 * (`due_at`). So the journey has no wait for a time of the product.
 */
export async function verifyProductionStage11Journey(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const first = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A }, serverBinary: SERVER });
    const stopFirst = () => stopStoryOSServer(first.server);
    started.push(stopFirst);
    let server = first;
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    // The Worker claims the work of each Project in the order of the due times, so the work of earlier files goes first.
    await drainDueMemoryWork();
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000005b1a00", "Stage 11 journey", "5b1a1");
    const project = setup.projectId;
    const chapterB = await addChapter(server.baseUrl, setup, "5b1a1");
    const workspace = () => `${server.baseUrl}/projects/${project}`;
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
    // The count of the Author Edit Receipts of the Project.
    const authorEdits = async () => Number(await value(`SELECT count(*)::text FROM storyos.domain_receipts
      WHERE project_id = '${project}'::uuid AND command_kind = 'applyAuthorEdit'`));
    // Each typed Chapter gives at least one new Author Edit.
    const typeInChapter = async (chapterId: string, lines: string[]) => {
      const before = await authorEdits();
      const button = page.locator(`button[data-chapter-id="${chapterId}"]`);
      // The editor shows the Chapter before the caret goes into its text.
      await button.click();
      await expect(button).toHaveAttribute("aria-current", "true");
      await expect(page.locator(".editor-panel h2")).toHaveText(await button.innerText());
      await page.locator(SAVED).waitFor();
      // An Enter before and after each line puts each line in its own paragraph.
      await page.locator(EDITOR).click();
      for (const line of lines) {
        await page.keyboard.press("Enter");
        await page.keyboard.insertText(line);
      }
      await page.keyboard.press("Enter");
      await page.locator(SAVED).waitFor();
      await expect.poll(authorEdits).toBeGreaterThan(before);
    };
    // The text of the current Revision of a Chapter.
    const chapterText = (chapterId: string) => value(`
      SELECT convert_from(payload.canonical_bytes, 'UTF8')
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
             (head.owner_user_id, head.project_id, head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
       WHERE head.project_id = '${project}'::uuid AND head.manuscript_object_id = '${chapterId}'::uuid`);
    const outcomes = (table: string) => value(`SELECT coalesce(string_agg(outcome, ', ' ORDER BY started_at), '')
      FROM storyos.${table} WHERE project_id = '${project}'::uuid`);
    // ADR 0038 permits 10 Challenges of the shared class in each minute for each Project, and the client does not wait
    // when the limit refuses a send or a Chapter switch (#1679). So the journey resets the windows before steps 4 and 5.
    const resetChallenges = () => value(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
      WHERE project_id = '${project}'::uuid`);
    const openSection = async (item: "memory" | "characters") => {
      await page.goto(`${workspace()}/settings`);
      await page.locator(`[data-settings-item="${item}"]`).click();
      await expect(page.locator(`[data-settings-item="${item}"]`)).toHaveAttribute("aria-current", "true");
    };
    const file = page.locator("[data-character-file]");
    const parts = file.locator(".character-part");
    const intent = file.locator("[data-character-intent]");
    const questions = file.locator("[data-character-questions]");
    // The text of the part `index` of the file, and its origin line.
    const part = async (index: number) => [await parts.nth(index).locator(".memory-entry-text").textContent(),
      await parts.nth(index).locator("a[data-memory-entry-origin]").textContent()];
    // The result lines of the current Run of the panel, and of each earlier Run of the Conversation.
    const answers = page.locator("[data-assistant-current-run] .assistant-result");
    const earlier = (runId: string) => page.locator(`[data-assistant-history-run="${runId}"] .assistant-result`);
    // After the workspace opens, the panel shows the last Run again. The checks of the workspace wait for it.
    const restored = async (runId: string, lines: string[]) => {
      await page.locator(`#writing-assistant-panel[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`)
        .waitFor();
      await expect(answers).toHaveText(lines);
    };
    const assertIntent = async (result: typeof FIRST) => {
      await expect(intent.locator("h4")).toHaveText("接下来想做什么");
      await expect(intent.locator(".character-intent-text")).toHaveText(result.intent);
      await expect(intent.locator(".character-intent-chapter")).toHaveText("写到《Chapter B》时的想法");
      await expect(questions.locator("h4")).toHaveText("助手还不清楚的事");
      await expect(questions.locator("li")).toHaveText(result.questions);
    };

    // 1. The author writes in Chapter B and completes one Run. Statement 1 makes the row of Chapter B due. The capture
    // records the fact, and the consolidation pass creates the character entry of the background work. Its settlement
    // makes the intent pass row due, and the intent pass records the intent of the script line of the entry.
    await page.goto(workspace());
    await typeInChapter(chapterB, ["林舟在渡口等了一夜，雨没有停。", CAPTURE]);
    await expect.poll(() => chapterText(chapterB), { timeout: 10_000 }).toContain("record_memory_observations");
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const firstRun = await message("开头这一句怎么样？");
    await expect(answers).toHaveText([ADVICE]);
    await value(`UPDATE storyos.memory_work_schedules SET due_at = clock_timestamp()
      WHERE project_id = '${project}'::uuid AND work_kind = 'chapter_capture' AND source_id = '${chapterB}'::uuid`);
    await workUntil("the capture, the consolidation pass, and the intent pass of step 1", async () =>
      await outcomes("intent_passes") === "recorded");
    assert.deepEqual([await outcomes("memory_captures"), await outcomes("memory_consolidation_passes")],
      ["observations_recorded", "applied"]);
    await page.reload();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await restored(firstRun.runId, [ADVICE]);
    await assertNoIntent(page);

    // 2. The navigation has its three items. The list of the section "人物" has the name, a part of the name keeps it,
    // and other characters give no name.
    await openSection("characters");
    await expect(page.locator("[data-settings-item]")).toHaveText(["项目说明", "记忆", "人物"]);
    await expect(page.locator("[data-character-section] h2")).toHaveText("人物");
    await expect(page.locator("[data-character-name]")).toHaveText([NAME]);
    await page.locator("[data-character-search]").fill("林");
    await expect(page.locator("[data-character-name]")).toHaveText([NAME]);
    await page.locator("[data-character-search]").fill("阿禾");
    await expect(page.locator("[data-character-name]")).toHaveCount(0);
    await expect(file).toHaveCount(0);
    await page.locator("[data-character-search]").fill("");

    // 3. The selected character has its file, the block of the intent with the line of the Chapter, and the block of
    // the questions. The section "记忆" has no group of the type character.
    await page.locator(`[data-character-name="${NAME}"]`).click();
    await expect(file.locator("h3")).toHaveText(NAME);
    await expect(parts).toHaveCount(1);
    await expect(file.locator(".character-part h4")).toHaveText(["档案"]);
    assert.deepEqual(await part(0), [MANUSCRIPT, "来自章节《Chapter B》"]);
    await assertIntent(FIRST);
    // The two sections show one read, and the file came from it. When each entry is a character, the section "记忆"
    // shows no entry and the sentence of #1824.
    await page.locator('[data-settings-item="memory"]').click();
    await expect(page.locator("[data-memory-section] h2")).toHaveText("记忆");
    await expect(page.locator("[data-memory-section] [data-memory-group]")).toHaveCount(0);
    await expect(page.locator("[data-memory-empty]")).toHaveText("这里还没有人物以外的记忆。");
    await expect(page.locator("[data-memory-section]")).not.toContainText(NAME);

    // 4. Statement 2 resets the Challenge rate windows. The author tells the assistant to remember a fact of the
    // character. The file shows the part of the author first and then the part of the background work, and no block
    // of the intent. Statement 3 moves the start of the pass of step 1 and the due time of the intent pass row back by
    // the rate interval, so the new pass starts at the next claim and records the intent of the script line of the
    // part of the author.
    await resetChallenges();
    await page.goto(workspace());
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const secondRun = await message(`记住林舟的这件事。\nSCRIPT:write_memory create ${JSON.stringify({ type: "character",
      title: NAME, description: "左手的旧疤。", text: AUTHOR })}`);
    await expect(answers).toHaveText([`已记住：${NAME}`, ADVICE]);
    await assertNoIntent(page);
    await openSection("characters");
    await expect(parts).toHaveCount(2);
    assert.deepEqual([await part(0), await part(1)],
      [[AUTHOR, "你让助手记住的"], [MANUSCRIPT, "来自章节《Chapter B》"]]);
    await expect(file.locator(".character-part h4")).toHaveText(["档案"]);
    await expect(intent).toHaveCount(0);
    await expect(questions).toHaveCount(0);
    assert.equal(await value(`WITH moved AS (UPDATE storyos.intent_passes SET started_at = started_at - interval '61 seconds'
        WHERE project_id = '${project}'::uuid RETURNING 1),
      due AS (UPDATE storyos.memory_work_schedules SET due_at = due_at - interval '61 seconds'
        WHERE project_id = '${project}'::uuid AND work_kind = 'intent_pass' AND due_at IS NOT NULL RETURNING 1)
      SELECT (SELECT count(*) FROM moved) || ' ' || (SELECT count(*) FROM due)`), "1 1");
    await workUntil("the intent pass of step 4", async () => await outcomes("intent_passes") === "recorded, recorded");
    await page.reload();
    await page.locator('[data-settings-item="characters"]').click();
    await assertIntent(SECOND);
    await page.goto(workspace());
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await restored(secondRun.runId, [`已记住：${NAME}`, ADVICE]);
    await expect(earlier(firstRun.runId)).toHaveText([ADVICE]);
    await assertNoIntent(page);

    // 5. Statement 4 resets the Challenge rate windows. The deployment restarts with no model destination on the same
    // database. The editor saves a line in Chapter A, and the settings page shows the file that exists.
    await resetChallenges();
    await stopFirst();
    started.splice(started.indexOf(stopFirst), 1);
    server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A }, serverBinary: SERVER,
      modelDestination: null });
    started.push(() => stopStoryOSServer(server.server));
    // The panel shows its default line also before its read, so the check also reads the response of the read.
    const assistance = page.waitForResponse((reply) => reply.request().method() === "GET"
      && new URL(reply.url()).pathname === `/api/v1/projects/${project}/assistance`);
    await page.goto(workspace());
    assert.equal((await (await assistance).json() as { assistance: { availability: string } }).assistance.availability,
      "unavailable");
    await expect(page.locator('[data-assistant-availability="unavailable"] .assistant-status').first())
      .toHaveText(EMPTY_LINE);
    // The Editor Session of the first Server still writes after the restart, with no click (ADR 0107).
    await page.locator(EDITOR).waitFor();
    await typeInChapter(setup.chapterId, ["雨停了，林舟把船推下了水。"]);
    await expect.poll(() => chapterText(setup.chapterId), { timeout: 10_000 }).toContain("林舟把船推下了水");
    await assertNoIntent(page);
    await openSection("characters");
    await expect(page.locator("[data-character-name]")).toHaveText([NAME]);
    assert.deepEqual([await part(0), await part(1)],
      [[AUTHOR, "你让助手记住的"], [MANUSCRIPT, "来自章节《Chapter B》"]]);
    await expect(file.locator(".character-part h4")).toHaveText(["档案"]);
    await assertIntent(SECOND);
    assert.deepEqual(errors, []);

    // Statement 5 parks each row of the background work of the Project, so that no later file starts its work.
    await value(`UPDATE storyos.memory_work_schedules SET due_at = NULL WHERE project_id = '${project}'::uuid`);
    assert.equal(await value(`SELECT ((SELECT count(*) FROM storyos.memory_work_schedules
        WHERE project_id = '${project}'::uuid AND due_at IS NOT NULL)
      + (SELECT count(*) FROM storyos.agent_runs WHERE project_id = '${project}'::uuid
          AND status IN ('queued', 'claimed', 'waiting', 'paused')))::text`), "0");
    // The editor can send the typed lines of one Chapter in one or more Author Edits, so the count of that kind is not
    // a fixed number: each typed Chapter added at least one. Each other kind has its exact count: the two Chapter
    // selections and the two Runs.
    const edits = await authorEdits();
    await assertProjectReceipts([project], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 2, setCurrentChapter: 2, applyAuthorEdit: edits, createAgentRun: 2 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

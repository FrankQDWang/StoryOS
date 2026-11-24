import assert from "node:assert/strict";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Locator } from "playwright";
import { expect } from "playwright/test";

import { getSubrun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, drainLeftoverWork, inspectRun, prepare, runWorker, settleRun } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { addChapter } from "./production-memory-settings-command.ts";
import { send } from "./production-project-conversations-command.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const SERVER = join(repositoryRoot, "target/release-package/storyos-server");
const EDITOR = '[data-manuscript-editor][contenteditable="true"]';
const SAVED = '[data-save-state="saved"][data-unsettled-intent-count="0"]';
// The scripts of the fake destination (ADR 0103 sections 2 and 3A), and the three step texts of the plan script.
const PLAN_SCRIPT = "SCRIPT:update_plan";
const SUBRUN_SCRIPT = "SCRIPT:start_subrun";
const STEPS = ["读第一章到第五章", "列出不一致之处", "改写第三章第一段"];
// The accessible name of the mark of each state of a plan step.
const MARKS = { completed: "已完成", in_progress: "进行中", pending: "未开始" } as const;
// A message with no script: the fake model answers it with its text answer, and with no plan and no Subrun.
const SIMPLE = "这一段的节奏怎么样？";
const ADVICE = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
const EMPTY_LINE = "写作助手当前不可用。你仍可以直接写作。";
// The Worker holds before a model request while this file exists, from the request that
// `STORYOS_TEST_FAKE_DISPATCH_HOLD_FROM` gives. Request 2 of the plan script comes after the first `update_plan` call,
// and request 2 of the Subrun script is the first request of the Subrun.
const HOLD = join(repositoryRoot, "target", "stage9-journey", "dispatch.hold");

/**
 * ADR 0103 and the author journey S9-JRN-001, in the production client of the packaged Server with the fake
 * destination. Steps 1 and 2 examine the panel while the Worker holds before a model request of the Run, so the Run
 * works at that time, and each answer comes from the Server. Step 4 restarts the deployment with no model destination
 * on the same database.
 */
export async function verifyProductionStage9Journey(context: BrowserContext): Promise<void> {
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
    // Each `settleOnce` claims the oldest queued Run, so the Runs that earlier journeys left settle first.
    await drainLeftoverWork();
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-000000943a00", "Stage 9 journey", "943a1");
    const project = setup.projectId;
    await addChapter(server.baseUrl, setup, "943a1");
    const workspace = () => `${server.baseUrl}/projects/${project}`;
    await page.goto(workspace());
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator(SAVED).waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const turn = page.locator("[data-assistant-current-run]");
    // The button is enabled only when the field has text and the panel has read the earlier Run.
    const message = async (text: string) => {
      await page.locator('textarea[name="assistant-message"]').fill(text);
      await expect(page.locator(".composer .send-button")).toBeEnabled({ timeout: 30_000 });
      return await send(page, text);
    };
    const ended = (runId: string) =>
      page.locator(`[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`).waitFor();
    // Starts one Worker pass that holds before model request `from` of the Run, and gives the end of the pass. When an
    // assertion fails, the cleanup deletes the file and waits for the end of the pass, so no held Worker stays for a
    // later journey.
    let held: Promise<unknown> | undefined;
    const holdFrom = (from: number) => {
      mkdirSync(join(HOLD, ".."), { recursive: true });
      writeFileSync(HOLD, "hold");
      const pass = runWorker(["--once"], { extraEnv: { STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: HOLD,
        STORYOS_TEST_FAKE_DISPATCH_HOLD_FROM: String(from) } });
      held = pass.catch(() => undefined);
      return async () => {
        rmSync(HOLD);
        await pass;
      };
    };
    started.push(async () => {
      if (existsSync(HOLD)) rmSync(HOLD);
      await held;
    });

    // 1. While the Run works, the plan list is below the status line, and the first step is in progress. After the
    // end, the list is above the answer, and each step is done.
    const plan = await message(PLAN_SCRIPT);
    let release = holdFrom(2);
    const list = turn.locator("[data-assistant-plan]");
    const steps = list.locator(":scope > li");
    // The author sees each step text and the mark of its state.
    const shownPlan = async (states: (keyof typeof MARKS)[]) => {
      await expect(list).toBeVisible();
      await expect(steps).toHaveText(STEPS);
      for (const [index, state] of states.entries()) {
        await expect(steps.nth(index)).toBeVisible();
        await shown(steps.nth(index).locator("span:not(.assistant-plan-mark)"), STEPS[index]!);
        await expect(steps.nth(index)).toHaveAttribute("data-plan-step-status", state);
        await expect(steps.nth(index).getByRole("img", { name: MARKS[state] })).toBeVisible();
      }
    };
    await shownPlan(["in_progress", "pending", "pending"]);
    await expect(turn.locator("[data-assistant-run-status] + [data-assistant-plan]")).toHaveCount(1);
    await expect(turn.locator("[data-assistant-run-status]")).toHaveText("正在处理");
    await release();
    const planRun = await settleRun(server.baseUrl, setup, plan.runId);
    assert.equal(planRun.status, "completed");
    const planAnswer = planRun.results?.[0];
    assert.ok(planAnswer?.kind === "answer");
    await ended(plan.runId);
    await shownPlan(["completed", "completed", "completed"]);
    await expect(turn.locator("[data-assistant-plan] + .assistant-result")).toHaveText(planAnswer.text);

    // 2. While the Subrun works, its card says that it reads, and its drawer opens from the right side with the brief
    // and the state text of a Subrun in work. The same open drawer then shows the record of the ended Subrun.
    const delegated = await message(SUBRUN_SCRIPT);
    release = holdFrom(2);
    const card = turn.locator("[data-assistant-subrun]");
    await expect(card).toHaveAttribute("data-subrun-state", "working");
    await shown(card.locator(".assistant-subrun-state"), "正在通读前文");
    await shown(card.locator(".assistant-subrun-title"), "读当前章");
    const working = (await inspectRun(server.baseUrl, setup, delegated.runId)).subruns?.[0];
    assert.ok(working !== undefined && working.state === "working");
    const brief = (await getSubrun({ baseUrl: server.baseUrl, fetchImpl: setup.fetchImpl, projectId: project,
      subrunId: working.subrun_id })).brief;
    await card.click();
    const drawer = page.getByRole("dialog", { name: "子任务记录" });
    const state = drawer.locator("[data-subrun-drawer-state]");
    const sections = drawer.locator("section");
    // The author sees the heading and each line of each section of the drawer, in order.
    const shownSections = async (rows: [string, ...string[]][]) => {
      await expect(sections).toHaveCount(rows.length);
      for (const [index, [heading, ...lines]] of rows.entries()) {
        const section = sections.nth(index);
        await expect(section).toBeVisible();
        await shown(section.getByRole("heading", { name: heading, exact: true }), heading);
        for (const text of lines) await shown(section.getByText(text, { exact: true }), text);
        await expect(section).toHaveText([heading, ...lines].join(""));
      }
    };
    await expect(drawer).toBeVisible();
    await shown(state, "进行中 · 第 1 步");
    await shownSections([["指令", brief], ["过程"]]);
    const box = await drawer.boundingBox();
    assert.ok(box !== null && Math.round(box.x + box.width) === page.viewportSize()?.width);
    await release();
    const subrunRun = await settleRun(server.baseUrl, setup, delegated.runId);
    assert.equal(subrunRun.status, "completed");
    const subrun = await getSubrun({ baseUrl: server.baseUrl, fetchImpl: setup.fetchImpl, projectId: project,
      subrunId: working.subrun_id });
    assert.ok(subrun.state === "completed" && subrun.answer !== undefined);
    await shown(state, `已完成 · ${subrun.step_count} 步`);
    await expect(drawer).toBeVisible();
    await shownSections([["指令", brief], ["过程", "读取 Chapter A"], ["结论", subrun.answer]]);
    await page.keyboard.press("Escape");
    await expect(drawer).toHaveCount(0);
    await ended(delegated.runId);
    await expect(card).toHaveAttribute("data-subrun-state", "completed");
    await shown(card.locator(".assistant-subrun-state"), "已通读前文");
    await shown(card.locator(".assistant-subrun-title"), "读当前章");
    const delegatedAnswer = subrunRun.results?.[0];
    assert.ok(delegatedAnswer?.kind === "answer" && delegatedAnswer.text.includes(subrun.answer));
    await expect(turn.locator(".assistant-subruns + .assistant-result")).toHaveText(delegatedAnswer.text);

    // 3. The fake model answers a simple request with its text answer, and with no plan and no Subrun.
    const simple = await message(SIMPLE);
    assert.equal((await settleRun(server.baseUrl, setup, simple.runId)).status, "completed");
    await ended(simple.runId);
    await expect(turn.locator(".assistant-result")).toHaveText([ADVICE]);
    await expect(turn.locator("[data-assistant-plan], [data-assistant-subrun]")).toHaveCount(0);
    assert.deepEqual(errors, []);

    // 4. The deployment restarts with no model destination on the same database. The panel says that the assistant
    // is not available, and the editor saves a line in Chapter A. ADR 0038 permits 10 Challenges of the shared class in
    // each minute for each Project, so the statement resets the Challenge rate windows of the Project before this step.
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
      WHERE project_id = '${project}'::uuid`);
    await stopFirst();
    started.splice(started.indexOf(stopFirst), 1);
    server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A }, serverBinary: SERVER,
      modelDestination: null });
    started.push(() => stopStoryOSServer(server.server));
    await page.goto(workspace());
    await expect(page.locator('[data-assistant-availability="unavailable"] .assistant-status').first())
      .toHaveText(EMPTY_LINE);
    // The editor session of the first Server still holds the writer lease, because that Server stopped before the page
    // released it. So the author takes over the writing, as after each other end of a session.
    await page.locator("[data-take-over-writer]").click();
    await page.locator(EDITOR).click();
    await page.keyboard.press("Enter");
    await page.keyboard.insertText("雨停了，林舟把船推下了水。");
    await page.locator(SAVED).waitFor();
    await expect.poll(() => chapterText(project, setup.chapterId)).toContain("林舟把船推下了水");
    assert.deepEqual(errors, []);

    assert.equal(await queryStoryOSPostgres(`SELECT count(*)::text FROM storyos.agent_runs
      WHERE project_id = '${project}'::uuid AND status IN ('queued', 'claimed', 'waiting', 'paused')`), "0");
    const edits = Number(await queryStoryOSPostgres(`SELECT count(*)::text FROM storyos.domain_receipts
      WHERE project_id = '${project}'::uuid AND command_kind = 'applyAuthorEdit'`));
    assert.ok(edits > 0);
    await assertProjectReceipts([project], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 2, createAgentRun: 3, takeOverProjectWriter: 1, applyAuthorEdit: edits } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

/** The author sees `locator`, and it has exactly `text`. */
async function shown(locator: Locator, text: string) {
  await expect(locator).toBeVisible();
  await expect(locator).toHaveText(text);
}

/** The text of the current Revision of a Chapter. */
function chapterText(projectId: string, chapterId: string) {
  return queryStoryOSPostgres(`
    SELECT convert_from(payload.canonical_bytes, 'UTF8')
      FROM storyos.authoritative_heads AS head
      JOIN storyos.authoritative_revisions AS revision
        ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
           (head.owner_user_id, head.project_id, head.current_revision_id)
      JOIN storyos.authoritative_payloads AS payload
        ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
           (revision.owner_user_id, revision.project_id, revision.payload_id)
     WHERE head.project_id = '${projectId}'::uuid AND head.manuscript_object_id = '${chapterId}'::uuid`);
}

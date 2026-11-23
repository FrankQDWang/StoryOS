import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext } from "playwright";
import { expect } from "playwright/test";

import { USER_A, drainLeftoverWork, prepare, settleRun } from "./acceptance.ts";
import { startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { send } from "./production-project-conversations-command.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
// The fake destination reads its script from the author message (ADR 0103 section 2).
const SCRIPT = "SCRIPT:update_plan";
// The three step texts of the plan script of the fake destination.
const STEPS = ["读第一章到第五章", "列出不一致之处", "改写第三章第一段"];

/**
 * ADR 0103 section 2: the author sends the plan script, and the Run records its plan with `update_plan`. After the
 * Run ends, the panel shows the three steps as done, in step order, directly above the answer.
 */
export async function verifyProductionPlanJourney(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A },
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server") });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    // Each `settleOnce` claims the oldest queued Run, so the Runs that earlier journeys left settle first.
    await drainLeftoverWork();
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-000000921a00", "Plan journey", "921a1");
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();

    const { runId } = await send(page, SCRIPT);
    const run = await settleRun(server.baseUrl, setup, runId);
    assert.equal(run.status, "completed");
    assert.deepEqual(run.plan, STEPS.map((text) => ({ text, status: "completed" })));
    const answer = run.results?.[0];
    assert.ok(answer?.kind === "answer" && answer.text.length > 0);

    // The refresh of the panel brings the plan of the ended Run. The status line is gone, and the list is directly
    // above the first result, which is the answer.
    await page.locator(`[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`).waitFor();
    const turn = page.locator("[data-assistant-current-run]");
    const steps = turn.locator("[data-assistant-plan] > li");
    await expect(steps).toHaveText(STEPS);
    assert.deepEqual(await steps.evaluateAll((items) => items.map((item) => [item.getAttribute("data-plan-step-status"),
      item.querySelector("[role=img]")?.getAttribute("aria-label")])), STEPS.map(() => ["completed", "已完成"]));
    await expect(turn.locator("[data-assistant-run-status]")).toHaveCount(0);
    await expect(turn.locator("[data-assistant-plan] + .assistant-result")).toHaveText(answer.text);
    assert.deepEqual(errors, []);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 1, createAgentRun: 1 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

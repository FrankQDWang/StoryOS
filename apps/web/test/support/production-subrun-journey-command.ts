import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext } from "playwright";
import { expect } from "playwright/test";

import { getSubrun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, drainLeftoverWork, prepare, settleRun } from "./acceptance.ts";
import { startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { send } from "./production-project-conversations-command.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
// The fake destination reads its script from the author message (ADR 0103 section 3A).
const SCRIPT = "SCRIPT:start_subrun";

/**
 * ADR 0103 section 3A: the author sends the Subrun script, and the Run starts one Subrun that reads the Current
 * Chapter. After the Run ends, the turn shows one card with `已通读前文`, and its drawer shows the brief, one `读取`
 * line, and the answer of the Subrun.
 */
export async function verifyProductionSubrunJourney(context: BrowserContext): Promise<void> {
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
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-000000924a00", "Subrun journey", "924a1");
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();

    const { runId } = await send(page, SCRIPT);
    // The Worker settles the parent, then the Subrun, then the parent again.
    const run = await settleRun(server.baseUrl, setup, runId);
    assert.equal(run.status, "completed");
    assert.equal(run.subruns?.length, 1);
    const subrun = await getSubrun({ baseUrl: server.baseUrl, fetchImpl: setup.fetchImpl, projectId: setup.projectId,
      subrunId: run.subruns[0]!.subrun_id });
    assert.ok(subrun.state === "completed" && subrun.answer !== undefined && subrun.answer.length > 0);

    // The refresh of the panel brings the card of the ended Run.
    await page.locator(`[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`).waitFor();
    const card = page.locator("[data-assistant-current-run] [data-assistant-subrun]");
    await expect(card).toHaveCount(1);
    await expect(card).toHaveAttribute("data-subrun-state", "completed");
    await expect(card.locator(".assistant-subrun-state")).toHaveText("已通读前文");
    await expect(card.locator(".assistant-subrun-title")).toHaveText(subrun.title);
    await card.click();
    const drawer = page.getByRole("dialog", { name: "子任务记录" });
    await expect(drawer.locator("section")).toHaveText([
      `指令${subrun.brief}`, "过程读取 Chapter A", `结论${subrun.answer}`]);
    await page.keyboard.press("Escape");
    await expect(drawer).toHaveCount(0);
    await expect(card).toBeFocused();
    assert.deepEqual(errors, []);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 1, createAgentRun: 1 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext } from "playwright";
import { expect } from "playwright/test";

import type { CreateAgentRunResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, drainLeftoverWork, prepare, settleRun } from "./acceptance.ts";
import { startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const TITLE = "林舟";
const TEXT = "船夫，不说话。\n夜里看灯。";
const MESSAGE = `记住林舟。\nSCRIPT:write_memory create ${JSON.stringify({ type: "character", title: TITLE,
  description: "沉默的船夫。", text: TEXT })}`;

/**
 * ADR 0094 section 9: the author asks the assistant to remember an entry, opens the settings page with the gear, and
 * finds the entry in the group of its type with its text, its origin line, and the link to its source Conversation.
 */
export async function verifyProductionMemorySettings(context: BrowserContext): Promise<void> {
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
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-000000b78a00", "Memory journey", "b78a1");
    const workspace = `${server.baseUrl}/projects/${setup.projectId}`;

    // 1. The author sends a message with the Memory write script of the fake model.
    await page.goto(workspace);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator('textarea[name="assistant-message"]').fill(MESSAGE);
    const admitted = page.waitForResponse((reply) =>
      reply.request().method() === "POST" && new URL(reply.url()).pathname.endsWith("/agent-runs"));
    await page.locator(".composer .send-button").click();
    const body = await (await admitted).json() as CreateAgentRunResponse;
    assert.ok(body.effect.kind === "admitted");
    const runId = body.effect.run_id;
    assert.equal((await settleRun(server.baseUrl, setup, runId)).status, "completed");
    await page.locator(`[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`).waitFor();

    // 2. The author opens the settings page with the gear, and finds the entry in the group `人物`.
    await page.locator('.tree-footer a[aria-label="项目设置"]').click();
    await page.waitForURL(`${workspace}/settings`);
    const group = page.locator('[data-memory-group="character"]');
    await expect(group.locator("h3")).toHaveText("人物");
    const entry = group.locator("[data-memory-entry]");
    await expect(entry).toHaveCount(1);
    await expect(entry.locator("h4")).toHaveText(TITLE);
    assert.equal(await entry.locator(".memory-entry-text").textContent(), TEXT);
    const origin = entry.locator('a[data-memory-entry-origin="conversation"]');
    await expect(origin).toHaveText("你让助手记住的");
    assert.equal(await origin.getAttribute("href"),
      `/projects/${setup.projectId}?conversation=${body.conversation_id}`);
    await expect(page.locator("[data-memory-group]")).toHaveCount(1);
    assert.deepEqual(errors, []);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1,
      updateProjectAssistance: 1, createVolume: 1, createChapter: 1, createAgentRun: 1 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

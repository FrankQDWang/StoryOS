import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import type { CreateAgentRunRequest, CreateAgentRunResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, drainLeftoverWork, prepare, settleRun } from "./acceptance.ts";
import { startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const MESSAGE_A = "第一段对话：渡口这一段的节奏怎么样？";
const MESSAGE_B = "第二段对话：林舟的语气要不要更冷？";
const MESSAGE_A2 = "回到第一段对话：再看一下结尾。";

/** Sends one message from the composer, and returns the createAgentRun request and the admitted response. */
async function send(page: Page, text: string) {
  await page.locator('textarea[name="assistant-message"]').fill(text);
  const response = page.waitForResponse((reply) =>
    reply.request().method() === "POST" && new URL(reply.url()).pathname.endsWith("/agent-runs"));
  await page.locator(".composer .send-button").click();
  const reply = await response;
  assert.equal(reply.status(), 202);
  const body = await reply.json() as CreateAgentRunResponse;
  assert.ok(body.effect.kind === "admitted");
  return { request: reply.request().postDataJSON() as CreateAgentRunRequest, runId: body.effect.run_id,
    conversationId: body.conversation_id };
}

/** Each exchange of the panel as its author messages and its result, the oldest first. */
function exchanges(page: Page) {
  return page.locator("[data-assistant-history-run], [data-assistant-current-run]").evaluateAll((sections) =>
    sections.map((section) => [...section.querySelectorAll(".assistant-author-message, .assistant-result")]
      .map((line) => line.textContent)));
}

/**
 * ADR 0094 section 10: the author opens an earlier Project Conversation in the writing assistant panel, reads it, and
 * continues it. A new browser page opens a Conversation from the workspace address.
 */
export async function verifyProductionProjectConversations(context: BrowserContext): Promise<void> {
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
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000007f1b00", "Conversation journey", "7f1b1");
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const settled = async (runId: string) => {
      assert.equal((await settleRun(server.baseUrl, setup, runId)).status, "completed");
      await page.locator(`[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`).waitFor();
    };

    // 1. The author sends a message in Conversation A, then selects `新对话` and sends a message in Conversation B.
    const a = await send(page, MESSAGE_A);
    await settled(a.runId);
    const resultA = await page.locator("[data-assistant-current-run] .assistant-result").textContent();
    assert.ok(resultA !== null && resultA.length > 0);
    await page.locator("[data-assistant-new-conversation]").click();
    const b = await send(page, MESSAGE_B);
    assert.deepEqual(b.request.create_agent_run_input.conversation, { kind: "new" });
    assert.notEqual(b.conversationId, a.conversationId);
    await settled(b.runId);
    const resultB = await page.locator("[data-assistant-current-run] .assistant-result").textContent();

    // 2. The list has two entries, and B is the first.
    await page.locator("[data-assistant-conversation-history]").click();
    const entries = page.locator("[data-assistant-conversation-entry]");
    await expect(entries).toHaveCount(2);
    assert.deepEqual(await entries.evaluateAll((buttons) => buttons.map((button) =>
      [button.getAttribute("data-assistant-conversation-entry"), button.getAttribute("aria-current")])),
    [[b.conversationId, "true"], [a.conversationId, null]]);

    // 3. The author opens A. The panel shows the message and the result of A, and not those of B.
    await page.locator(`[data-assistant-conversation-entry="${a.conversationId}"]`).click();
    await page.locator(`[data-assistant-run-id="${a.runId}"][data-assistant-dispatch="completed"]`).waitFor();
    assert.deepEqual(await exchanges(page), [[MESSAGE_A, resultA]]);

    // 4. The next message continues A, and its exchange follows the first exchange of A.
    const a2 = await send(page, MESSAGE_A2);
    assert.deepEqual(a2.request.create_agent_run_input.conversation, { kind: "existing", conversation_id: a.conversationId });
    assert.equal(a2.conversationId, a.conversationId);
    await settled(a2.runId);
    const continued = await exchanges(page);
    assert.deepEqual(continued.map((exchange) => exchange[0]), [MESSAGE_A, MESSAGE_A2]);
    assert.deepEqual(continued[0], [MESSAGE_A, resultA]);

    // 5. A new page, which has its own empty session storage, opens the address of B, and the panel shows B.
    const next = await context.newPage();
    started.push(() => next.close());
    next.on("pageerror", (error) => errors.push(error.message));
    next.setDefaultTimeout(10_000);
    await next.setViewportSize({ width: 1487, height: 1058 });
    await next.goto(`${server.baseUrl}/projects/${setup.projectId}?conversation=${b.conversationId}`);
    await next.locator(`[data-assistant-run-id="${b.runId}"][data-assistant-dispatch="completed"]`).waitFor();
    assert.deepEqual(await exchanges(next), [[MESSAGE_B, resultB]]);
    // The panel records the opened Conversation, so the address drops its parameter.
    assert.equal(new URL(next.url()).search, "");
    assert.deepEqual(errors, []);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 1, createAgentRun: 3 }, authorActions: 2 });
  } finally {
    for (const close of started.reverse()) await close();
  }
}

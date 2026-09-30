import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import type { BrowserContext } from "playwright";
import { expect } from "playwright/test";

import { getAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { id, prepare, settleOnce, USER_A } from "./acceptance";
import { startStoryOSServer, stopStoryOSServer } from "./node-integration";

export async function verifyProductionRunEvidence(context: BrowserContext): Promise<void> {
  const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
  const server = await startStoryOSServer({ repositoryRoot,
    serverBinary: join(repositoryRoot, "target/release-package/storyos-server"),
    sessions: { "session-a": USER_A } });
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.setDefaultTimeout(10_000);
  try {
    const setup = await prepare(server.baseUrl, id("f87700"), "Run evidence acceptance", "f8771");
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.locator(".composer button:not([disabled])").waitFor();
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    const saved = page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]');
    const original = "The lantern stayed lit.";
    await editor.click();
    await page.keyboard.insertText(original);
    await saved.waitFor();
    const submit = async (message: string) => {
      const responsePromise = page.waitForResponse((response) =>
        response.request().method() === "POST" && new URL(response.url()).pathname.endsWith("/agent-runs"));
      await page.locator('input[name="assistant-message"]').fill(message);
      await page.locator(".composer button").click();
      const response = await responsePromise;
      assert.equal(response.status(), 202);
      return (await response.json() as CreateAgentRunResponse).effect.run_id;
    };
    const inspect = (runId: string) => getAgentRun({ baseUrl: server.baseUrl,
      projectId: setup.projectId, runId, fetchImpl: setup.fetchImpl });
    const message = "Help with this passage.";
    const firstId = await submit(message);
    await settleOnce();
    const first = await inspect(firstId);
    assert.equal(first.status, "completed");
    assert.equal(first.context.sufficiency.kind, "complete");
    assert.deepEqual(first.context.selected.filter((input) => input.source_class !== "host_control")
      .map((input) => [input.source_class, input.content]),
    [["author_instruction", message], ["working_target", original]]);
    await expect(page.locator('[data-run-evidence]')).toHaveCount(0);
    await page.locator('[data-assistant-inspect]').click();
    const evidence = page.locator('[data-run-evidence]');
    await expect(evidence).toBeVisible();
    await expect(evidence).toContainText("输入范围");
    await expect(evidence).toContainText(message);
    await expect(evidence).toContainText(original);
    await expect(evidence).toContainText("提供方内部内容仍未知");
    assert.equal(await evidence.locator("button").count(), 0);
    assert.ok(!(await evidence.innerText()).includes(first.context.assembly_manifest_id));
    assert.ok(!(await evidence.innerText()).includes(first.run_id));
    await editor.click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.insertText(" Later edit.");
    await saved.waitFor();
    await page.locator('[data-assistant-inspect]').click();
    await expect(evidence).toContainText("章节已改变；这里保留请求当时的片段。");
    assert.ok(!(await evidence.innerText()).includes("Later edit."));
    await page.reload();
    await expect(evidence).toHaveAttribute("data-run-evidence-run-id", firstId);
    await expect(evidence).toContainText(original);
    const secondId = await submit("Keep the scene calm.");
    await settleOnce();
    await page.locator('[data-assistant-inspect]').click();
    await expect(evidence).toHaveAttribute("data-run-evidence-run-id", secondId);
    let release = (): void => {};
    let reached = (): void => {};
    const released = new Promise<void>((resolve) => { release = resolve; });
    const held = new Promise<void>((resolve) => { reached = resolve; });
    const oldQuery = `**/agent-runs/${firstId}`;
    await page.route(oldQuery, async (route) => {
      const response = await route.fetch();
      reached();
      await released;
      await route.fulfill({ response });
    });
    const history = page.locator(`[data-assistant-history-inspect="${firstId}"]`);
    try {
      await history.click();
      await held;
      await expect(evidence).toHaveCount(0);
      await page.locator('[data-assistant-inspect]').click();
      await expect(evidence).toHaveAttribute("data-run-evidence-run-id", secondId);
      const oldResponse = page.waitForResponse((response) => new URL(response.url()).pathname.endsWith(firstId));
      release();
      await oldResponse;
      await expect(evidence).toHaveAttribute("data-run-evidence-run-id", secondId);
    } finally { release(); await page.unroute(oldQuery); }
    await history.click();
    await expect(evidence).toHaveAttribute("data-run-evidence-run-id", firstId);
    await page.reload();
    await expect(evidence).toHaveAttribute("data-run-evidence-run-id", firstId);
    await expect(evidence).toContainText(original);
    await page.route(oldQuery, async (route) => {
      const wrongScope = await route.fetch({ url: route.request().url().replace(setup.projectId, id("ffff")) });
      assert.equal(wrongScope.status(), 404);
      await route.fulfill({ response: wrongScope });
    });
    await history.click();
    await expect(evidence).toHaveCount(0);
    await expect(page.locator('[data-assistant-memory-settings]')).toContainText("无法确认");
    await page.unroute(oldQuery);
    await history.click();
    await expect(evidence).toHaveAttribute("data-run-evidence-run-id", firstId);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
    await stopStoryOSServer(server.server);
  }
}

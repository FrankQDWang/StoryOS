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
    const message = "Help with this passage.";
    const admittedResponse = page.waitForResponse((response) =>
      response.request().method() === "POST" && new URL(response.url()).pathname.endsWith("/agent-runs"));
    await page.locator('input[name="assistant-message"]').fill(message);
    await page.locator(".composer button").click();
    const response = await admittedResponse;
    assert.equal(response.status(), 202);
    const admitted = await response.json() as CreateAgentRunResponse;
    await settleOnce();
    const run = await getAgentRun({ baseUrl: server.baseUrl, projectId: setup.projectId,
      runId: admitted.effect.run_id, fetchImpl: setup.fetchImpl });
    assert.equal(run.status, "completed");
    assert.equal(run.context.sufficiency.kind, "complete");
    await page.locator('[data-assistant-inspect]').click();
    const evidence = page.locator('[data-run-evidence]');
    await expect(evidence).toBeVisible();
    await expect(evidence).toContainText("输入范围");
    await expect(evidence).toContainText(message);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
    await stopStoryOSServer(server.server);
  }
}

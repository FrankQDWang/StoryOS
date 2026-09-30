import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import type { BrowserContext } from "playwright";
import { expect } from "playwright/test";

import { cancelAgentRun, digestCancelAgentRun, getAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CancelAgentRunRequest, CreateAgentRunResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, challenged, id, prepare, settleOnce, USER_A } from "./acceptance";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration";

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
    let setup = await prepare(server.baseUrl, id("f87700"), "Run evidence acceptance", "f8771");
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
    await expect(evidence).toContainText("模型还记住了哪些内容，我们无法确认");
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
    const openCase = async (suffix: string) => {
      setup = await prepare(server.baseUrl, id(`f877${suffix}00`), `Run evidence ${suffix}`, `f877${suffix}`);
      await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
      await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
      await editor.waitFor();
      await page.locator(".composer button:not([disabled])").waitFor();
    };
    for (const [suffix, installed] of [["2", true], ["3", false]] as const) {
      await openCase(suffix);
      const runId = await submit("Compact active context between calls.");
      const hold = join(repositoryRoot, "target/issue-877/compaction.hold");
      writeFileSync(hold, "hold");
      const worker = settleOnce({ STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH: hold });
      void worker.catch(() => undefined);
      try {
        await expect.poll(async () => {
          const current = (await inspect(runId)).active_compaction;
          return current.kind === "present" ? current.install_state : "absent";
        }).toBe("staged");
        await page.locator('[data-assistant-inspect]').click();
        await expect(page.locator('[data-run-compaction="staged"]')).toContainText("摘要已准备好，尚未使用");
        if (!installed) {
          await editor.click();
          await page.keyboard.insertText("A later change.");
          await expect.poll(async () => (await inspect(runId)).context.current_availability.working_target.kind)
            .toBe("superseded");
        }
      } finally { if (existsSync(hold)) unlinkSync(hold); await worker; }
      await page.locator('[data-assistant-inspect]').click();
      const summary = page.locator(`[data-run-compaction="${installed ? "installed" : "refused"}"]`);
      await expect(summary).toContainText(installed ? "后面的生成已使用这份摘要。" : "章节或请求内容已改变");
      await expect(summary).toContainText("Bounded later-request summary. Semantic preservation is unknown.");
      await expect(summary).toContainText("无法确认摘要是否保留了原文的全部意思。");
      await page.reload();
      await expect(summary).toBeVisible();
    }
    for (const [suffix, disposition, explanation] of [
      ["4", "rebuilt", "已用还能读取的内容重新准备这次请求。"],
      ["5", "blocked", "无法找回先前内容：可用额度不足。"],
      ["6", "unknown_create", "先前那次生成的结果还不确定，不能认定先前内容已经过期。"],
    ] as const) {
      await openCase(suffix);
      const priorId = await submit("Help with this passage.");
      await settleOnce();
      await page.locator('[data-assistant-inspect]').click();
      await expect(evidence).toHaveAttribute("data-run-evidence-run-id", priorId);
      const prior = await inspect(priorId);
      assert.equal(prior.model_attempt.kind, "present");
      await queryStoryOSPostgres(`UPDATE storyos.model_attempts
        SET payload = payload || jsonb_build_object('produced_binding',
          COALESCE(payload->'produced_binding', '{}'::jsonb) ||
          jsonb_build_object('reference_condition', 'confirmed_expired'
            ${disposition === "blocked" ? ", 'budget_exhausted', true" : ""}))
          ${disposition === "unknown_create" ? ", dispatch_state = 'uncertain'" : ""}
        WHERE project_id = '${setup.projectId}'::uuid AND run_id = '${priorId}'::uuid
          AND attempt_role = 'decision';`);
      const runId = await submit("I changed my mind: keep the voice.");
      await settleOnce();
      assert.equal((await inspect(runId)).reference_recovery.kind, "present");
      await page.locator('[data-assistant-inspect]').click();
      const recovery = page.locator(`[data-run-reference-recovery="${disposition}"]`);
      await expect(recovery).toContainText(explanation);
      await expect(recovery).toContainText("不能保证模型记住的内容被完整找回。");
      if (disposition === "unknown_create") {
        assert.ok(!(await recovery.innerText()).includes("先前内容的引用已过期或无法使用。"));
      }
      await page.reload();
      await expect(recovery).toContainText(explanation);
    }
    for (const [suffix, message, disposition, explanation] of [
      ["7", "SCRIPT:retrieve-complete", "settled", "已找回并确认最初那次生成的结果。"],
      ["8", "SCRIPT:retrieve-missing", "kept_unknown", "原来的结果仍无法确认"],
    ] as const) {
      await openCase(suffix);
      const runId = await submit(message);
      await settleOnce();
      const run = await inspect(runId);
      assert.equal(run.original_result_retrieval.kind, "present");
      await page.locator('[data-assistant-inspect]').click();
      const lookup = page.locator(`[data-run-result-retrieval="${disposition}"]`);
      await expect(lookup).toContainText(explanation);
      await expect(lookup).toContainText("没有重新生成，也没有继续原来的回复。");
      await page.reload();
      await expect(lookup).toContainText(explanation);
    }
    const cancel = async (runId: string, suffix: string) => {
      const request: CancelAgentRunRequest = { command_schema: "storyos.command.cancel-agent-run.request.v1",
        cancel_agent_run_input: { ...BINDING, correlation_id: id(`f877${suffix}b0`) } };
      return challenged(server.baseUrl, setup.fetchImpl, setup.projectId, "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel", request.command_schema,
        await digestCancelAgentRun(request), id(`f877${suffix}b1`), (antiForgery) => cancelAgentRun({
          baseUrl: server.baseUrl, projectId: setup.projectId, runId, fetchImpl: setup.fetchImpl,
          idempotencyKey: id(`f877${suffix}b1`), antiForgery, request }));
    };
    await openCase("9");
    const cancelledLookupId = await submit("SCRIPT:retrieve-complete");
    const lookupHold = join(repositoryRoot, "target/issue-877/lookup.hold");
    writeFileSync(lookupHold, "hold");
    const lookupWorker = settleOnce({ STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: lookupHold });
    void lookupWorker.catch(() => undefined);
    try {
      await expect.poll(async () => (await inspect(cancelledLookupId)).model_attempt.kind).toBe("present");
      assert.equal((await cancel(cancelledLookupId, "9")).effect.kind, "applied");
    } finally { if (existsSync(lookupHold)) unlinkSync(lookupHold); await lookupWorker; }
    await page.locator('[data-assistant-inspect]').click();
    await expect(page.locator('[data-run-result-retrieval="evidence_only"]'))
      .toContainText("找回的记录已保留，但请求已经结束，结果保持原状。");
    await page.reload();
    await expect(page.locator('[data-run-result-retrieval="evidence_only"]')).toBeVisible();
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
    await stopStoryOSServer(server.server);
  }
}

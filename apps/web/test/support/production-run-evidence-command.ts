import { existsSync, mkdirSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import type { BrowserContext } from "playwright";
import { expect } from "playwright/test";

import { cancelAgentRun, digestCancelAgentRun, getAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CancelAgentRunRequest, CreateAgentRunResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, challenged, drainLeftoverWork, id, prepare, settleOnce, USER_A } from "./acceptance";
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
  await page.setViewportSize({ width: 1487, height: 1058 });
  mkdirSync(join(repositoryRoot, "target/issue-877/screens"), { recursive: true });
  try {
    await drainLeftoverWork();
    let setup = await prepare(server.baseUrl, id("f87700"), "Run evidence acceptance", "f8771");
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.locator('[name="assistant-message"]').waitFor();
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    const saved = page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]');
    const original = "The lantern stayed lit.";
    await editor.click();
    await page.keyboard.insertText(original);
    await saved.waitFor();
    let latest: { projectId: string; runId: string } | undefined;
    const submit = async (message: string) => {
      if (latest?.projectId === setup.projectId) {
        const prior = await inspect(latest.runId);
        await page.reload();
        await page.locator(`[data-assistant-run-id="${latest.runId}"][data-assistant-dispatch="${prior.status}"]`).waitFor();
      }
      const responsePromise = page.waitForResponse((response) =>
        response.request().method() === "POST" && new URL(response.url()).pathname.endsWith("/agent-runs"));
      await page.locator('[name="assistant-message"]').fill(message);
      await page.locator(".composer button").click();
      const response = await responsePromise;
      assert.equal(response.status(), 202);
      const runId = (await response.json() as CreateAgentRunResponse).effect.run_id;
      latest = { projectId: setup.projectId, runId };
      return runId;
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
    assert.ok(first.evidence.some((item) => item.kind === "provider_opaque"));
    assert.equal(first.run_id, firstId);
    assert.equal(first.project_scope.project_id, setup.projectId);
    await editor.click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.insertText(" Later edit.");
    await saved.waitFor();
    const historical = await inspect(firstId);
    assert.equal(historical.context.current_availability.working_target.kind, "superseded");
    assert.deepEqual(historical.context.selected, first.context.selected);
    await page.reload();
    assert.deepEqual((await inspect(firstId)).context, historical.context);
    const secondId = await submit("Keep the scene calm.");
    await settleOnce();
    const second = await inspect(secondId);
    assert.equal(second.run_id, secondId);
    assert.equal(second.conversation_id, first.conversation_id);
    assert.deepEqual((await inspect(firstId)).context, historical.context);
    await assert.rejects(getAgentRun({ baseUrl: server.baseUrl, projectId: id("ffff"),
      runId: firstId, fetchImpl: setup.fetchImpl }), /404/);
    await page.reload();
    assert.deepEqual((await inspect(firstId)).context, historical.context);
    const openCase = async (suffix: string) => {
      setup = await prepare(server.baseUrl, id(`f877${suffix}00`), `Run evidence ${suffix}`, `f877${suffix}`);
      await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
      await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
      await editor.waitFor();
      await page.locator('[name="assistant-message"]').waitFor();
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
        const staged = (await inspect(runId)).active_compaction;
        assert.equal(staged.kind, "present");
        if (!installed) {
          await editor.click();
          await page.keyboard.insertText("A later change.");
          await expect.poll(async () => (await inspect(runId)).context.current_availability.working_target.kind)
            .toBe("superseded");
        }
      } finally { if (existsSync(hold)) unlinkSync(hold); await worker; }
      const summary = (await inspect(runId)).active_compaction;
      assert.ok(summary.kind === "present");
      assert.equal(summary.install_state, installed ? "installed" : "refused");
      assert.equal(summary.output_text, "Bounded later-request summary. Semantic preservation is unknown.");
      assert.ok(summary.loss_facts.includes("semantic_preservation_unknown"));
      if (!installed) assert.equal(summary.refusal_reason, "changed_input");
      await page.reload();
      assert.deepEqual((await inspect(runId)).active_compaction, summary);
    }
    for (const [suffix, disposition] of [
      ["4", "rebuilt"],
      ["5", "blocked"],
      ["6", "unknown_create"],
    ] as const) {
      await openCase(suffix);
      const priorId = await submit(disposition === "unknown_create"
        ? "Help with this passage." : "Help with this passage. SCRIPT:reference-expires");
      await settleOnce();
      const prior = await inspect(priorId);
      assert.equal(prior.model_attempt.kind, "present");
      if (disposition !== "rebuilt") {
        await queryStoryOSPostgres(`UPDATE storyos.model_attempts
          SET ${disposition === "blocked"
            ? `payload = jsonb_set(payload, '{produced_binding,budget_exhausted}', 'true'::jsonb)`
            : "dispatch_state = 'uncertain'"}
          WHERE project_id = '${setup.projectId}'::uuid AND run_id = '${priorId}'::uuid
            AND attempt_role = 'decision';`);
      }
      const runId = await submit("I changed my mind: keep the voice.");
      await settleOnce();
      const recovery = (await inspect(runId)).reference_recovery;
      assert.ok(recovery.kind === "present");
      assert.equal(recovery.disposition, disposition);
      assert.equal(recovery.lossless_provider_reconstruction, false);
      assert.equal(recovery.semantic_erasure, false);
      if (disposition === "blocked") assert.equal(recovery.block_reason, "budget_insufficient");
      await page.reload();
      assert.deepEqual((await inspect(runId)).reference_recovery, recovery);
    }
    for (const [suffix, message, disposition] of [
      ["7", "SCRIPT:retrieve-complete", "settled"],
      ["8", "SCRIPT:retrieve-missing", "kept_unknown"],
    ] as const) {
      await openCase(suffix);
      const runId = await submit(message);
      await settleOnce();
      const lookup = (await inspect(runId)).original_result_retrieval;
      assert.ok(lookup.kind === "present");
      assert.equal(lookup.disposition, disposition);
      assert.equal(lookup.repeats_original_create, false);
      assert.equal(lookup.resumes_stream, false);
      await page.reload();
      assert.deepEqual((await inspect(runId)).original_result_retrieval, lookup);
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
    await settleOnce();
    const cancelledLookup = await inspect(cancelledLookupId);
    assert.equal(cancelledLookup.status, "cancelled");
    assert.ok(cancelledLookup.original_result_retrieval.kind === "present");
    assert.equal(cancelledLookup.original_result_retrieval.disposition, "evidence_only");
    await page.reload();
    assert.deepEqual((await inspect(cancelledLookupId)).original_result_retrieval, cancelledLookup.original_result_retrieval);
    for (const [suffix, message, disposition] of [
      ["a", "SCRIPT:successor-missing", "dispatched"],
      ["b", "SCRIPT:successor-budget", "paused"],
    ] as const) {
      await openCase(suffix);
      const runId = await submit(message);
      await settleOnce();
      const run = await inspect(runId);
      const later = run.unknown_create_successor;
      assert.ok(later.kind === "present");
      assert.equal(later.disposition, disposition);
      assert.equal(later.successor_settles_predecessor, false);
      if (disposition === "dispatched") {
        assert.ok(run.model_attempt.kind === "present");
        assert.equal(run.status, "completed");
        assert.equal(run.model_attempt.dispatch_state, "uncertain");
      } else assert.equal(later.pause_reason, "budget_insufficient");
      await page.reload();
      assert.deepEqual((await inspect(runId)).unknown_create_successor, later);
    }
    await openCase("c");
    const successorId = await submit("SCRIPT:successor-once");
    const fenceHold = join(repositoryRoot, "target/issue-877/fence.hold");
    writeFileSync(fenceHold, "hold");
    const fenceWorker = settleOnce({ STORYOS_TEST_FAKE_SUCCESSOR_FENCE_HOLD_PATH: fenceHold });
    void fenceWorker.catch(() => undefined);
    try {
      await expect.poll(async () => {
        const successor = (await inspect(successorId)).unknown_create_successor;
        return successor.kind === "present" ? successor.disposition : "absent";
      }).toBe("fenced");
      const fenced = (await inspect(successorId)).unknown_create_successor;
      assert.ok(fenced.kind === "present");
      assert.ok(!fenced.successor_model_attempt_id);
      assert.equal((await cancel(successorId, "c")).effect.kind, "applied");
    } finally { if (existsSync(fenceHold)) unlinkSync(fenceHold); await fenceWorker; }
    await settleOnce();
    const prohibited = await inspect(successorId);
    assert.equal(prohibited.status, "cancelled");
    assert.ok(prohibited.unknown_create_successor.kind === "present");
    assert.equal(prohibited.unknown_create_successor.disposition, "prohibited");
    await page.reload();
    assert.deepEqual((await inspect(successorId)).unknown_create_successor, prohibited.unknown_create_successor);
    await openCase("d");
    const omitted = "This passage is outside the usable range. " + "a".repeat(10_001);
    await editor.click();
    await page.keyboard.insertText(omitted);
    await saved.waitFor();
    const blockedId = await submit("Help with this passage.");
    await settleOnce();
    const blocked = await inspect(blockedId);
    assert.equal(blocked.context.sufficiency.kind, "blocked");
    assert.ok(blocked.context.rejected.some((input) => input.reason.kind === "over_item_token_limit"));
    assert.ok(!blocked.context.selected.some((input) => input.content.includes("This passage is outside the usable range.")));
    assert.equal(blocked.model_attempt.kind, "absent");
    await page.reload();
    assert.deepEqual((await inspect(blockedId)).context, blocked.context);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
    await stopStoryOSServer(server.server);
  }
}

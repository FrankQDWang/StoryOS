import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { promisify } from "node:util";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { uuidV7 } from "../../src/acceptance-journal.ts";
import { createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun, getChapter, getProposal, cancelAgentRun, digestCancelAgentRun, pauseAgentRun, digestPauseAgentRun,
  updateProjectAssistance } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { runStoryOSWorker, queryStoryOSPostgres, sessionFetch } from "./node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const id = () => uuidV7(globalThis.crypto);
const workerBinary = join(repositoryRoot, "target", "release-package", "storyos-worker");
const execFileAsync = promisify(execFile);

export async function verifyProductionComposerControls(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  page.setDefaultTimeout(10_000);
  const hold = join(repositoryRoot, "target", "issue-875", "dispatch.hold");
  let held: Promise<unknown> | undefined;
  try {
    await page.goto(origin);
    await page.locator('input[name="title"]').fill(`Composer ${id()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator('[data-project-id]').getAttribute("data-project-id");
    assert.ok(projectId);
    const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
    const request: import("../../../../generated/typescript/storyos-public-release-1/client.mjs").UpdateProjectAssistanceRequest = { command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: id() } };
    const key = id();
    const challenge = await createProjectCommandChallenge({ ...options, request: { method: "PUT",
      route_template: "/api/v1/projects/{project_id}/assistance", command_schema: request.command_schema,
      canonical_command_digest: await digestUpdateProjectAssistance(request), idempotency_key: key } });
    await updateProjectAssistance({ ...options, request, idempotencyKey: key, antiForgery: challenge.nonce });
    await page.locator('[data-add-chapter]').click();
    await page.locator('[data-create-volume-action]').click();
    await page.locator('input[name="volume-title"]').fill("Composer Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('[data-add-chapter]').click();
    await page.locator('[data-chapter-placement="append"]').click();
    await page.locator('input[name="chapter-title"]').fill("Composer Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    const chapterId = await page.locator('nav[aria-label="稿件目录"] button[aria-current="true"]')
      .getAttribute("data-chapter-id");
    assert.ok(chapterId);
    await page.locator('[data-manuscript-editor][contenteditable="true"]').click();
    await page.keyboard.insertText("The lantern went dark.");
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.body)
      .toBe("The lantern went dark.");
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.goto(`${origin}/projects/${projectId}`);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const composer = page.locator('textarea[name="assistant-message"]');
    await expect(composer).toHaveCount(1);
    await composer.fill("Revise this passage: keep the voice.");
    const short = await composer.boundingBox();
    assert.ok(short && short.height >= 50 && short.height < 75);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-875", "composer-two-lines.png") });
    await composer.fill(Array.from({ length: 10 }, (_, index) => `Line ${index + 1}`).join("\n"));
    const long = await composer.boundingBox();
    assert.ok(long && long.height > short.height && long.height <= 225);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-875", "composer-ten-lines.png") });
    await composer.fill(Array.from({ length: 14 }, () => "Another line").join("\n"));
    assert.equal((await composer.boundingBox())?.height, long.height);
    let admitted: import("../../../../generated/typescript/storyos-public-release-1/client.mjs").CreateAgentRunResponse | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const response = await route.fetch();
      assert.equal(response.status(), 202);
      admitted = await response.json();
      await route.fulfill({ response });
    });
    await composer.fill("Revise this passage: keep the voice.");
    await page.getByRole("button", { name: "发送", exact: true }).click();
    await expect.poll(() => admitted?.effect.kind).toBe("admitted");
    assert.ok(admitted?.effect.kind === "admitted");
    const firstId = admitted.effect.run_id;
    const conversationId = admitted.conversation_id;
    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
    const first = await getAgentRun({ ...options, runId: firstId });
    assert.equal(first.status, "completed");
    assert.ok(first.decision.kind === "prose_change" && first.decision.opened_proposal.kind === "present");
    const proposalId = first.decision.opened_proposal.proposal_id;
    const proposal = (await getProposal({ ...options, proposalId })).proposal;
    const revision = (await getChapter({ ...options, chapterId })).chapter.current_revision;
    await page.locator('[data-assistant-inspect]').click();
    await expect(page.locator('[data-assistant-inspect]')).toHaveCount(0);
    await composer.fill("Help with this passage.");
    await page.getByRole("button", { name: "发送", exact: true }).click();
    await expect.poll(() => admitted?.effect.kind === "admitted" ? admitted.effect.run_id : undefined)
      .not.toBe(firstId);
    assert.ok(admitted?.effect.kind === "admitted");
    const runId = admitted.effect.run_id;
    assert.equal(admitted.conversation_id, conversationId);
    writeFileSync(hold, "hold");
    held = execFileAsync(workerBinary, ["--once"], { cwd: repositoryRoot, timeout: 60_000,
      env: { ...process.env, STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL,
        STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH: hold, STORYOS_EXPORT_LEASE_TTL_SECS: "0" } });
    void held.catch(() => undefined);
    await expect.poll(async () => (await getAgentRun({ ...options, runId })).model_attempt.kind).toBe("present");
    await expect(composer).toHaveValue("");
    await expect(page.getByRole("button", { name: "暂停", exact: true })).toBeEnabled();
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-875", "composer-active-empty.png") });
    await composer.fill("Keep the voice.");
    await expect(page.getByRole("button", { name: "发送", exact: true })).toBeEnabled();
    await composer.fill("");
    await page.getByRole("button", { name: "暂停", exact: true }).click();
    await expect.poll(async () => (await getAgentRun({ ...options, runId })).status).toBe("paused");
    await expect(page.getByRole("button", { name: "发送", exact: true })).toBeDisabled();
    assert.deepEqual((await getProposal({ ...options, proposalId })).proposal, proposal);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter.current_revision, revision);
    unlinkSync(hold);
    await held;
    held = undefined;
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0 WHERE project_id = '${projectId}'::uuid`);
    const guidance = "Keep the voice and the ending open.";
    const inputs: Array<{ body: unknown; key: string | undefined; response: unknown }> = [];
    await page.route((url) => url.pathname.endsWith("/steering-inputs"), async (route) => {
      const response = await route.fetch();
      assert.equal(response.status(), 200);
      inputs.push({ body: route.request().postDataJSON(), key: route.request().headers()["idempotency-key"],
        response: await response.json() });
      if (inputs.length === 1) await route.abort("failed");
      else await route.fulfill({ response });
    });
    await composer.fill(guidance);
    await page.getByRole("button", { name: "发送", exact: true }).click();
    await expect(page.getByRole("status")).toContainText("操作结果待确认");
    await expect(composer).toHaveValue(guidance);
    await expect(page.getByRole("button", { name: "发送", exact: true })).toBeDisabled();
    await page.reload();
    await expect.poll(() => inputs.length).toBe(2);
    assert.deepEqual(inputs[1], inputs[0]);
    await expect(composer).toHaveValue("");
    const retained = await getAgentRun({ ...options, runId });
    assert.equal(retained.conversation_id, conversationId);
    assert.equal(retained.status, "queued");
    assert.deepEqual(retained.steering_inputs.map(input => input.author_message), [guidance]);
    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
    const consumed = await getAgentRun({ ...options, runId });
    assert.equal(consumed.status, "completed");
    assert.ok(consumed.model_attempt.kind === "present");
    assert.equal(consumed.steering_inputs[0]?.model_attempt_id, consumed.model_attempt.model_attempt_id);
    assert.equal(consumed.context.selected.find(item => item.source_class === "author_instruction")?.content,
      ["Help with this passage.", guidance].join("\n"));
    assert.deepEqual((await getProposal({ ...options, proposalId })).proposal, proposal);
    await page.locator('[data-assistant-inspect]').click();
    await expect(page.locator('[data-assistant-inspect]')).toHaveCount(0);
    await expect(page.locator('.assistant-author-message').filter({ hasText: guidance })).toHaveCount(1);
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0 WHERE project_id = '${projectId}'::uuid`);
    await composer.fill("Help with this passage again.");
    await page.getByRole("button", { name: "发送", exact: true }).click();
    await expect.poll(() => admitted?.effect.kind === "admitted" ? admitted.effect.run_id : undefined).not.toBe(runId);
    assert.ok(admitted?.effect.kind === "admitted");
    const cancelledId = admitted.effect.run_id;
    const browserFetch: typeof fetch = async (input, init) => {
      const response = await page.evaluate(async ({ url, body, headers, method }) => {
        const result = await fetch(url, { method, body, headers, credentials: "same-origin" });
        return { status: result.status, body: await result.text(), headers: [...result.headers] };
      }, { url: String(input), body: String(init?.body), headers: [...new Headers(init?.headers)], method: init?.method ?? "POST" });
      return new Response(response.body, { status: response.status, headers: response.headers });
    };
    const cancel = { command_schema: "storyos.command.cancel-agent-run.request.v1" as const,
      cancel_agent_run_input: { client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: id() } };
    const cancelKey = id();
    const cancelOptions = { ...options, fetchImpl: browserFetch };
    const cancelChallenge = await createProjectCommandChallenge({ ...cancelOptions, request: { method: "POST",
      route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel", command_schema: cancel.command_schema,
      canonical_command_digest: await digestCancelAgentRun(cancel), idempotency_key: cancelKey } });
    const cancelled = await cancelAgentRun({ ...cancelOptions, runId: cancelledId, request: cancel,
      idempotencyKey: cancelKey, antiForgery: cancelChallenge.nonce });
    assert.ok(cancelled.effect.kind === "applied" && cancelled.effect.status === "cancelled");
    assert.equal(cancelled.receipt.command_kind, "cancelAgentRun");
    await page.reload();
    await expect(page.locator('[data-assistant-inspect]')).toHaveCount(0);
    assert.equal((await getAgentRun({ ...options, runId: cancelledId })).status, "cancelled");
    assert.deepEqual((await getProposal({ ...options, proposalId })).proposal, proposal);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter.current_revision, revision);
    await expect(page.getByRole("button", { name: "暂停", exact: true })).toHaveCount(0);
    const pauseRequest = { command_schema: "storyos.command.pause-agent-run.request.v1" as const,
      pause_agent_run_input: { ...cancel.cancel_agent_run_input, correlation_id: id() } };
    const pauseKey = id();
    const refusedChallenge = await createProjectCommandChallenge({ ...cancelOptions, request: { method: "POST",
      route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause", command_schema: pauseRequest.command_schema,
      canonical_command_digest: await digestPauseAgentRun(pauseRequest), idempotency_key: pauseKey } });
    const refused = await pauseAgentRun({ ...cancelOptions, runId: cancelledId, request: pauseRequest,
      idempotencyKey: pauseKey, antiForgery: refusedChallenge.nonce });
    assert.deepEqual(refused.effect, { kind: "conflicted", reason: "terminal_run" });
    assert.equal((await getAgentRun({ ...options, runId: cancelledId })).status, "cancelled");
    const observer = await context.newPage();
    try {
      await observer.goto(`${origin}/projects/${projectId}`);
      await observer.locator('[data-take-over-writer]').click();
      await observer.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
      await page.reload();
      await page.locator('[data-manuscript-editor][contenteditable="false"]').waitFor();
      await composer.fill("Guidance from the old writer.");
      await expect(page.getByRole("button", { name: "发送", exact: true })).toBeDisabled();
      assert.equal(admitted.effect.run_id, cancelledId);
      assert.deepEqual((await getProposal({ ...options, proposalId })).proposal, proposal);
      assert.deepEqual((await getChapter({ ...options, chapterId })).chapter.current_revision, revision);
    } finally {
      await observer.close();
    }

  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    if (held !== undefined) await Promise.allSettled([held]);
    await page.close();
  }
}

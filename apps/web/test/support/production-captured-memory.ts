import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";

import { cancelAgentRun, createProjectCommandChallenge, digestCancelAgentRun,
  digestUpdateProjectAssistance, getAgentRun, updateProjectAssistance }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CancelAgentRunRequest, CreateAgentRunResponse, UpdateProjectAssistanceRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { queryStoryOSPostgres, sessionFetch, startStoryOSServer, stopStoryOSServer }
  from "./node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const USER = "018f0000-0000-7001-8000-000000000001";
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

function uuidV7(): string {
  const bytes = webcrypto.getRandomValues(new Uint8Array(16));
  let now = Date.now();
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export async function verifyProductionCapturedMemory(context: BrowserContext): Promise<void> {
  const owned = await startStoryOSServer({ repositoryRoot,
    serverBinary: join(repositoryRoot, "target/release-package/storyos-server"),
    sessions: { "session-a": USER },
  });
  const page = await context.newPage().catch(async (error: unknown) => {
    await stopStoryOSServer(owned.server);
    throw error;
  });
  let releaseOld = (): void => {};
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.setViewportSize({ width: 1487, height: 1058 });
    page.setDefaultTimeout(10_000);
    await page.goto(owned.baseUrl);
    await page.locator('#app[data-boot-state="protected-ready"]').waitFor();
    await page.locator('input[name="title"]').fill(`Captured Memory acceptance ${uuidV7()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator("form[data-rename]").getAttribute("data-rename");
    assert.ok(projectId);
    const options = { baseUrl: owned.baseUrl, projectId, fetchImpl: sessionFetch(owned.baseUrl, "session-a") };
    const assistance: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { ...BINDING, correlation_id: uuidV7(),
        availability: "available", expected_assistance_revision: "0" },
    };
    const assistanceKey = uuidV7();
    const assistanceChallenge = await createProjectCommandChallenge({ ...options, request: {
      method: "PUT", route_template: "/api/v1/projects/{project_id}/assistance",
      command_schema: assistance.command_schema,
      canonical_command_digest: await digestUpdateProjectAssistance(assistance), idempotency_key: assistanceKey,
    } });
    await updateProjectAssistance({ ...options, request: assistance,
      idempotencyKey: assistanceKey, antiForgery: assistanceChallenge.nonce });
    await page.locator('input[name="volume-title"]').fill("Memory Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('input[name="chapter-title"]').fill("Memory Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.goto(`${owned.baseUrl}/projects/${projectId}`);
    const admitted = async (message: string): Promise<CreateAgentRunResponse> => {
      await page.locator('.composer button:not([disabled])').waitFor();
      const response = page.waitForResponse((value) => value.request().method() === "POST"
        && value.url().endsWith("/agent-runs"));
      await page.locator('input[name="assistant-message"]').fill(message);
      await page.locator(".composer button").click();
      const received = await response;
      assert.equal(received.status(), 202);
      return received.json();
    };
    const first = await admitted("请记录这次请求的记忆设置。");
    await page.locator(`[data-assistant-run-id="${first.effect.run_id}"]`).waitFor();
    await page.locator("[data-assistant-inspect]").click();
    const memory = page.locator("[data-assistant-memory-settings]");
    await expect(memory).toHaveText("本次请求记录的记忆设置使用记忆：开启参与后续记忆整理：开启");
    await expect(memory).toHaveAttribute("data-assistant-memory-run-id", first.effect.run_id);
    const evidence = join(repositoryRoot, "target/issue-876/browser");
    await mkdir(evidence, { recursive: true });
    await page.screenshot({ path: join(evidence, "captured-default.png") });
    const cancel: CancelAgentRunRequest = {
      command_schema: "storyos.command.cancel-agent-run.request.v1",
      cancel_agent_run_input: { ...BINDING, correlation_id: uuidV7() },
    };
    const cancelKey = uuidV7();
    const cancelChallenge = await createProjectCommandChallenge({ ...options, request: {
      method: "POST", route_template: "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
      command_schema: cancel.command_schema, canonical_command_digest: await digestCancelAgentRun(cancel),
      idempotency_key: cancelKey,
    } });
    await cancelAgentRun({ ...options, runId: first.effect.run_id, request: cancel,
      idempotencyKey: cancelKey, antiForgery: cancelChallenge.nonce });
    const revision = uuidV7();
    await queryStoryOSPostgres(`
      UPDATE storyos.conversation_memory_settings SET is_current = FALSE
       WHERE project_id = '${projectId}'::uuid;
      INSERT INTO storyos.conversation_memory_settings
      SELECT (jsonb_populate_record(NULL::storyos.conversation_memory_settings,
        (to_jsonb(settings) - 'is_current') || jsonb_build_object(
          'memory_settings_revision', '${revision}', 'use_enabled', false))).*
        FROM storyos.conversation_memory_settings AS settings WHERE project_id = '${projectId}'::uuid;
      UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
       WHERE project_id = '${projectId}'::uuid;
    `);
    await page.locator("[data-assistant-inspect]").click();
    await expect(page.locator("[data-assistant-run-status]")).toHaveText("已取消");
    const second = await admitted("继续这个对话，并记录新的记忆设置。");
    assert.equal(second.conversation_id, first.conversation_id);
    assert.equal(second.memory_settings_revision, revision);
    await page.locator(`[data-assistant-run-id="${second.effect.run_id}"]`).waitFor();
    await page.locator("[data-assistant-inspect]").click();
    await expect(memory).toHaveText("本次请求记录的记忆设置使用记忆：关闭参与后续记忆整理：开启");
    await expect(memory).toHaveAttribute("data-assistant-memory-run-id", second.effect.run_id);
    const oldInspect = page.locator(`[data-assistant-history-inspect="${first.effect.run_id}"]`);
    await oldInspect.click();
    await expect(memory).toHaveText("本次请求记录的记忆设置使用记忆：开启参与后续记忆整理：开启");
    await page.reload();
    await expect(memory).toHaveAttribute("data-assistant-memory-run-id", first.effect.run_id);
    await expect(memory).toHaveText("本次请求记录的记忆设置使用记忆：开启参与后续记忆整理：开启");
    let reachedOld = (): void => {};
    const oldReached = new Promise<void>((resolve) => { reachedOld = resolve; });
    const heldOld = new Promise<void>((resolve) => { releaseOld = resolve; });
    const oldPath = `${owned.baseUrl}/api/v1/projects/${projectId}/agent-runs/${first.effect.run_id}`;
    await page.route(oldPath, async (route) => {
      const response = await route.fetch();
      reachedOld();
      await heldOld;
      await route.fulfill({ response });
    });
    await oldInspect.click();
    await oldReached;
    await page.locator("[data-assistant-inspect]").click();
    await expect(memory).toHaveAttribute("data-assistant-memory-run-id", second.effect.run_id);
    releaseOld();
    await page.unrouteAll({ behavior: "wait" });
    await expect(memory).toHaveText("本次请求记录的记忆设置使用记忆：关闭参与后续记忆整理：开启");
    await queryStoryOSPostgres(`CREATE POLICY issue_876_browser_withheld ON storyos.conversation_memory_settings
      AS RESTRICTIVE FOR SELECT TO storyos_runtime USING (
        NOT (project_id = '${projectId}'::uuid AND memory_settings_revision = '${revision}'::uuid));`);
    assert.deepEqual((await getAgentRun({ ...options, runId: second.effect.run_id })).captured_memory_settings,
      { kind: "unavailable" });
    await page.locator("[data-assistant-inspect]").click();
    await expect(memory).toHaveText("无法确认本次请求记录的记忆设置。");
    await page.screenshot({ path: join(evidence, "captured-unavailable.png") });
    await page.reload();
    await expect(memory).toHaveText("无法确认本次请求记录的记忆设置。");
    await page.locator(`[data-assistant-history-inspect="${first.effect.run_id}"]`).click();
    await expect(memory).toHaveText("本次请求记录的记忆设置使用记忆：开启参与后续记忆整理：开启");
    await page.screenshot({ path: join(evidence, "captured-history.png") });
    assert.deepEqual(pageErrors, []);
    const facts = await queryStoryOSPostgres(`
      SELECT json_build_object(
        'project_count', (SELECT count(*) FROM storyos.projects WHERE project_id = '${projectId}'::uuid),
        'receipts', (SELECT json_object_agg(command_kind, count) FROM (
          SELECT command_kind, count(*) FROM storyos.domain_receipts
          WHERE project_id = '${projectId}'::uuid GROUP BY command_kind) AS receipts),
        'author_action_count', (SELECT count(*) FROM storyos.author_action_entries
          WHERE project_id = '${projectId}'::uuid))::text;
    `);
    await writeFile(join(evidence, "fixture-facts.json"), facts + "\n");
  } finally {
    releaseOld();
    try {
      await page.unrouteAll({ behavior: "ignoreErrors" });
      await page.close();
    } finally {
      try {
        await queryStoryOSPostgres("DROP POLICY IF EXISTS issue_876_browser_withheld ON storyos.conversation_memory_settings");
      } finally {
        await stopStoryOSServer(owned.server);
      }
    }
  }
}

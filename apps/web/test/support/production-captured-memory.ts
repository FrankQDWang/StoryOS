import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
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
    const projectId = await page.locator("[data-project-id]").getAttribute("data-project-id");
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
    await page.locator("[data-add-chapter]").click();
    await page.locator("[data-create-volume-action]").click();
    await page.locator('input[name="volume-title"]').fill("Memory Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator("[data-add-chapter]").click();
    await page.locator('[data-chapter-placement="append"]').click();
    await page.locator('input[name="chapter-title"]').fill("Memory Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.goto(`${owned.baseUrl}/projects/${projectId}`);
    const admitted = async (message: string): Promise<CreateAgentRunResponse> => {
      await page.locator('[name="assistant-message"]').waitFor();
      const response = page.waitForResponse((value) => value.request().method() === "POST"
        && value.url().endsWith("/agent-runs"));
      await page.locator('[name="assistant-message"]').fill(message);
      await page.locator(".composer button").click();
      const received = await response;
      assert.equal(received.status(), 202);
      return received.json();
    };
    const first = await admitted("请记录这次请求的记忆设置。");
    await page.locator(`[data-assistant-run-id="${first.effect.run_id}"]`).waitFor();
    const inspect = (runId: string) => getAgentRun({ ...options, runId });
    const firstRun = await inspect(first.effect.run_id);
    assert.equal(firstRun.run_id, first.effect.run_id);
    assert.equal(firstRun.conversation_id, first.conversation_id);
    const firstSettings = { kind: "available", memory_settings_revision: first.memory_settings_revision,
      use_enabled: true, contribution_enabled: true };
    assert.deepEqual(firstRun.captured_memory_settings, firstSettings);
    const evidence = join(repositoryRoot, "target/issue-876/browser");
    await mkdir(evidence, { recursive: true });
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
    assert.equal((await inspect(first.effect.run_id)).status, "cancelled");
    await page.reload();
    const second = await admitted("继续这个对话，并记录新的记忆设置。");
    assert.equal(second.conversation_id, first.conversation_id);
    assert.equal(second.memory_settings_revision, revision);
    const secondSettings = { kind: "available", memory_settings_revision: revision,
      use_enabled: false, contribution_enabled: true };
    assert.deepEqual((await inspect(second.effect.run_id)).captured_memory_settings, secondSettings);
    assert.deepEqual((await inspect(first.effect.run_id)).captured_memory_settings, firstSettings);
    await page.reload();
    const historical = await inspect(first.effect.run_id);
    const current = await inspect(second.effect.run_id);
    assert.equal(historical.run_id, first.effect.run_id);
    assert.equal(current.run_id, second.effect.run_id);
    assert.equal(current.project_scope.project_id, projectId);
    assert.deepEqual(historical.captured_memory_settings, firstSettings);
    assert.deepEqual(current.captured_memory_settings, secondSettings);
    await queryStoryOSPostgres(`CREATE POLICY issue_876_browser_withheld ON storyos.conversation_memory_settings
      AS RESTRICTIVE FOR SELECT TO storyos_runtime USING (
        NOT (project_id = '${projectId}'::uuid AND memory_settings_revision = '${revision}'::uuid));`);
    assert.deepEqual((await getAgentRun({ ...options, runId: second.effect.run_id })).captured_memory_settings,
      { kind: "unavailable" });
    await page.reload();
    assert.deepEqual((await inspect(second.effect.run_id)).captured_memory_settings, { kind: "unavailable" });
    assert.deepEqual((await inspect(first.effect.run_id)).captured_memory_settings, firstSettings);
    await writeFile(join(evidence, "captured-settings.json"), JSON.stringify({ first: historical, second: current,
      unavailable: await inspect(second.effect.run_id) }, null, 2));
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

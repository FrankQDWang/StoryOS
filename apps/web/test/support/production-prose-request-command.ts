import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext } from "playwright";

import {
  createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getProjectAssistance, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunResponse, UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { runStoryOSWorker, sessionFetch } from "./node-integration";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const USER = "018f0000-0000-7001-8000-000000000001";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const MESSAGE = "Revise this passage: keep the voice.";

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

export async function verifyProductionProseRequest(context: BrowserContext): Promise<void> {
  const configured = process.env.STORYOS_DEV_SERVER;
  assert.ok(configured, "the packaged Server origin is required");
  const origin = new URL(configured).origin;
  const page = await context.newPage();
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.setDefaultTimeout(10_000);
  try {
    assert.equal((await page.goto(origin))?.status(), 200);
    await page.locator('#app[data-boot-state="protected-ready"]').waitFor();
    await page.locator('input[name="title"]').fill(`Prose request ${uuidV7()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator("form[data-rename]").getAttribute("data-rename");
    assert.ok(projectId !== null && UUID.test(projectId));
    await page.locator('input[name="volume-title"]').fill("Request Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('input[name="chapter-title"]').fill("Request Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    await editor.waitFor();
    const chapterId = await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]')
      .getAttribute("data-chapter-id");
    assert.ok(chapterId !== null && UUID.test(chapterId));
    await page.locator('[data-assistant-availability="unavailable"]').waitFor();
    assert.equal(await page.locator(".composer button").isDisabled(), true);
    await editor.click();
    await page.keyboard.insertText("The lantern went dark.");
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();

    const fetchImpl = sessionFetch(origin, "session-a");
    const options = { baseUrl: origin, projectId, fetchImpl };
    const before = await getChapter({ ...options, chapterId });
    assert.equal(before.project_scope.owner_user_id, USER);
    const unavailable = await getProjectAssistance(options);
    assert.equal(unavailable.assistance.availability, "unavailable");
    const request: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: {
        availability: "available",
        expected_assistance_revision: unavailable.assistance.revision,
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1",
        correlation_id: uuidV7(),
      },
    };
    const idempotencyKey = uuidV7();
    const challenge = await createProjectCommandChallenge({
      ...options,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/assistance",
        command_schema: request.command_schema,
        canonical_command_digest: await digestUpdateProjectAssistance(request),
        idempotency_key: idempotencyKey,
      },
    });
    const enabled = await updateProjectAssistance({
      ...options, request, idempotencyKey, antiForgery: challenge.nonce,
    });
    assert.equal(enabled.assistance.availability, "available");
    await page.reload();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator(".composer button:not([disabled])").waitFor();

    let posted = 0;
    let admitted: CreateAgentRunResponse | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      posted += 1;
      const response = await route.fetch();
      assert.equal(response.status(), 200);
      admitted = await response.json() as CreateAgentRunResponse;
      await route.abort("failed");
    });
    await page.locator('input[name="assistant-message"]').fill(MESSAGE);
    await page.locator(".composer button").click();
    await page.locator('[data-assistant-dispatch="uncertain"]').waitFor();
    assert.ok(admitted !== undefined && admitted.effect.kind === "admitted");
    const runId = admitted.effect.run_id;
    const correlationId = admitted.correlation_id;
    const pendingRef = await page.evaluate((key) => sessionStorage.getItem(key),
      `prose_request:${USER}:${projectId}`);
    assert.ok(pendingRef);
    assert.equal(JSON.parse(pendingRef).correlationId, correlationId);
    assert.equal(await page.locator(".composer button").isDisabled(), true);
    await page.reload();
    await page.locator('[data-assistant-run-id]').waitFor();
    await page.locator('[data-assistant-run-id="' + runId + '"]').waitFor();
    assert.equal(posted, 1, "reload must not resubmit the request");
    const queued = await getAgentRun({ ...options, runId });
    assert.equal(queued.run_id, runId);
    assert.equal(queued.conversation_id, admitted.conversation_id);
    assert.deepEqual(queued.project_scope, admitted.project_scope);
    assert.equal(queued.context.selected.find((item) =>
      item.source_class === "author_instruction")?.content, MESSAGE);
    assert.equal(queued.context.current_availability.working_target.kind, "current");

    for (let attempt = 0; attempt < 8; attempt += 1) {
      const current = await getAgentRun({ ...options, runId });
      if (current.status === "completed") break;
      await runStoryOSWorker({
        repositoryRoot,
        workerBinary: join(repositoryRoot, "target", "release-package", "storyos-worker"),
        args: ["--once"],
      });
    }
    const completed = await getAgentRun({ ...options, runId });
    assert.equal(completed.status, "completed");
    assert.equal(completed.decision.kind, "prose_change");
    await page.locator("[data-assistant-inspect]").click();
    await page.locator('[data-assistant-dispatch="completed"]').waitFor();
    assert.equal(await page.locator("[data-assistant-result]").textContent(),
      completed.decision.kind === "prose_change" ? completed.decision.text : null);
    assert.equal(await page.locator("[data-assistant-run-id]").getAttribute("data-assistant-run-id"), runId);
    await page.reload();
    await page.locator('[data-assistant-dispatch="completed"]').waitFor();
    assert.equal(await page.locator("[data-assistant-result]").textContent(),
      completed.decision.kind === "prose_change" ? completed.decision.text : null);
    assert.equal(posted, 1);
    const after = await getChapter({ ...options, chapterId });
    assert.deepEqual(after.chapter, before.chapter);
    assert.equal(await editor.textContent(), "The lantern went dark.");
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
    await context.clearCookies({ name: "storyos_session" });
  }
}

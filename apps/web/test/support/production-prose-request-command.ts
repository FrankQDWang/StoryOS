import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext } from "playwright";

import {
  createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getProjectAssistance, getProposal, StoryOSProtocolError, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest, CreateAgentRunResponse, UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { runStoryOSWorker, sessionFetch } from "./node-integration";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const USER = "018f0000-0000-7001-8000-000000000001";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
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
    await page.locator("form[data-rename]").waitFor();
    const projectId = await page.locator("form[data-rename]").getAttribute("data-rename");
    assert.ok(projectId !== null && UUID.test(projectId), `Project id: ${projectId}`);
    await page.locator('[data-assistant-availability="unavailable"]').waitFor();
    assert.equal(await page.locator(".composer button").isDisabled(), true);
    const fetchImpl = sessionFetch(origin, "session-a");
    const options = { baseUrl: origin, projectId, fetchImpl };
    await assert.rejects(() => getProjectAssistance(options), (error) =>
      error instanceof StoryOSProtocolError && error.status === 404);
    const request: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: {
        availability: "available",
        expected_assistance_revision: "0",
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
    await page.locator('input[name="volume-title"]').fill("Request Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('input[name="chapter-title"]').fill("Request Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    assert.equal((await page.goto(`${origin}/projects/${projectId}`))?.status(), 200);
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    await editor.waitFor();
    const chapterId = await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]')
      .getAttribute("data-chapter-id");
    assert.ok(chapterId !== null && UUID.test(chapterId));
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator(".composer button:not([disabled])").waitFor();
    await editor.click();
    await page.keyboard.insertText("The lantern went dark.");
    await page.keyboard.press("Enter");
    await page.keyboard.insertText("The second door opened.");
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    const before = await getChapter({ ...options, chapterId });
    assert.equal(before.project_scope.owner_user_id, USER);
    const [firstBlock, secondBlock] = before.chapter.current_revision.blocks;
    assert.ok(firstBlock && secondBlock);
    await page.evaluate(() => {
      document.body.dataset.authorInputEvents = "0";
      document.addEventListener("beforeinput", (event) => {
        if (!(event.target instanceof HTMLElement)
          || event.target.closest("[data-manuscript-editor]") === null) return;
        document.body.dataset.authorInputEvents = String(
          Number(document.body.dataset.authorInputEvents) + 1);
      }, { capture: true });
    });

    let posted = 0;
    let delivery: "lost" | "historical" = "lost";
    let admitted: CreateAgentRunResponse | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }
      posted += 1;
      const submitted = route.request().postDataJSON() as CreateAgentRunRequest;
      assert.deepEqual(submitted.create_agent_run_input.working_target,
        { kind: "current_chapter", chapter_id: chapterId });
      assert.deepEqual(submitted.create_agent_run_input.author_message, { text: MESSAGE });
      const response = await route.fetch();
      assert.equal(response.status(), 202);
      admitted = await response.json() as CreateAgentRunResponse;
      if (delivery === "lost") {
        await route.abort("failed");
      } else {
        await route.fulfill({ status: 409, contentType: "application/problem+json",
          body: JSON.stringify({ code: "historical_acknowledgement_unavailable" }) });
      }
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
    assert.equal(queued.context.selected.find((item) =>
      item.source_class === "working_target")?.content, before.chapter.current_revision.body);
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
    if (completed.decision.kind !== "prose_change"
      || completed.decision.opened_proposal.kind !== "present") throw new Error("Proposal not opened");
    const firstProposalId = completed.decision.opened_proposal.proposal_id;
    const firstProposal = (await getProposal({ ...options, proposalId: firstProposalId })).proposal;
    assert.equal(firstProposal.manuscript_block_id, firstBlock.manuscript_block_id);
    await page.locator("[data-assistant-inspect]").click();
    await page.locator('[data-assistant-dispatch="completed"]').waitFor();
    const firstCandidate = page.locator(`[data-proposal-id="${firstProposalId}"]`);
    await firstCandidate.waitFor();
    assert.equal(await firstCandidate.getAttribute("data-proposal-operation-id"),
      firstProposal.operations.find((operation) =>
        operation.manuscript_block_id === firstBlock.manuscript_block_id)?.operation_id);
    assert.equal(await firstCandidate.getAttribute("data-proposal-revision-id"), firstProposal.revision_id);
    assert.equal(await firstCandidate.getAttribute("data-proposal-source-run-id"), runId);
    assert.equal(await firstCandidate.getAttribute("data-proposal-source-decision-id"),
      completed.decision.decision_id);
    assert.equal(await firstCandidate.textContent(), `候选文字 · 尚未成为正文${firstProposal.candidate_text}`);
    assert.equal(await firstCandidate.evaluate((element) => element.previousElementSibling?.textContent),
      firstBlock.text);
    assert.equal(await page.locator("body").getAttribute("data-author-input-events"), "0");
    assert.equal(await page.locator("[data-assistant-result]").textContent(),
      completed.decision.kind === "prose_change" ? completed.decision.text : null);
    assert.equal(await page.locator("[data-assistant-run-id]").getAttribute("data-assistant-run-id"), runId);
    await page.reload();
    await page.locator('[data-assistant-dispatch="completed"]').waitFor();
    await page.locator(`[data-proposal-id="${firstProposalId}"]`).waitFor();
    assert.equal(await page.locator("[data-assistant-result]").textContent(),
      completed.decision.kind === "prose_change" ? completed.decision.text : null);
    assert.equal(posted, 1);
    const after = await getChapter({ ...options, chapterId });
    assert.deepEqual(after.chapter, before.chapter);
    assert.deepEqual(await page.locator("[data-manuscript-editor] > p").allTextContents(),
      ["The lantern went dark.", "The second door opened."]);
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    delivery = "historical";
    await page.locator('input[name="assistant-message"]').fill(MESSAGE);
    await page.locator(".composer button").click();
    await page.getByText("原始回复无法恢复。请刷新后查看当前结果。").waitFor();
    assert.ok(admitted !== undefined && admitted.effect.kind === "admitted");
    const secondRunId = admitted.effect.run_id;
    assert.notEqual(secondRunId, runId);
    await page.reload();
    await page.locator('[data-assistant-run-id="' + secondRunId + '"]').waitFor();
    assert.equal(posted, 2, "historical acknowledgement recovery must not resubmit");
    for (let attempt = 0; attempt < 8; attempt += 1) {
      const current = await getAgentRun({ ...options, runId: secondRunId });
      if (current.status === "completed") break;
      await runStoryOSWorker({
        repositoryRoot,
        workerBinary: join(repositoryRoot, "target", "release-package", "storyos-worker"),
        args: ["--once"],
      });
    }
    const second = await getAgentRun({ ...options, runId: secondRunId });
    if (second.decision.kind !== "prose_change"
      || second.decision.opened_proposal.kind !== "present") throw new Error("Second Proposal not opened");
    const secondProposalId = second.decision.opened_proposal.proposal_id;
    const secondProposal = (await getProposal({ ...options, proposalId: secondProposalId })).proposal;
    assert.notEqual(secondProposalId, firstProposalId);
    assert.equal(secondProposal.manuscript_block_id, secondBlock.manuscript_block_id);
    await page.locator("[data-assistant-inspect]").click();
    await page.locator(`[data-proposal-id="${secondProposalId}"]`).waitFor();
    assert.equal(await page.locator("[data-proposal-id]").count(), 2);
    assert.equal(await page.locator(`[data-proposal-id="${secondProposalId}"]`).evaluate(
      (element) => element.previousElementSibling?.textContent), secondBlock.text);
    await page.reload();
    await page.locator("[data-proposal-id]").first().waitFor();
    assert.deepEqual((await page.locator("[data-proposal-id]").evaluateAll((elements) =>
      elements.map((element) => element.getAttribute("data-proposal-id")))).sort(),
    [firstProposalId, secondProposalId].sort());
    assert.equal(await page.locator(`[data-proposal-id="${secondProposalId}"]`)
      .getAttribute("data-proposal-revision-id"), secondProposal.revision_id);
    assert.equal(await page.locator(`[data-proposal-id="${secondProposalId}"]`)
      .getAttribute("data-proposal-source-run-id"), secondRunId);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-787-block-proposals.png"),
      fullPage: true });
    const missingBlockId = uuidV7();
    let readMode: "invalid" | "missing" = "invalid";
    await page.route((url) => url.pathname.endsWith(`/proposals/${firstProposalId}`),
      async (route) => {
        const response = await route.fetch();
        const body = await response.json() as Awaited<ReturnType<typeof getProposal>>;
        if (readMode === "invalid") {
          body.proposal.validation = "invalid";
        } else {
          body.proposal.manuscript_block_id = missingBlockId;
          body.proposal.operations = body.proposal.operations.map((operation) => ({
            ...operation, manuscript_block_id: missingBlockId,
          }));
        }
        await route.fulfill({ response, body: JSON.stringify(body) });
      });
    await page.locator("[data-assistant-inspect]").click();
    const ineligible = page.locator(`[data-proposal-id="${firstProposalId}"]`);
    await page.locator(`[data-proposal-id="${firstProposalId}"][data-proposal-eligibility="ineligible"]`)
      .waitFor();
    assert.ok((await ineligible.textContent())?.includes(firstProposal.candidate_text));
    assert.equal(await ineligible.getAttribute("data-proposal-revision-id"), firstProposal.revision_id);
    readMode = "missing";
    await page.locator("[data-assistant-inspect]").click();
    await page.locator(`[data-proposal-unavailable="${firstProposalId}"]`).waitFor();
    assert.equal(await page.locator(`[data-proposal-id="${firstProposalId}"]`).count(), 0);
    assert.equal(await page.locator(`[data-proposal-id="${secondProposalId}"]`).count(), 1);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, before.chapter);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
    await context.clearCookies({ name: "storyos_session" });
  }
}

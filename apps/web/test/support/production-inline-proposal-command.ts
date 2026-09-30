import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";
import {
  createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getProposal, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunResponse, UpdateProjectAssistanceRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { runStoryOSWorker, sessionFetch } from "./node-integration.ts";

function uuidV7() {
  const value = randomUUID();
  const time = Date.now().toString(16).padStart(12, "0");
  return `${time.slice(0, 8)}-${time.slice(8)}-7${value.slice(15)}`;
}

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const SOURCE = "Guard the narrator voice in this passage.";
const SIBLING = "The second door opened.";

export async function verifyProductionInlineProposal(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.setDefaultTimeout(10_000);
  try {
    assert.equal((await page.goto(origin))?.status(), 200);
    await page.locator('#app[data-boot-state="protected-ready"]').waitFor();
    await page.locator('input[name="title"]').fill(`Inline Proposal ${uuidV7()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator("form[data-rename]").getAttribute("data-rename");
    assert.ok(projectId);
    const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
    const request: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: uuidV7() },
    };
    const idempotencyKey = uuidV7();
    const challenge = await createProjectCommandChallenge({ ...options, request: {
      method: "PUT", route_template: "/api/v1/projects/{project_id}/assistance",
      command_schema: request.command_schema, canonical_command_digest: await digestUpdateProjectAssistance(request),
      idempotency_key: idempotencyKey,
    } });
    await updateProjectAssistance({ ...options, request, idempotencyKey, antiForgery: challenge.nonce });
    await page.locator('input[name="volume-title"]').fill("Inline Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('input[name="chapter-title"]').fill("Inline Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.goto(`${origin}/projects/${projectId}`);
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    await editor.waitFor();
    const chapterId = await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]')
      .getAttribute("data-chapter-id");
    assert.ok(chapterId);
    await editor.click();
    await page.keyboard.insertText(SOURCE);
    await page.keyboard.press("Enter");
    await page.keyboard.insertText(SIBLING);
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks.map((block) => block.text))
      .toEqual([SOURCE, SIBLING]);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    const before = await getChapter({ ...options, chapterId });
    assert.deepEqual(before.chapter.current_revision.blocks.map((block) => block.text), [SOURCE, SIBLING]);
    let admitted: CreateAgentRunResponse | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const response = await route.fetch();
      admitted = await response.json() as CreateAgentRunResponse;
      await route.fulfill({ response });
    });
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator('input[name="assistant-message"]').fill("Revise this phrase: keep the voice.");
    await page.locator(".composer button").click();
    await expect.poll(() => admitted?.effect.kind).toBe("admitted");
    assert.ok(admitted?.effect.kind === "admitted");
    const runId = admitted.effect.run_id;
    for (let attempt = 0; attempt < 8; attempt += 1) {
      if ((await getAgentRun({ ...options, runId })).status === "completed") break;
      await runStoryOSWorker({ repositoryRoot, workerBinary: join(repositoryRoot,
        "target", "release-package", "storyos-worker"), args: ["--once"] });
    }
    const run = await getAgentRun({ ...options, runId });
    assert.equal(run.status, "completed");
    assert.ok(run.decision.kind === "prose_change" && run.decision.opened_proposal.kind === "present");
    const proposalId = run.decision.opened_proposal.proposal_id;
    const proposal = (await getProposal({ ...options, proposalId })).proposal;
    assert.equal(proposal.kind, "inline_edit");
    assert.equal(proposal.validation, "valid");
    await page.locator('[data-assistant-inspect]').click();
    const candidate = page.locator(`span[data-inline-proposal-id="${proposalId}"]`);
    await expect(candidate).toHaveCount(1);
    assert.equal(await candidate.textContent(), "narrator tone");
    assert.equal(await candidate.evaluate((element) => element.parentElement?.textContent),
      "Guard the narrator tone in this passage.");
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, before.chapter);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-ready.png") });
    await page.locator(`[data-proposal-id="${proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
    await editor.waitFor();
    await candidate.click();
    await candidate.evaluate((element) => {
      const range = document.createRange();
      range.setStart(element.firstChild!, 5); range.collapse(true);
      const selection = window.getSelection()!;
      selection.removeAllRanges(); selection.addRange(range);
    });
    await page.keyboard.insertText("xx");

    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.candidate_text)
      .toBe("narrxxator tone");
    const edited = (await getProposal({ ...options, proposalId })).proposal;
    assert.notEqual(edited.revision_id, proposal.revision_id);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, before.chapter);
    await page.reload();
    await expect(candidate).toHaveText("narrxxator tone");
    assert.equal(await candidate.getAttribute("data-proposal-revision-id"), edited.revision_id);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-edited-reloaded.png") });
    await page.locator(`[data-proposal-accept="${proposalId}"]`).click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.operation_resolution)
      .toBe("applied");
    const accepted = await getChapter({ ...options, chapterId });
    assert.deepEqual(accepted.chapter.current_revision.blocks, [
      { ...before.chapter.current_revision.blocks[0]!, text: "Guard the narrxxator tone in this passage." },
      before.chapter.current_revision.blocks[1]!,
    ]);
    await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([
      "Guard the narrxxator tone in this passage.", SIBLING,
    ]);
    await expect(candidate).toHaveCount(0);
    assert.deepEqual(errors, []);
  } finally { await page.close(); }
}

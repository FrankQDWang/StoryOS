import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import type { BrowserContext, Page } from "playwright";

import { getChapter, getProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, BlockProposalInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres, sessionFetch } from "./node-integration";

const USER = "018f0000-0000-7001-8000-000000000001";

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

function watchAcceptances(page: Page): { key: string; correlation: string }[] {
  const sent: { key: string; correlation: string }[] = [];
  page.on("request", (request) => {
    if (request.method() !== "POST" || !request.url().includes("/acceptances")) return;
    const body = request.postDataJSON() as AcceptProposalRequest;
    sent.push({ key: request.headers()["idempotency-key"] ?? "",
      correlation: body.accept_proposal_input.correlation_id });
  });
  return sent;
}

export async function verifyConflictedProposalRecovery(input: {
  page: Page; context: BrowserContext; origin: string; projectId: string; chapterId: string;
  proposalId: string; proposal: BlockProposalInspect; chapterRevisionId: string;
}): Promise<void> {
  const { page, context, origin, projectId, chapterId, proposalId, proposal } = input;
  const fetchImpl = sessionFetch(origin, "session-a");
  const read = { baseUrl: origin, projectId, chapterId, proposalId, fetchImpl };
  const candidate = page.locator(`[data-proposal-id="${proposalId}"]`);
  const prose = async () => (await getChapter(read)).chapter.current_revision.blocks
    .map((block) => block.text).join("\n");
  const originalProse = await prose();
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin });
  await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows
    SET issued_count = 0 WHERE owner_user_id = '${USER}'::uuid AND project_id = '${projectId}'::uuid`);
  await candidate.locator("button[data-proposal-accept]").waitFor();
  assert.equal(await candidate.getAttribute("data-proposal-validity"), "valid");
  assert.equal(await candidate.getAttribute("data-proposal-condition"), "absent");
  assert.equal(await candidate.getAttribute("data-proposal-session"), "eligible");
  await candidate.locator("button[data-proposal-copy]").click();
  await page.locator(`[data-proposal-decision="${proposalId}"]`).getByText("已复制候选文字。").waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), proposal.candidate_text);

  await queryStoryOSPostgres(`UPDATE storyos.proposal_revisions SET validation = 'invalid'
    WHERE project_id = '${projectId}'::uuid AND revision_id = '${proposal.revision_id}'::uuid`);
  await page.reload();
  await page.locator(`[data-proposal-id="${proposalId}"][data-proposal-validity="invalid"]`).waitFor();
  assert.equal(await candidate.locator("button[data-proposal-accept]").count(), 0);
  assert.equal(await candidate.locator("button[data-proposal-replan]").count(), 0);
  await page.getByText("候选文字的验证已失效，请检查当前结果。").waitFor();
  assert.equal((await getProposal(read)).proposal.validation, "invalid");
  await page.locator("[data-manuscript-editor][contenteditable='true']").waitFor();
  await queryStoryOSPostgres(`UPDATE storyos.proposal_revisions SET validation = 'valid'
    WHERE project_id = '${projectId}'::uuid AND revision_id = '${proposal.revision_id}'::uuid`);
  await page.reload();
  await candidate.locator("button[data-proposal-accept]").waitFor();

  let releaseStaleAccept = (): void => {};
  const staleAcceptHeld = new Promise<void>((resolve) => { releaseStaleAccept = resolve; });
  let noteStaleAccept = (): void => {};
  const staleAcceptPosted = new Promise<void>((resolve) => { noteStaleAccept = resolve; });
  await page.route((url) => url.pathname.endsWith("/acceptances"), async (route) => {
    if (route.request().method() !== "POST") { await route.continue(); return; }
    noteStaleAccept();
    await staleAcceptHeld;
    await route.fulfill({ response: await route.fetch() });
  });
  const writerAcceptances = watchAcceptances(page);
  await page.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-accept]`).click();
  await Promise.race([staleAcceptPosted, new Promise((_, reject) => {
    setTimeout(() => reject(new Error("stale accept was not posted")), 10_000);
  })]);
  const observer = await context.newPage();
  const observerAcceptances = watchAcceptances(observer);
  observer.setDefaultTimeout(10_000);
  try {
    assert.equal((await observer.goto(`${origin}/projects/${projectId}`))?.status(), 200);
    await observer.locator("[data-manuscript-editor][contenteditable='false']").waitFor();
    const locatorKey = `block_proposals:${USER}:${projectId}`;
    const stored = await page.evaluate((key) => sessionStorage.getItem(key), locatorKey);
    if (typeof stored !== "string") throw new Error("missing proposal locator");
    await observer.evaluate(({ key, value }) => sessionStorage.setItem(key, value),
      { key: locatorKey, value: stored });
    await observer.reload();
    await observer.locator("[data-take-over-writer]").click();
    await observer.locator("[data-manuscript-editor][contenteditable='true']").waitFor();
    releaseStaleAccept();
    await page.getByText("上次接受因编辑会话失效而被拒绝，候选文字仍保留。").waitFor();
    const refused = (await getProposal(read)).proposal;
    assert.equal(refused.validation, "valid");
    assert.equal(refused.candidate_text, proposal.candidate_text);
    assert.equal(refused.latest_acceptance_refusal.kind, "present");
    if (refused.latest_acceptance_refusal.kind === "present") {
      assert.ok(refused.latest_acceptance_refusal.reason === "stale_writer"
        || refused.latest_acceptance_refusal.reason === "session_changed");
    }
    assert.equal(await prose(), originalProse);
    await page.reload();
    await page.locator(`[data-proposal-id="${proposalId}"][data-proposal-session="ineligible"]`).waitFor();
    assert.equal(await page.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-accept]`).count(), 0);
    assert.equal(await prose(), originalProse);
    assert.equal(writerAcceptances.length, 1);

    const restored = observer.locator(`[data-proposal-id="${proposalId}"][data-proposal-session="eligible"]`);
    await restored.locator("button[data-proposal-accept]").waitFor();
    assert.equal(observerAcceptances.length, 0);
    assert.equal(await prose(), originalProse);
    const nextRevision = uuidV7();
    await queryStoryOSPostgres(`INSERT INTO storyos.authoritative_revisions
      SELECT owner_user_id, project_id, manuscript_object_id, '${nextRevision}'::uuid, payload_id
      FROM storyos.authoritative_revisions WHERE project_id = '${projectId}'::uuid
        AND revision_id = '${input.chapterRevisionId}'::uuid;
      INSERT INTO storyos.manuscript_revision_members
      SELECT owner_user_id, project_id, manuscript_object_id, '${nextRevision}'::uuid,
        manuscript_block_id, block_order FROM storyos.manuscript_revision_members
      WHERE project_id = '${projectId}'::uuid AND revision_id = '${input.chapterRevisionId}'::uuid;
      UPDATE storyos.authoritative_heads SET current_revision_id = '${nextRevision}'::uuid
      WHERE project_id = '${projectId}'::uuid AND manuscript_object_id = '${chapterId}'::uuid`);
    await restored.locator("button[data-proposal-accept]").click();
    await observer.locator(`[data-proposal-id="${proposalId}"][data-proposal-condition="proposal_conflict"]`).waitFor();
    assert.equal(await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-accept]`).count(), 0);
    await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-replan]`).waitFor();
    await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-reject]`).waitFor();
    await observer.getByText("正文已变化，候选文字尚未接受。").waitFor();
    await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-copy]`).click();
    await observer.getByText("已复制候选文字。").waitFor();
    assert.equal(await observer.evaluate(() => navigator.clipboard.readText()), proposal.candidate_text);
    assert.equal(observerAcceptances.length, 1);
    assert.notEqual(observerAcceptances[0]?.key, writerAcceptances[0]?.key);
    assert.notEqual(observerAcceptances[0]?.correlation, writerAcceptances[0]?.correlation);
    const conflicted = (await getProposal(read)).proposal;
    assert.equal(conflicted.validation, "conflicted");
    assert.equal(conflicted.source_condition.kind, "proposal_conflict");
    assert.equal(conflicted.candidate_text, proposal.candidate_text);
    if (conflicted.source_condition.kind !== "proposal_conflict") throw new Error("missing conflict");
    const conflictRef = conflicted.source_condition.proposal_conflict_ref;
    await observer.reload();
    await observer.evaluate(({ userId, id }) => {
      sessionStorage.setItem(`block_proposal_acceptance_eligible:${userId}:${id}`, "eligible");
    }, { userId: USER, id: projectId });
    await observer.reload();
    await observer.locator(`[data-proposal-id="${proposalId}"][data-proposal-condition="proposal_conflict"]`).waitFor();
    assert.equal(await observer.locator(`button[data-proposal-accept="${proposalId}"]`).count(), 0);

    await queryStoryOSPostgres(`UPDATE storyos.proposal_validation_conditions
      SET condition_kind = 'proposal_recovery_conflict' WHERE conflict_id = '${conflictRef}'::uuid`);
    await observer.reload();
    await observer.locator(`[data-proposal-id="${proposalId}"][data-proposal-condition="proposal_recovery_conflict"]`).waitFor();
    await observer.getByText("候选文字的恢复材料冲突，正文保持不变。").waitFor();
    await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-copy]`).click();
    await observer.getByText("已复制候选文字。").waitFor();
    assert.equal(await observer.evaluate(() => navigator.clipboard.readText()), proposal.candidate_text);
    assert.equal(await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-reject]`).count(), 0);
    assert.equal(await observer.locator("[data-proposal-withdraw]").count(), 0);
    await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-replan]`).waitFor();
    await queryStoryOSPostgres(`UPDATE storyos.proposal_validation_conditions
      SET condition_kind = 'proposal_conflict' WHERE conflict_id = '${conflictRef}'::uuid`);
    await observer.reload();
    await observer.locator(`[data-proposal-id="${proposalId}"][data-proposal-condition="proposal_conflict"]`).waitFor();

    let replanPosts = 0;
    let replanKey = "";
    await observer.route((url) => url.pathname.endsWith("/replans"), async (route) => {
      if (route.request().method() !== "POST") { await route.continue(); return; }
      replanPosts += 1;
      const headers = await route.request().allHeaders();
      if (replanKey === "") replanKey = headers["idempotency-key"] ?? "";
      else assert.equal(headers["idempotency-key"], replanKey);
      if (replanPosts === 1) {
        await route.abort("failed");
        return;
      }
      await route.fulfill({ response: await route.fetch() });
    });
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows
      SET issued_count = 0 WHERE owner_user_id = '${USER}'::uuid AND project_id = '${projectId}'::uuid`);
    await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-replan]`).click();
    await observer.getByText("重新规划结果尚未确认。请重试同一操作。").waitFor();
    assert.equal(replanPosts, 1);
    await observer.reload();
    await observer.locator(`button[data-proposal-replan="${proposalId}"]`).click();
    await observer.getByText("候选文字正在等待当前版本验证，尚不能接受。").waitFor();
    assert.equal(replanPosts, 2);
    const replanned = (await getProposal(read)).proposal;
    assert.equal(replanned.validation, "pending");
    assert.equal(replanned.validation_receipt.kind, "absent");
    assert.equal(replanned.candidate_text, proposal.candidate_text);
    assert.equal(replanned.source_condition.kind, "absent");
    assert.equal(await observer.locator(`[data-proposal-id="${proposalId}"] button[data-proposal-accept]`).count(), 0);
    await observer.unrouteAll();
    await observer.locator("[data-manuscript-editor][contenteditable='true']").waitFor();
    const paragraph = observer.locator("[data-manuscript-editor] > p").first();
    await paragraph.click();
    await observer.keyboard.insertText("Still writing. ");
    await observer.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    const written = await prose();
    assert.equal(written.includes("Still writing."), true);
    assert.equal(written.includes("The lantern went dark."), true);
    assert.equal((await getProposal(read)).proposal.candidate_text, proposal.candidate_text);
  } finally {
    releaseStaleAccept();
    await page.unrouteAll();
    await observer.close();
  }
}

import assert from "node:assert/strict";
import { expect } from "playwright/test";
import type { Page } from "playwright";
import type { ApplyAuthorEditRequest, ApplyAuthorEditResponse, BlockProposalInspect,
  GetChapterResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

/**
 * A selection from a block candidate into the next Chapter block, then typing, changes only the candidate
 * (ADR 0072). A lost response, a Server restart, and a reload do not submit the edit again.
 */
export async function verifyProductionFirstOwnerEdit({ page, projectId, chapter, proposal, restart }: {
  page: Page; projectId: string; chapter: GetChapterResponse; proposal: BlockProposalInspect;
  restart: () => Promise<void> }) {
  await page.setViewportSize({ width: 1487, height: 1058 });
  const blocks = chapter.chapter.current_revision.blocks;
  let posts = 0;
  let request: ApplyAuthorEditRequest | undefined;
  let response: ApplyAuthorEditResponse | undefined;
  let reached!: () => void;
  const committed = new Promise<void>((resolve) => { reached = resolve; });
  await page.route((url) => url.pathname.endsWith("/manuscript/author-edits"), async (route) => {
    posts += 1;
    request = route.request().postDataJSON() as ApplyAuthorEditRequest;
    const settled = await route.fetch();
    assert.equal(settled.status(), 200);
    response = await settled.json() as ApplyAuthorEditResponse;
    await route.abort("failed");
    reached();
  });
  await page.locator('[data-manuscript-editor][contenteditable="true"]').focus();
  await page.evaluate(({ proposalId }) => {
    const candidate = document.querySelector(`[data-proposal-id="${proposalId}"] .block-proposal-text`)?.firstChild;
    const right = document.querySelector("[data-manuscript-editor] > p:last-of-type")?.firstChild;
    if (!candidate || !right) throw new Error("Crossing editor sources missing");
    window.getSelection()?.setBaseAndExtent(candidate, 5, right, 7);
    document.dispatchEvent(new Event("selectionchange"));
  }, { proposalId: proposal.proposal_id });
  await page.keyboard.insertText(" first owner");
  await committed;
  const edited = `${proposal.candidate_text.slice(0, 5)} first owner`;
  assert.ok(request && response?.effect.kind === "proposal_revised", JSON.stringify(response?.effect));
  // The Journal can drop the common prefix of the old and new text, so examine the result of the one unit.
  const primitives = request.author_edit_units.flatMap((unit) => unit.normalized_primitives);
  const [primitive] = primitives;
  assert.ok(primitive?.kind === "replace_selection" && primitives.length === 1, JSON.stringify(request.author_edit_units));
  assert.deepEqual({ target: request.proposal_target?.proposal_id, to: primitive.to,
    result: proposal.candidate_text.slice(0, primitive.from) + primitive.text },
  { target: proposal.proposal_id, to: proposal.candidate_text.length, result: edited });
  await restart();
  await page.reload();
  const candidate = page.locator(`[data-proposal-id="${proposal.proposal_id}"] .block-proposal-text`);
  await expect(candidate).toHaveText(edited);
  await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
  assert.equal(posts, 1, "response loss and restart must not submit another command");
  assert.deepEqual(await page.locator("[data-manuscript-editor] > p").allTextContents(), blocks.map((block) => block.text));
  const read = await page.evaluate(async ({ projectId, chapterId, proposalId }) => {
    const [chapter, proposal] = await Promise.all([fetch(`/api/v1/projects/${projectId}/chapters/${chapterId}`),
      fetch(`/api/v1/projects/${projectId}/proposals/${proposalId}`)]);
    return { chapter: await chapter.json(), proposal: (await proposal.json()).proposal };
  }, { projectId, chapterId: chapter.chapter.chapter_id, proposalId: proposal.proposal_id });
  assert.deepEqual(read.chapter.chapter.current_revision, chapter.chapter.current_revision);
  assert.equal(read.proposal.candidate_text, edited);
}

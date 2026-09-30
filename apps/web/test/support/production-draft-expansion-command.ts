import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { Page } from "playwright";
import type { ExpandRefusedEditDraftResponse, RefusedEditDraftInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readProductionJournal } from "./production-discard-command.ts";
import { verifyProductionDraftUndo } from "./production-draft-undo-command.ts";

export async function verifyProductionDraftExpansion(page: Page, projectId: string,
  draft: RefusedEditDraftInspect, restart: () => Promise<void>): Promise<RefusedEditDraftInspect> {
  const surface = page.locator(`[data-refused-edit-draft="${draft.draft_id}"]`);
  const sessionRoute = (url: URL) => url.pathname.includes("/editor-sessions/");
  await page.route(sessionRoute, async (route) => {
    const reply = await route.fetch();
    const changed = await reply.json();
    for (const block of changed.base_snapshot.materialized_revision.blocks) block.text = `X${block.text.slice(1)}`;
    await route.fulfill({ response: reply, json: changed });
  });
  await surface.locator("button[data-draft-retry]").click();
  await surface.locator('select[name="draft-retry-target"]').selectOption("1");
  const target = surface.locator('textarea[name="draft-target-text"]');
  await target.waitFor(); await target.click(); await page.keyboard.press("ControlOrMeta+A");
  const conflict = page.waitForResponse((reply) => reply.url().endsWith(`/drafts/${draft.draft_id}/proposal-expansions`));
  await surface.locator("button[data-draft-expand]").click();
  assert.equal((await (await conflict).json()).effect.kind, "conflicted");
  await page.unroute(sessionRoute);
  await expect.poll(async () => (await readProductionJournal(page, projectId)).metadata!.filter((row) => String(row.key).startsWith("expansion-observation:")).length).toBe(1);
  await surface.locator("button[data-draft-retry]").click();
  await surface.locator('select[name="draft-retry-target"]').selectOption("1");
  await target.click(); await page.keyboard.press("ControlOrMeta+A");
  let response: ExpandRefusedEditDraftResponse | undefined;
  let request: unknown, key = "", frozen: Record<string, unknown> | undefined;
  let committed!: () => void, failed!: (error: unknown) => void;
  const settlement = new Promise<void>((resolve, reject) => { committed = resolve; failed = reject; });
  let posts = 0;
  const routeMatch = (url: URL) => url.pathname.endsWith(`/drafts/${draft.draft_id}/proposal-expansions`);
  await page.route(routeMatch, async (route) => {
    try {
      posts += 1;
      if (posts === 1) {
        request = route.request().postDataJSON(); key = route.request().headers()["idempotency-key"]!;
        frozen = (await readProductionJournal(page, projectId)).metadata!.find((row) =>
          String(row.key).startsWith(`expansion:${draft.draft_id}:`) && (row.group as { idempotency_key?: string })?.idempotency_key === key);
        assert.ok(frozen, "Expansion must be frozen before its first POST");
      } else {
        assert.deepEqual(route.request().postDataJSON(), request);
        assert.equal(route.request().headers()["idempotency-key"], key);
      }
      const reply = await route.fetch(); assert.equal(reply.status(), 200, await reply.text());
      const observed = await reply.json() as ExpandRefusedEditDraftResponse;
      if (response) assert.deepEqual(observed, response);
      response = observed;
      if (posts === 1) await route.abort("failed"); else await route.fulfill({ response: reply });
      committed();
    } catch (error) { failed(error); await route.abort("failed"); }
  });
  try {
    await surface.locator("button[data-draft-expand]").click(); await settlement;
    assert.ok(response && response.effect.kind === "proposal_created_from_draft");
    const proposalId = response.effect.proposal_id;
    const proposal = page.locator(`section[aria-label="Draft Proposal"][data-proposal-id="${proposalId}"]`);
    await expect(proposal.locator("[data-proposal-structured-block]")).toHaveText(["mixed"]);
    await restart(); await page.reload();
    await expect(proposal.locator("[data-proposal-structured-block]")).toHaveText(["mixed"]);
    assert.equal(posts, 2, "Recovery retries one frozen command identity");
    const sourceText = await proposal.innerText();
    assert.ok(!sourceText.includes(draft.draft_id) && !sourceText.includes(draft.draft_revision_id),
      "The Proposal shows its preserved source without technical Draft bindings");
    assert.deepEqual((await readProductionJournal(page, projectId)).metadata!.find((row) => row.key === frozen!.key), frozen);
    await proposal.scrollIntoViewIfNeeded();
    await page.screenshot({ path: fileURLToPath(new URL("../../../../target/issue-824/screenshots/draft-proposal.png", import.meta.url)) });
    const closed = await page.evaluate(async ({ projectId, draftId }) => {
      const reply = await fetch(`/api/v1/projects/${projectId}/refused-edit-drafts/${draftId}`);
      if (!reply.ok) throw new Error(`Expanded source read ${reply.status}`); return (await reply.json()).draft;
    }, { projectId, draftId: draft.draft_id }) as RefusedEditDraftInspect;
    assert.deepEqual(closed.payload, draft.payload);
    assert.equal(closed.closure_event?.event_id, response.effect.event.event_id);
    await page.unroute(routeMatch);
    return await verifyProductionDraftUndo(page, projectId, closed, restart);
  } finally { await page.unroute(routeMatch); }
}

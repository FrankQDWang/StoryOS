import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import type { BrowserContext, Page } from "playwright";
import type { ApplyAuthorEditRequest, ApplyAuthorEditResponse, BlockProposalInspect,
  GetChapterResponse, GetRefusedEditDraftResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

export async function verifyProductionRefusedEdit({ page, context, origin, projectId, chapter,
  proposal, restart }: { page: Page; context: BrowserContext; origin: string; projectId: string;
  chapter: GetChapterResponse; proposal: BlockProposalInspect; restart: () => Promise<void> }) {
  const blocks = chapter.chapter.current_revision.blocks;
  const right = blocks[1]!;
  let posts = 0;
  let request: ApplyAuthorEditRequest | undefined;
  let response: ApplyAuthorEditResponse | undefined;
  let key = "";
  let reached!: () => void;
  const committed = new Promise<void>((resolve) => { reached = resolve; });
  await page.route((url) => url.pathname.endsWith("/manuscript/author-edits"), async (route) => {
    posts += 1;
    request = route.request().postDataJSON() as ApplyAuthorEditRequest;
    key = route.request().headers()["idempotency-key"]!;
    const settled = await route.fetch();
    assert.equal(settled.status(), 200);
    response = await settled.json() as ApplyAuthorEditResponse;
    await route.abort("failed");
    reached();
  });
  const select = async () => page.evaluate(({ proposalId }) => {
    const candidate = document.querySelector(`[data-proposal-id="${proposalId}"] .block-proposal-text`)?.firstChild;
    const right = document.querySelector("[data-manuscript-editor] > p:last-of-type")?.firstChild;
    if (!candidate || !right) throw new Error("Mixed editor sources missing");
    window.getSelection()?.setBaseAndExtent(candidate, 5, right, 7);
    document.dispatchEvent(new Event("selectionchange"));
  }, { proposalId: proposal.proposal_id });
  await select();
  await page.keyboard.insertText("Complete mixed replacement");
  await committed;
  assert.ok(request && response && response.effect.kind === "refused_to_draft");
  const effect = response.effect;
  const unit = { normalized_primitives: [{ kind: "replace_structured_selection",
    replacement: [{ block_kind: "paragraph", text: "Complete mixed replacement" }] }],
    selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 5, to: 7,
      ordered_selection: { anchor: { source_index: 0, source_offset: 5 }, head: { source_index: 1, source_offset: 7 },
        sources: [{ owner: { kind: "proposal", proposal_id: proposal.proposal_id,
          operation_id: proposal.operation_id, revision_id: proposal.revision_id,
          manuscript_block_id: proposal.manuscript_block_id }, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
          from: 5, to: proposal.candidate_text.length, block_kind: "paragraph", source_text: proposal.candidate_text },
        { owner: { kind: "manuscript", manuscript_block_id: right.manuscript_block_id },
          coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 7,
          block_kind: "paragraph", source_text: "The second door opened." }] } } };
  assert.deepEqual(request.author_edit_units, [unit]);
  await restart();
  await page.reload();
  const draft = page.locator(`[data-refused-edit-draft="${effect.draft_id}"]`);
  await draft.locator("button[data-draft-copy]").waitFor();
  assert.equal(posts, 1, "response loss and restart must not submit another command");
  const read = async (): Promise<GetRefusedEditDraftResponse> => page.evaluate(async ({ projectId, draftId }) => {
    const result = await fetch(`/api/v1/projects/${projectId}/refused-edit-drafts/${draftId}`);
    if (!result.ok) throw new Error(`Draft read ${result.status}`);
    return result.json();
  }, { projectId, draftId: effect.draft_id });
  const retained = await read();
  const expectedPayload = { schema_revision: "storyos.refused-edit-payload.v1", chapter_id: request.chapter_id,
    expected_authoritative_revision_id: chapter.chapter.current_revision.revision_id,
    expected_proposal_head_revision_ids: [proposal.revision_id], target_refs: request.target_refs,
    author_edit_units: [unit], undo_group_id: request.undo_group_id,
    completed_intent_record_id: request.completed_intent_record_id, local_intent_sequence: request.local_intent_sequence };
  assert.deepEqual(retained.draft.payload, expectedPayload);
  const keys = new Set<string>();
  JSON.stringify(expectedPayload, (key, value: unknown) => { keys.add(key); return value; });
  assert.equal(retained.draft.payload_digest, createHash("sha256")
    .update(JSON.stringify(expectedPayload, [...keys].sort())).digest("hex"));
  assert.equal(retained.draft.draft_revision_id, effect.draft_revision_id);
  assert.equal(retained.draft.creation.creation_event_id, effect.creation_event_id);
  assert.deepEqual(retained.draft.creation.source, { command_id: response.command_id,
    author_command_admission_id: response.author_command_admission_id, receipt_id: response.receipt.receipt_id,
    idempotency_key: key, command_digest: response.receipt.command_digest });
  assert.equal(retained.draft.creation.creator.kind, "core_transition");
  assert.deepEqual(await page.locator("[data-manuscript-editor] > p").allTextContents(), blocks.map((block) => block.text));
  assert.equal(await page.locator(`[data-proposal-id="${proposal.proposal_id}"] .block-proposal-text`).textContent(), proposal.candidate_text);
  const journal = await page.evaluate((projectId) => new Promise<unknown[]>((resolve, reject) => {
    const open = indexedDB.open(`storyos-local-edit-journal:018f0000-0000-7001-8000-000000000001:${projectId}`);
    open.onupgradeneeded = () => open.transaction?.abort();
    open.onerror = () => reject(open.error);
    open.onsuccess = () => { const db = open.result; const read = db.transaction("intents").objectStore("intents").getAll();
      read.onsuccess = () => { db.close(); resolve(read.result as unknown[]); }; };
  }), projectId);
  assert.ok(journal.some((record) => JSON.stringify((record as { author_edit_unit?: unknown }).author_edit_unit) === JSON.stringify(unit)));
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin });
  const mutations: string[] = [];
  const track = (request: import("playwright").Request) => { if (request.method() !== "GET") mutations.push(request.url()); };
  page.on("request", track);
  await draft.locator("button[data-draft-copy]").click();
  await draft.getByText("Copied").waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "Complete mixed replacement");
  page.off("request", track);
  assert.deepEqual(mutations, []);
  assert.equal(await draft.locator("button").count(), 1);
  const wrong = await page.evaluate(async (draftId) => {
    const response = await fetch(`/api/v1/projects/018f0000-0000-7001-8000-000000000005/refused-edit-drafts/${draftId}`);
    return { status: response.status, text: await response.text() };
  }, effect.draft_id);
  assert.equal(wrong.status, 404);
  assert.ok(!wrong.text.includes("Complete mixed replacement"));
  const secondary = await context.newPage();
  try { await secondary.goto(`${origin}/projects/${projectId}`);
    await secondary.locator('[data-manuscript-editor][contenteditable="false"]').waitFor();
  } finally { await secondary.close(); }
  const draftRoute = (url: URL) => url.pathname.endsWith(`/refused-edit-drafts/${effect.draft_id}`);
  await page.route(draftRoute, async (route) => {
    const original = await route.fetch();
    const value = await original.json() as GetRefusedEditDraftResponse;
    await route.fulfill({ response: original, json: { ...value,
      project_scope: { ...value.project_scope, project_id: "018f0000-0000-7001-8000-000000000005" } } });
  });
  await page.reload();
  await page.locator(`[data-draft-unavailable="${effect.draft_id}"]`).waitFor();
  assert.equal(await page.locator("button[data-draft-copy]").count(), 0);
  await page.unroute(draftRoute);
  await page.reload();
  await page.locator("button[data-draft-copy]").waitFor();
  assert.deepEqual((await read()).draft, retained.draft);
  await page.route(draftRoute, (route) => route.fulfill({ status: 404, contentType: "application/problem+json",
    body: JSON.stringify({ code: "draft_unavailable" }) }));
  await page.locator("button[data-draft-copy]").click();
  await page.locator(`[data-draft-unavailable="${effect.draft_id}"]`).waitFor();
  assert.equal(await page.locator("[data-draft-replacement]").count(), 0);
  assert.equal(await page.locator("button[data-draft-copy]").count(), 0);
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "Complete mixed replacement");
  await page.unroute(draftRoute);
}

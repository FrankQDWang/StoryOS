import assert from "node:assert/strict";
import { expect } from "playwright/test";
import type { Page } from "playwright";
import type { CloseEditorFlowDraftResponse, RefusedEditDraftInspect, UndoLatestAuthorActionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readProductionJournal } from "./production-discard-command.ts";

export async function verifyProductionDraftUndo(page: Page, projectId: string,
  draft: RefusedEditDraftInspect, restart: () => Promise<void>): Promise<RefusedEditDraftInspect> {
  const surface = page.locator(`[data-refused-edit-draft="${draft.draft_id}"]`);
  const original = await readProductionJournal(page, projectId);
  let posts = 0;
  let response: UndoLatestAuthorActionResponse | undefined;
  let frozen: Record<string, unknown> | undefined;
  let request: unknown;
  let key = "", nonce = "";
  let release!: () => void;
  const committed = new Promise<void>((resolve) => { release = resolve; });
  const undoRoute = (url: URL) => url.pathname.endsWith(`/projects/${projectId}/author-actions/undo`);
  await page.route(undoRoute, async (route) => {
    posts += 1; request = route.request().postDataJSON();
    key = route.request().headers()["idempotency-key"]!;
    nonce = route.request().headers()["x-storyos-anti-forgery"]!;
    frozen = (await readProductionJournal(page, projectId)).metadata!.find((row) => String(row.key).startsWith("draft-undo:"));
    if (frozen === undefined) { await route.abort("failed"); release(); return; }
    const group = frozen.group as { frozen_request_body: unknown; idempotency_key: string };
    assert.deepEqual(group.frozen_request_body, request); assert.equal(group.idempotency_key, key);
    const reply = await route.fetch(); assert.equal(reply.status(), 200, await reply.text());
    response = await reply.json(); await route.abort("failed"); release();
  });
  await page.locator("[data-manuscript-editor]").focus();
  await page.keyboard.press("ControlOrMeta+Z");
  await expect.poll(() => posts, { timeout: 5000 }).toBe(1).catch(async (cause: unknown) => {
    throw new Error(`Root Undo did not submit (${await page.locator("[data-editor-failure]").getAttribute("data-editor-failure")}): ${await page.locator("body").innerText()}`, { cause });
  });
  await committed;
  assert.ok(frozen, "Original Undo identity must be durable before the first POST");
  assert.ok(response?.effect.kind === "draft_compensated");
  await restart(); await page.reload();
  await surface.locator("[data-draft-reopened]").waitFor();
  await surface.locator("button[data-draft-discard]").waitFor();
  assert.equal(posts, 1, "Reload must not create a new Undo");
  assert.equal(await surface.locator("button[data-draft-copy]").count(), 1);
  const read = () => page.evaluate(async ({ projectId, draftId }) => {
    const result = await fetch(`/api/v1/projects/${projectId}/refused-edit-drafts/${draftId}`);
    assertResponse(result); return (await result.json()).draft;
    function assertResponse(result: Response) { if (!result.ok) throw new Error(`Draft read ${result.status}`); }
  }, { projectId, draftId: draft.draft_id });
  const reopened = await read() as RefusedEditDraftInspect;
  assert.deepEqual(reopened, { ...draft, closure: "open", reopen_event: response.effect.event });
  const journal = await readProductionJournal(page, projectId);
  assert.deepEqual(journal.metadata!.find((row) => row.key === frozen!.key), frozen);
  for (const row of original.metadata!.filter((row) => String(row.key).startsWith("discard:")))
    assert.deepEqual(journal.metadata!.find((other) => other.key === row.key), row);
  await page.unroute(undoRoute);
  const replay = await page.evaluate(async ({ projectId, request, key, nonce }) => {
    const reply = await fetch(`/api/v1/projects/${projectId}/author-actions/undo`, { method: "POST",
      headers: { "content-type": "application/json", "idempotency-key": key, "x-storyos-anti-forgery": nonce },
      body: JSON.stringify(request) });
    if (!reply.ok) throw new Error(`Undo replay ${reply.status}`); return reply.json();
  }, { projectId, request, key, nonce });
  assert.deepEqual(replay, response); assert.deepEqual(await read(), reopened);
  const closeReply = page.waitForResponse((reply) => reply.url().endsWith(`/drafts/${draft.draft_id}/closures`) && reply.request().method() === "POST");
  await surface.locator("button[data-draft-discard]").click();
  const closedAgain = await (await closeReply).json() as CloseEditorFlowDraftResponse;
  assert.ok(closedAgain.effect.kind === "draft_closure_changed");
  const secondCloseEvent = closedAgain.effect.event;
  await surface.locator("[data-draft-closed]").waitFor();
  const discarded = (await readProductionJournal(page, projectId)).metadata!.find((row) => row.key === `discard:${draft.draft_id}:${reopened.reopen_event!.event_id}`)!;
  assert.equal((discarded.group as { frozen_request_body: { close_editor_flow_draft_input: { source_reopen_event_id: string } } }).frozen_request_body.close_editor_flow_draft_input.source_reopen_event_id, reopened.reopen_event!.event_id);
  let second!: UndoLatestAuthorActionResponse;
  await page.route(undoRoute, async (route) => {
    const reply = await route.fetch(); assert.equal(reply.status(), 200); second = await reply.json();
    await route.fulfill({ response: reply, json: { ...second, receipt: { ...second.receipt, draft_artifact_refs: [] } } });
  });
  await page.locator("[data-manuscript-editor]").focus(); await page.keyboard.press("ControlOrMeta+Z");
  await surface.locator("[data-draft-reopened]").waitFor();
  await surface.locator("button[data-draft-discard]").waitFor(); await page.unroute(undoRoute);
  assert.ok(second.effect.kind === "draft_compensated");
  const final = await read() as RefusedEditDraftInspect;
  assert.equal(final.reopen_event!.source_close_event_id, secondCloseEvent.event_id);
  assert.notEqual(final.reopen_event!.event_id, reopened.reopen_event!.event_id);
  const settled = await readProductionJournal(page, projectId);
  assert.equal(settled.metadata!.filter((row) => String(row.key).startsWith("draft-undo:")).length, 2);
  const observed = settled.metadata!.find((row) => row.key === `draft-undo-observation:${secondCloseEvent.event_id}`)!;
  assert.deepEqual(observed, { key: observed.key, record_key: `draft-undo:${secondCloseEvent.event_id}`, event: second.effect.event });
  for (const row of journal.metadata!.filter((row) => String(row.key).startsWith("discard:") || String(row.key).startsWith("draft-undo")))
    assert.deepEqual(settled.metadata!.find((other) => other.key === row.key), row);
  return final;
}

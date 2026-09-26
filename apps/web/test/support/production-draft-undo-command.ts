import assert from "node:assert/strict";
import type { Page } from "playwright";
import type { RefusedEditDraftInspect, UndoLatestAuthorActionResponse }
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
    assert.deepEqual(frozen.request, request); assert.equal(frozen.idempotency_key, key);
    const reply = await route.fetch(); assert.equal(reply.status(), 200, await reply.text());
    response = await reply.json(); await route.abort("failed"); release();
  });
  await page.locator("[data-manuscript-editor]").focus();
  await page.keyboard.press("ControlOrMeta+Z");
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
  return reopened;
}

import assert from "node:assert/strict";
import { expect } from "playwright/test";
import type { Page, Request, Response } from "playwright";
import type { CreateChapterResponse, GetChapterResponse, RefusedEditDraftInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

export async function verifyProductionDraftChapterSource(page: Page, projectId: string,
  chapter: GetChapterResponse, draft: RefusedEditDraftInspect, screenshots: string) {
  const surface = page.locator(`[data-refused-edit-draft="${draft.draft_id}"]`);
  const source = surface.locator("[data-draft-chapter-source]");
  const refused: string[] = [];
  const observe = (reply: Response) => {
    if (reply.status() >= 400) void reply.json().then((body) =>
      refused.push(`${reply.status()} ${new URL(reply.url()).pathname}: ${body.code}`)).catch(() => undefined);
  };
  page.on("response", observe);
  const volume = page.locator(`li[data-volume-id]:has(li[data-chapter-id="${chapter.chapter.chapter_id}"])`);
  await volume.locator("[data-create-chapter-menu]").click();
  await page.locator('[data-chapter-placement="append"]').click();
  const title = volume.locator('form[data-create-chapter] input[name="chapter-title"]');
  await title.fill("Other Chapter");
  const [created] = await Promise.all([page.waitForResponse((reply) => reply.request().method() === "POST"
    && new URL(reply.url()).pathname.startsWith(`/api/v1/projects/${projectId}/`)
    && new URL(reply.url()).pathname.endsWith("/chapters")).catch((error: unknown) => {
    throw new Error(`Chapter creation did not submit: ${refused.join("; ")}`, { cause: error });
  }), title.press("Enter")]);
  const creation = await created.json() as CreateChapterResponse;
  page.off("response", observe);
  assert.ok(creation.effect.kind === "authoritative_applied");
  await page.locator(`li[data-chapter-id="${creation.effect.chapter_id}"] [data-chapter-menu]`).click();
  await page.locator(`[data-make-current-chapter="${creation.effect.chapter_id}"]`).click();
  await expect(page.getByRole("heading", { name: "Other Chapter", exact: true })).toBeVisible();
  await expect(source).toContainText(`Source chapter: ${chapter.chapter.title}`);
  await expect(source).toContainText("This edit belongs to another chapter, not the chapter shown here.");
  await page.reload();
  await expect(source).toContainText(`Source chapter: ${chapter.chapter.title}`);
  await surface.scrollIntoViewIfNeeded(); await page.screenshot({ path: `${screenshots}/other-chapter-source.png` });
  const mutations: string[] = [];
  const track = (request: Request) => { if (request.method() !== "GET") mutations.push(request.url()); };
  page.on("request", track);
  await surface.locator("[data-draft-copy]").click();
  await surface.getByText("Copied", { exact: true }).waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "Complete mixed replacement");
  await surface.locator("[data-draft-retry]").click();
  await surface.getByText("Retry target unavailable", { exact: true }).waitFor();
  await expect(surface.locator("[data-draft-retry-submit]")).toBeDisabled();
  await surface.getByRole("button", { name: "Cancel", exact: true }).click();
  page.off("request", track); assert.deepEqual(mutations, []);
  const original = await page.evaluate(async ({ projectId, draftId }) => {
    const reply = await fetch(`/api/v1/projects/${projectId}/refused-edit-drafts/${draftId}`);
    if (!reply.ok) throw new Error(`Draft read ${reply.status}`);
    return (await reply.json()).draft;
  }, { projectId, draftId: draft.draft_id });
  assert.deepEqual(original, draft);
  const sourcePath = `/api/v1/projects/${projectId}/chapters/${chapter.chapter.chapter_id}`;
  const match = (url: URL) => url.pathname === sourcePath;
  await page.route(match, async (route) => {
    const response = await route.fetch({ url: `${new URL(page.url()).origin}/api/v1/projects/018f0000-0000-7001-8000-000000000005/chapters/${chapter.chapter.chapter_id}` });
    assert.equal(response.status(), 404); await route.fulfill({ response });
  });
  await surface.locator("[data-draft-copy]").click();
  await expect(source).toContainText("Source chapter details are unavailable.");
  await expect(surface.locator("[data-draft-replacement]")).toHaveText(["Complete mixed replacement"]);
  await page.reload(); await expect(source).toContainText("Source chapter details are unavailable.");
  assert.ok(!(await source.innerText()).includes(chapter.chapter.title));
  await page.unroute(match); await page.reload();
  await expect(source).toContainText(`Source chapter: ${chapter.chapter.title}`);
  let release!: () => void, reached!: () => void, delivered!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const fetched = new Promise<void>((resolve) => { reached = resolve; });
  const completed = new Promise<void>((resolve) => { delivered = resolve; });
  let first = true;
  await page.route(match, async (route) => {
    const response = await route.fetch();
    const delayed = first;
    if (delayed) { first = false; reached(); await held; }
    await route.fulfill({ response });
    if (delayed) delivered();
  });
  try {
    await surface.locator("[data-draft-copy]").click(); await fetched;
    await page.locator(`li[data-chapter-id="${chapter.chapter.chapter_id}"] [data-chapter-menu]`).click();
    await page.locator(`[data-begin-rename-chapter="${chapter.chapter.chapter_id}"]`).click();
    const rename = page.locator(`form[data-rename-chapter="${chapter.chapter.chapter_id}"] input`);
    await rename.fill("Renamed source chapter");
    const [renamed] = await Promise.all([page.waitForResponse((reply) => reply.request().method() === "PATCH"
      && new URL(reply.url()).pathname === sourcePath), rename.press("Enter")]);
    assert.equal(renamed.status(), 200);
    await page.locator(`li[data-chapter-id="${chapter.chapter.chapter_id}"] [data-chapter-menu]`).click();
    await page.locator(`[data-make-current-chapter="${chapter.chapter.chapter_id}"]`).click();
    await expect(source).toContainText("Source chapter: Renamed source chapter.");
    release(); await completed; await page.unroute(match);
    await expect(source).toContainText("Source chapter: Renamed source chapter.");
    await expect(source).toContainText("This edit belongs to the chapter shown here.");
    await surface.scrollIntoViewIfNeeded();
    await page.screenshot({ path: `${screenshots}/chapter-source.png` });
  } finally { release(); await page.unroute(match); }
}

export async function verifyRemovedDraftChapterSource(page: Page, projectId: string,
  draft: RefusedEditDraftInspect, screenshots: string) {
  const surface = page.locator(`[data-refused-edit-draft="${draft.draft_id}"]`);
  await page.locator(`li[data-chapter-id="${draft.payload.chapter_id}"] [data-chapter-menu]`).click();
  await page.locator(`[data-delete-chapter="${draft.payload.chapter_id}"]`).click();
  await page.locator(`[data-confirm-delete-chapter="${draft.payload.chapter_id}"]`).click();
  await expect(page.getByRole("heading", { name: "Other Chapter", exact: true })).toBeVisible();
  await expect(surface.locator("[data-draft-chapter-source]")).toContainText("Source chapter details are unavailable.");
  await expect(surface.locator("[data-draft-replacement]")).toHaveText(["Complete mixed replacement"]);
  assert.ok(!(await surface.innerText()).includes("deleted"));
  await surface.locator("[data-draft-copy]").click();
  await surface.getByText("Copied", { exact: true }).waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "Complete mixed replacement");
  const retained = await page.evaluate(async ({ projectId, draftId }) => {
    const reply = await fetch(`/api/v1/projects/${projectId}/refused-edit-drafts/${draftId}`);
    if (!reply.ok) throw new Error(`Removed source Draft read ${reply.status}`); return (await reply.json()).draft;
  }, { projectId, draftId: draft.draft_id }) as RefusedEditDraftInspect;
  assert.deepEqual(retained.payload, draft.payload);
  assert.equal(retained.draft_revision_id, draft.draft_revision_id);
  assert.deepEqual(retained.creation, draft.creation);
  assert.equal(await surface.locator("[data-draft-retry]").count(), 0);
  await page.reload();
  await expect(surface.locator("[data-draft-chapter-source]")).toContainText("Source chapter details are unavailable.");
  await surface.scrollIntoViewIfNeeded(); await page.screenshot({ path: `${screenshots}/removed-chapter-source.png` });
}

import assert from "node:assert/strict";
import type { BrowserContext, Cookie, Page } from "playwright";

import { manuscriptBody as visibleChapterBody } from "./manuscript-surface.ts";
import { queryStoryOSPostgres as queryPostgres } from "./node-integration";
import { assertProjectReceipts } from "./project-receipts.ts";

const USER = "018f0000-0000-7001-8000-000000000001";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const MANUSCRIPT_EDITOR = "[data-manuscript-editor]";
const MANUSCRIPT_EDITABLE = "[data-manuscript-editor][contenteditable='true']";

async function sessionId(page: Page, projectId: string): Promise<string> {
  const value = await page.evaluate((key) => sessionStorage.getItem(key),
    `active_session:${USER}:${projectId}`);
  assert.ok(value !== null && UUID.test(value), "the production Editor Session must exist");
  return value;
}

async function manuscriptBody(page: Page): Promise<string> {
  return page.locator(MANUSCRIPT_EDITOR).evaluate(visibleChapterBody);
}

async function clientSessionCookie(
  context: BrowserContext, origin: string,
): Promise<Cookie | undefined> {
  const matches = (await context.cookies(`${origin}/`))
    .filter((cookie) => cookie.name === "storyos_session");
  assert.ok(matches.length <= 1, "the browser holds at most one storyos_session cookie");
  return matches[0];
}

function assertIssuedSessionCookie(cookie: Cookie | undefined): void {
  assert.ok(cookie, "the printed-origin HTML GET must issue storyos_session");
  assert.deepEqual({
    httpOnly: cookie.httpOnly,
    name: cookie.name,
    path: cookie.path,
    sameSite: cookie.sameSite,
    secure: cookie.secure,
    value: cookie.value,
  }, {
    httpOnly: true,
    name: "storyos_session",
    path: "/",
    sameSite: "Strict",
    secure: false,
    value: "session-a",
  });
}

async function replaceAndSave(page: Page, text: string): Promise<void> {
  const settled = page.waitForResponse((response) =>
    response.url().endsWith("/manuscript/author-edits") && response.request().method() === "POST");
  const editor = page.locator(MANUSCRIPT_EDITABLE);
  await editor.click();
  await editor.press("ControlOrMeta+A");
  await page.keyboard.insertText(text);
  assert.equal((await settled).status(), 200);
  await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
  assert.equal(await manuscriptBody(page), text);
}

/** Focuses the editor with no click, moves the caret with `key`, types `text`, and waits for its automatic save. */
async function typeAndSave(page: Page, place: "start" | "end", text: string): Promise<void> {
  const settled = page.waitForResponse((response) =>
    response.url().endsWith("/manuscript/author-edits") && response.request().method() === "POST");
  const editor = page.locator(MANUSCRIPT_EDITABLE);
  await editor.focus();
  // A DOM selection puts the caret at the start or the end of the text, with no key that differs by platform.
  await editor.evaluate((element, atStart) => {
    const range = document.createRange();
    range.selectNodeContents(element);
    range.collapse(atStart);
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);
  }, place === "start");
  const [first, ...rest] = text.split("\n");
  if (first) await page.keyboard.insertText(first);
  for (const line of rest) {
    await page.keyboard.press("Enter");
    if (line) await page.keyboard.insertText(line);
  }
  const response = await settled;
  assert.equal(response.status(), 200);
  const body: unknown = await response.json();
  assert.ok(typeof body === "object" && body !== null);
  assert.equal(Reflect.get(Reflect.get(body, "effect") ?? {}, "kind"), "authoritative_applied",
    "the automatic save must apply on the Server");
  await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
}

/**
 * Types `text` at the start or the end of the text of a page that can have an earlier base, and waits until the page
 * shows `body` and saved it. A save on an earlier base gets `stale_authoritative_head`, and the page catches up with
 * no sentence (ADR 0107, section 2).
 */
async function typeOnEarlierBase(page: Page, place: "start" | "end", text: string, body: string): Promise<void> {
  const editor = page.locator(MANUSCRIPT_EDITABLE);
  await editor.focus();
  await editor.evaluate((element, atStart) => {
    const range = document.createRange();
    range.selectNodeContents(element);
    range.collapse(atStart);
    window.getSelection()?.removeAllRanges();
    window.getSelection()?.addRange(range);
  }, place === "start");
  await page.keyboard.insertText(text);
  await page.waitForFunction((expected) => {
    const status = document.querySelector('[data-save-state="saved"][data-unsettled-intent-count="0"]');
    const shown = [...document.querySelectorAll("[data-manuscript-editor] > p, [data-manuscript-editor] > h1")]
      .map((block) => block.textContent ?? "").join("\n");
    return status !== null && status.getAttribute("data-editor-failure") === "" && shown === expected;
  }, body);
}

export async function verifyProductionHostJourney(sharedContext: BrowserContext): Promise<void> {
  const configured = process.env.STORYOS_DEV_SERVER;
  assert.ok(configured, "run the production journey through the Project Scope verification entry");
  const server = new URL(configured);
  assert.ok(server.protocol === "http:" && server.hostname === "127.0.0.1"
    && server.port && server.pathname === "/" && !server.search && !server.hash
    && !server.username && !server.password, "the production fixture needs an exact loopback origin");
  const origin = server.origin;
  const browser = sharedContext.browser();
  assert.ok(browser, "the production cold-start journey needs an owned browser context");
  const context = await browser.newContext();
  const pages: Page[] = [];
  try {
    for (let index = 0; index < 3; index += 1) pages.push(await context.newPage());
    const [writer, observer, embedding] = pages;
    assert.ok(writer && observer && embedding);
    const errors: string[] = [];
    const requests: URL[] = [];
    for (const page of pages) {
      page.setDefaultTimeout(10_000);
      page.on("pageerror", (error) => errors.push(error.message));
      page.on("request", (request) => requests.push(new URL(request.url())));
    }
    assert.equal(await clientSessionCookie(context, origin), undefined);
    assert.equal((await writer.goto(`${origin}/`))?.status(), 200);
    assertIssuedSessionCookie(await clientSessionCookie(context, origin));
    await writer.locator('#app[data-boot-state="protected-ready"]').waitFor();
    assert.equal(await writer.evaluate(() => {
      try { document.createElement("div").innerHTML = "<p>blocked</p>"; return false; }
      catch (error) { return error instanceof TypeError; }
    }), true, "Chrome must enforce Trusted Types without a default policy");
    const blockedEmbedding = embedding.waitForEvent("console", {
      predicate: (message) => message.text().includes("frame-ancestors"),
    });
    await embedding.setContent(`<iframe src="${origin}/" title="blocked production frame"></iframe>`);
    await blockedEmbedding;
    assert.equal(await embedding.frameLocator("iframe").locator("#app").count(), 0);

    await writer.locator('input[name="title"]').fill("Production host acceptance");
    await writer.locator('input[name="title"]').press("Enter");
    await writer.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await writer.locator("[data-project-id]").getAttribute("data-project-id");
    assert.ok(projectId !== null && UUID.test(projectId));
    await writer.locator("[data-add-chapter]").click();
    await writer.locator("[data-create-volume-action]").click();
    await writer.locator('input[name="volume-title"]').fill("Production Volume");
    await writer.locator('input[name="volume-title"]').press("Enter");
    await writer.locator("[data-add-chapter]").click();
    await writer.locator('[data-chapter-placement="append"]').click();
    await writer.locator('input[name="chapter-title"]').fill("Production Chapter");
    await writer.locator('input[name="chapter-title"]').press("Enter");
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    const projectUrl = `${origin}/projects/${projectId}`;
    assert.equal((await writer.goto(projectUrl))?.status(), 200);
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    const writerId = await sessionId(writer, projectId);
    await replaceAndSave(writer, "Saved through the production host.");
    await writer.locator('[data-activity-replay-generation="1"]').waitFor({ state: "attached" });
    await writer.reload();
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.equal(await manuscriptBody(writer), "Saved through the production host.");
    assert.equal(await sessionId(writer, projectId), writerId);
    await writer.locator('[data-activity-replay-generation="1"]').waitFor({ state: "attached" });
    await queryPostgres(`
      INSERT INTO storyos.replay_generations (owner_user_id, project_id, replay_generation)
      VALUES ('${USER}'::uuid, '${projectId}'::uuid, 2)
      ON CONFLICT DO NOTHING;
      INSERT INTO storyos.replay_floors
        (owner_user_id, project_id, replay_generation, floor_position)
      VALUES ('${USER}'::uuid, '${projectId}'::uuid, 2, 0)
      ON CONFLICT DO NOTHING`);
    await replaceAndSave(writer, "Saved after replay generation two.");
    await writer.locator(
      '[data-activity-resync="applied"][data-activity-replay-generation="2"]',
    ).waitFor({ state: "attached" });
    assert.equal(await manuscriptBody(writer), "Saved after replay generation two.");

    // Page 1 adds a second block before page 2 opens the Project.
    await typeAndSave(writer, "end", "\nThe second block of page one.");
    assert.equal(await manuscriptBody(writer),
      "Saved after replay generation two.\nThe second block of page one.");

    // ADR 0107: page 2 opens the same Project and types at once, with no click on a button.
    assert.equal((await observer.goto(projectUrl))?.status(), 200);
    await observer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.notEqual(await sessionId(observer, projectId), writerId);
    await typeAndSave(observer, "start", "Page two: ");
    const afterPageTwo = "Page two: Saved after replay generation two.\nThe second block of page one.";
    await observer.reload();
    await observer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.equal(await manuscriptBody(observer), afterPageTwo);

    // Page 1 did not read again, so its base is earlier. It types on the second block, which page 2 did not change, and
    // both edits stay (ADR 0107, section 2).
    const afterPageOne = `${afterPageTwo} Page one again.`;
    await typeOnEarlierBase(writer, "end", " Page one again.", afterPageOne);
    await writer.reload();
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.equal(await manuscriptBody(writer), afterPageOne);

    // Both pages type on the first block. Page 2 types later on its earlier base, so its text of that block wins, and
    // the second block keeps the text of page 1.
    await typeOnEarlierBase(writer, "start", "One: ", `One: ${afterPageOne}`);
    const afterSameBlock = `Two: ${afterPageOne}`;
    await typeOnEarlierBase(observer, "start", "Two: ", afterSameBlock);
    await observer.reload();
    await observer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.equal(await manuscriptBody(observer), afterSameBlock);
    // The editor can send a typed line feed and the text after it in one or two Author Edits, so the count of that
    // kind comes from the store. Each applied Author Edit, the Volume, and the Chapter is one author action. Each save
    // on an earlier base also has a conflicted Receipt, which is no author action.
    const count = async (result: string) => Number(await queryPostgres(`SELECT count(*)::text
      FROM storyos.domain_receipts WHERE project_id = '${projectId}'::uuid AND command_kind = 'applyAuthorEdit'
      AND result_kind ${result}`));
    const [applied, conflicted] = [await count("= 'authoritative_applied'"), await count("= 'conflicted'")];
    assert.ok(applied >= 5 && conflicted === 2);
    await assertProjectReceipts([projectId], { projects: 1, receipts: { createProject: 1, createVolume: 1,
      createChapter: 1, applyAuthorEdit: applied + conflicted }, authorActions: applied + 2 });
    assert.deepEqual(errors, []);
    assert.ok(requests.length > 0 && requests.every((url) => url.origin === origin));
    assert.ok(requests.some((url) => url.pathname.startsWith("/assets/")));
    assert.ok(requests.every((url) => !url.pathname.startsWith("/@vite/")));
  } finally {
    await context.close();
  }
}

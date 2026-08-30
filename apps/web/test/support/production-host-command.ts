import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
import type { BrowserContext, Page } from "playwright";

import { getEditorSession }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

const USER = "018f0000-0000-7001-8000-000000000001";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const MANUSCRIPT_EDITOR = "[data-manuscript-editor]";
const MANUSCRIPT_EDITABLE = "[data-manuscript-editor][contenteditable='true']";
const MANUSCRIPT_READONLY = "[data-manuscript-editor][contenteditable='false']";
type JournalEvidence = Record<"partitions" | "intents" | "payload_chains", unknown[]>;

async function sessionId(page: Page, projectId: string): Promise<string> {
  const value = await page.evaluate((key) => sessionStorage.getItem(key),
    `active_session:${USER}:${projectId}`);
  assert.ok(value !== null && UUID.test(value), "the production Editor Session must exist");
  return value;
}

async function readJournal(page: Page, projectId: string): Promise<JournalEvidence> {
  return page.evaluate((name) => new Promise<JournalEvidence>((resolve, reject) => {
    const opened = indexedDB.open(name);
    opened.onupgradeneeded = () => opened.transaction?.abort();
    opened.onerror = () => reject(new Error("the existing production Journal is unavailable"));
    opened.onsuccess = () => {
      const database = opened.result;
      const stores = ["partitions", "intents", "payload_chains"] as const;
      const snapshot: JournalEvidence = { partitions: [], intents: [], payload_chains: [] };
      const transaction = database.transaction(stores, "readonly");
      let remaining = stores.length;
      for (const store of stores) {
        const request = transaction.objectStore(store).getAll();
        request.onerror = () => { database.close(); reject(request.error); };
        request.onsuccess = () => {
          const value: unknown = request.result;
          if (!Array.isArray(value)) {
            database.close(); reject(new Error("the Journal observation is invalid")); return;
          }
          snapshot[store] = value;
          remaining -= 1;
          if (remaining === 0) { database.close(); resolve(snapshot); }
        };
      }
    };
  }), `storyos-local-edit-journal:${USER}:${projectId}`);
}

function assertPreservedJournal(before: JournalEvidence, after: JournalEvidence): void {
  for (const store of ["partitions", "intents", "payload_chains"] as const) {
    for (const record of before[store]) {
      assert.ok(after[store].some((candidate) => isDeepStrictEqual(candidate, record)),
        `takeover must preserve the complete old ${store} record`);
    }
  }
}

// Only generated Editor Session queries cross this bridge. Explicit Takeover
// stays on the production page.
function takeoverFetch(page: Page, origin: string, projectId: string,
  writerId: string, observerId: string): typeof fetch {
  const scope = `/api/v1/projects/${projectId}`;
  const queryPaths = [writerId, observerId].map((id) => `${scope}/editor-sessions/${id}`);
  return async (input, init) => {
    assert.ok(input instanceof URL && input.origin === origin && !input.search && !input.hash);
    assert.equal(new URL(page.url()).origin, origin);
    assert.equal(init?.method, "GET");
    assert.ok(queryPaths.includes(input.pathname));
    assert.equal(init?.body, undefined);
    const result = await page.evaluate(async (request) => {
      const response = await fetch(request.url, {
        method: request.method, headers: request.headers, credentials: "same-origin",
      });
      return { body: await response.text(), status: response.status,
        headers: Object.fromEntries(response.headers) };
    }, { url: input.href, method: init?.method, headers: Object.fromEntries(new Headers(init?.headers)) });
    return new Response(result.body, { status: result.status, headers: result.headers });
  };
}

async function manuscriptBody(page: Page): Promise<string> {
  const value = await page.locator(MANUSCRIPT_EDITOR).getAttribute("data-manuscript-body");
  assert.ok(value !== null, "the production manuscript body must be present");
  return value;
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

export async function verifyProductionHostJourney(context: BrowserContext): Promise<void> {
  const configured = process.env.STORYOS_DEV_SERVER;
  assert.ok(configured, "run the production journey through the Project Scope verification entry");
  const server = new URL(configured);
  assert.ok(server.protocol === "http:" && server.hostname === "127.0.0.1"
    && server.port && server.pathname === "/" && !server.search && !server.hash
    && !server.username && !server.password, "the production fixture needs an exact loopback origin");
  const origin = server.origin;
  const pages: Page[] = [];
  try {
    await context.addCookies([{
      name: "storyos_session", value: "session-a", url: `${origin}/`,
      httpOnly: true, sameSite: "Strict", secure: false,
    }]);
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
    assert.equal((await writer.goto(`${origin}/`))?.status(), 200);
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
    const projectId = await writer.locator("form[data-rename]").getAttribute("data-rename");
    assert.ok(projectId !== null && UUID.test(projectId));
    await writer.locator('input[name="volume-title"]').fill("Production Volume");
    await writer.locator('input[name="volume-title"]').press("Enter");
    await writer.locator('input[name="chapter-title"]').fill("Production Chapter");
    await writer.locator('input[name="chapter-title"]').press("Enter");
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    const projectUrl = `${origin}/projects/${projectId}`;
    assert.equal((await writer.goto(projectUrl))?.status(), 200);
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    const writerId = await sessionId(writer, projectId);
    await replaceAndSave(writer, "Saved through the production host.");
    await writer.reload();
    await writer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.equal(await manuscriptBody(writer), "Saved through the production host.");
    assert.equal(await sessionId(writer, projectId), writerId);

    assert.equal((await observer.goto(projectUrl))?.status(), 200);
    await observer.locator(MANUSCRIPT_READONLY).waitFor();
    await observer.locator("[data-take-over-writer]").waitFor();
    const observerId = await sessionId(observer, projectId);
    assert.notEqual(observerId, writerId);
    const options = { baseUrl: origin, projectId,
      fetchImpl: takeoverFetch(observer, origin, projectId, writerId, observerId) };
    const prior = await getEditorSession({ ...options, editorSessionId: writerId });
    assert.ok(prior.writer.kind === "current_writer");
    const secondary = await getEditorSession({ ...options, editorSessionId: observerId });
    assert.deepEqual(secondary.writer, { kind: "read_only", reason: "secondary_session",
      observed_writer_generation: prior.writer.writer_generation });
    let releaseWriterEdits = (): void => {};
    const writerEditsHeld = new Promise<void>((resolve) => {
      releaseWriterEdits = resolve;
    });
    await writer.route("**/manuscript/author-edits", async (route) => {
      if (route.request().method() === "POST") await writerEditsHeld;
      await route.continue();
    });
    await writer.locator(MANUSCRIPT_EDITOR).click();
    await writer.locator(MANUSCRIPT_EDITOR).press("ControlOrMeta+A");
    await writer.keyboard.insertText("Unsettled before takeover.");
    await writer.locator('[data-save-state="saving"]').waitFor();
    await observer.locator("[data-take-over-writer]").click();
    await observer.locator(MANUSCRIPT_EDITABLE).waitFor();
    const generation = String(BigInt(prior.writer.writer_generation) + 1n);
    const winner = await getEditorSession({ ...options, editorSessionId: observerId });
    assert.deepEqual(winner.writer, { kind: "current_writer", writer_generation: generation });
    const fenced = await getEditorSession({ ...options, editorSessionId: writerId });
    assert.deepEqual(fenced.writer, { kind: "read_only", reason: "superseded_by_takeover",
      observed_writer_generation: generation });
    assert.equal(await manuscriptBody(writer), "Unsettled before takeover.");
    const refusal = writer.waitForResponse((response) => response.status() === 412
      && response.url().startsWith(`${origin}/api/v1/projects/${projectId}/`));
    releaseWriterEdits();
    const problem: unknown = await (await refusal).json();
    assert.ok(typeof problem === "object" && problem !== null);
    assert.equal(Reflect.get(problem, "code"), "editor_writer_stale");
    await writer.locator(MANUSCRIPT_READONLY).waitFor();
    await writer.locator('[data-save-state="needs_attention"]').waitFor();
    assert.equal(await manuscriptBody(writer), "Unsettled before takeover.");
    const oldJournal = await readJournal(writer, projectId);
    assert.equal(oldJournal.partitions.length, 2);
    assert.ok(oldJournal.intents.length > 0 && oldJournal.payload_chains.length > 0);
    await observer.reload();
    await observer.locator(MANUSCRIPT_EDITABLE).waitFor();
    assert.equal(await sessionId(observer, projectId), observerId);
    const newJournal = await readJournal(observer, projectId);
    assertPreservedJournal(oldJournal, newJournal);
    const added = newJournal.partitions.filter((record) =>
      !oldJournal.partitions.some((old) => isDeepStrictEqual(record, old)));
    assert.equal(added.length, 1);
    const partition = added[0];
    assert.ok(typeof partition === "object" && partition !== null);
    assert.deepEqual({ session: Reflect.get(partition, "editor_session_id"),
      generation: Reflect.get(partition, "writer_generation"),
      disposition: Reflect.get(partition, "disposition") },
    { session: observerId, generation, disposition: "current_writer_open" });
    assert.equal(await observer.locator(MANUSCRIPT_EDITOR).getAttribute("data-manuscript-body"),
      "Saved through the production host.");
    await replaceAndSave(observer, "Saved by the new production writer.");
    assertPreservedJournal(oldJournal, await readJournal(observer, projectId));
    assert.equal(await writer.locator(MANUSCRIPT_READONLY).getAttribute("data-manuscript-body"),
      "Unsettled before takeover.");
    assert.deepEqual(errors, []);
    assert.ok(requests.length > 0 && requests.every((url) => url.origin === origin));
    assert.ok(requests.some((url) => url.pathname.startsWith("/assets/")));
    assert.ok(requests.every((url) => !url.pathname.startsWith("/@vite/")));
  } finally {
    await Promise.all(pages.map((page) => page.close()));
    await context.clearCookies({ name: "storyos_session" });
  }
}

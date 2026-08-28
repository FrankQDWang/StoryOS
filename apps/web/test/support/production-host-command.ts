import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
import type { BrowserContext, Page } from "playwright";

import {
  createProjectCommandChallenge, digestTakeOverProjectWriter, getEditorSession,
  takeOverProjectWriter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { withChallengeRetry } from "./node-integration";

const USER = "018f0000-0000-7001-8000-000000000001";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
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

// Only the generated session queries and Takeover challenge/command cross this bridge.
function takeoverFetch(page: Page, origin: string, projectId: string,
  writerId: string, observerId: string): typeof fetch {
  const scope = `/api/v1/projects/${projectId}`;
  const queryPaths = [writerId, observerId].map((id) => `${scope}/editor-sessions/${id}`);
  const commandPaths = [`${scope}/anti-forgery-challenges`,
    `${scope}/editor-sessions/${observerId}/takeovers`];
  return async (input, init) => {
    assert.ok(input instanceof URL && input.origin === origin && !input.search && !input.hash);
    assert.equal(new URL(page.url()).origin, origin);
    const method = init?.method;
    assert.ok((method === "GET" && queryPaths.includes(input.pathname))
      || (method === "POST" && commandPaths.includes(input.pathname)));
    assert.ok(init?.body === undefined || typeof init.body === "string");
    const result = await page.evaluate(async (request) => {
      const response = await fetch(request.url, {
        method: request.method, headers: request.headers, credentials: "same-origin",
        ...(request.body === null ? {} : { body: request.body }),
      });
      return { body: await response.text(), status: response.status,
        headers: Object.fromEntries(response.headers) };
    }, { url: input.href, method, headers: Object.fromEntries(new Headers(init?.headers)),
      body: init?.body ?? null });
    return new Response(result.body, { status: result.status, headers: result.headers });
  };
}

async function replaceAndSave(page: Page, text: string): Promise<void> {
  const settled = page.waitForResponse((response) =>
    response.url().endsWith("/manuscript/author-edits") && response.request().method() === "POST");
  const editor = page.locator("textarea:not([readonly])");
  await editor.press("ControlOrMeta+A");
  await page.keyboard.insertText(text);
  assert.equal((await settled).status(), 200);
  await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
  assert.equal(await editor.inputValue(), text);
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
    await writer.locator("textarea:not([readonly])").waitFor();
    const projectUrl = `${origin}/projects/${projectId}`;
    assert.equal((await writer.goto(projectUrl))?.status(), 200);
    await writer.locator("textarea:not([readonly])").waitFor();
    const writerId = await sessionId(writer, projectId);
    await replaceAndSave(writer, "Saved through the production host.");
    await writer.reload();
    await writer.locator("textarea:not([readonly])").waitFor();
    assert.equal(await writer.locator("textarea").inputValue(), "Saved through the production host.");
    assert.equal(await sessionId(writer, projectId), writerId);

    assert.equal((await observer.goto(projectUrl))?.status(), 200);
    await observer.locator("textarea[readonly]").waitFor();
    const observerId = await sessionId(observer, projectId);
    assert.notEqual(observerId, writerId);
    const options = { baseUrl: origin, projectId,
      fetchImpl: takeoverFetch(observer, origin, projectId, writerId, observerId) };
    const prior = await getEditorSession({ ...options, editorSessionId: writerId });
    assert.ok(prior.writer.kind === "current_writer");
    const secondary = await getEditorSession({ ...options, editorSessionId: observerId });
    assert.deepEqual(secondary.writer, { kind: "read_only", reason: "secondary_session",
      observed_writer_generation: prior.writer.writer_generation });
    const request = {
      command_schema: "storyos.command.take-over-project-writer.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000901",
      editor_session_id: observerId, observed_writer_generation: prior.writer.writer_generation,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
    };
    const idempotencyKey = "018f0000-0000-7001-8000-000000000902";
    const digest = await digestTakeOverProjectWriter(request);
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      ...options, request: { method: "POST",
        route_template: "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
        command_schema: request.command_schema, canonical_command_digest: digest,
        idempotency_key: idempotencyKey },
    }));
    const takeover = await takeOverProjectWriter({ ...options, editorSessionId: observerId,
      request, idempotencyKey, antiForgery: challenge.nonce });
    assert.ok(takeover.result.kind === "takeover_applied");
    const generation = String(BigInt(prior.writer.writer_generation) + 1n);
    const winner = await getEditorSession({ ...options, editorSessionId: observerId });
    assert.deepEqual(winner.writer, { kind: "current_writer", writer_generation: generation });
    const fenced = await getEditorSession({ ...options, editorSessionId: writerId });
    assert.deepEqual(fenced.writer, { kind: "read_only", reason: "superseded_by_takeover",
      observed_writer_generation: generation });

    const refusal = writer.waitForResponse((response) => response.status() === 412
      && response.url().startsWith(`${origin}/api/v1/projects/${projectId}/`));
    await writer.locator("textarea").press("ControlOrMeta+A");
    await writer.keyboard.insertText("Unsent after takeover.");
    const problem: unknown = await (await refusal).json();
    assert.ok(typeof problem === "object" && problem !== null);
    assert.equal(Reflect.get(problem, "code"), "editor_writer_stale");
    await writer.locator('textarea[readonly]').waitFor();
    await writer.locator('[data-save-state="needs_attention"]').waitFor();
    assert.equal(await writer.locator("textarea").inputValue(), "Unsent after takeover.");
    const oldJournal = await readJournal(writer, projectId);
    assert.equal(oldJournal.partitions.length, 2);
    assert.ok(oldJournal.intents.length > 0 && oldJournal.payload_chains.length > 0);
    await observer.reload();
    await observer.locator("textarea:not([readonly])").waitFor();
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
    assert.equal(await observer.locator("textarea").inputValue(), "Saved through the production host.");
    await replaceAndSave(observer, "Saved by the new production writer.");
    assertPreservedJournal(oldJournal, await readJournal(observer, projectId));
    assert.equal(await writer.locator("textarea[readonly]").inputValue(), "Unsent after takeover.");
    assert.deepEqual(errors, []);
    assert.ok(requests.length > 0 && requests.every((url) => url.origin === origin));
    assert.ok(requests.some((url) => url.pathname.startsWith("/assets/")));
    assert.ok(requests.every((url) => !url.pathname.startsWith("/@vite/")));
  } finally {
    await Promise.all(pages.map((page) => page.close()));
    await context.clearCookies({ name: "storyos_session" });
  }
}

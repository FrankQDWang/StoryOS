import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { promisify } from "node:util";

import { JOURNAL_DATABASE_VERSION } from "../src/local-edit-journal.mjs";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const distDir = join(repositoryRoot, "apps/web/dist");
const serverBinary = join(
  repositoryRoot, "target", "debug",
  process.platform === "win32" ? "storyos-server.exe" : "storyos-server",
);
const execFileAsync = promisify(execFile);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const chromeExecutable = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean).find(existsSync);

const OPEN_BODY = "Authoritative A";
const AFTER_TYPE = "Authoritative A Hello";
const AFTER_IME = "Authoritative A Hello中文";
const AFTER_PASTE = "Authoritative A Hello中文 EN";
const AFTER_UNSETTLED = "Authoritative A Hello中文 EN!";

function contentType(pathname) {
  switch (extname(pathname)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    default:
      return "application/octet-stream";
  }
}

function distPath(pathname) {
  const relative = pathname === "/" || /^\/projects\/[0-9a-f-]{36}\/?$/i.test(pathname)
    ? "index.html" : pathname.replace(/^\/+/, "");
  const resolved = join(distDir, relative);
  if (!resolved.startsWith(`${distDir}/`) && resolved !== join(distDir, "index.html")) {
    return null;
  }
  return resolved;
}

function devToolsAddress(browser) {
  return new Promise((resolve, reject) => {
    let stderr = "";
    const timeout = setTimeout(() => reject(new Error(`Chrome DevTools did not start: ${stderr}`)), 10_000);
    browser.stderr.setEncoding("utf8");
    browser.stderr.on("data", (chunk) => {
      stderr += chunk;
      const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    });
    browser.once("exit", (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`Chrome exited before DevTools started (${code ?? signal}): ${stderr}`));
    });
  });
}

async function startRealServer() {
  return new Promise((resolve, reject) => {
    const server = spawn(serverBinary, ["--bind", "127.0.0.1:0"], {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL,
        STORYOS_BOOTSTRAP_SESSIONS: JSON.stringify({ "session-a": USER_A }),
        STORYOS_CHALLENGE_SECRET: "test-only-challenge-secret-that-is-at-least-thirty-two-bytes",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "", stderr = "";
    const fail = (error) => { clearTimeout(timeout); server.kill("SIGTERM"); reject(error); };
    const timeout = setTimeout(() => fail(new Error(`StoryOS Server did not become ready: ${stderr}`)), 5_000);
    server.once("error", fail);
    server.once("exit", (code) => fail(new Error(`StoryOS Server exited with ${code}: ${stderr}`)));
    server.stderr.on("data", (chunk) => { stderr += chunk; });
    server.stdout.on("data", (chunk) => {
      stdout += chunk;
      const match = stdout.match(/^STORYOS_SERVER_URL=(http:\/\/[^\s]+)$/m);
      if (match) { clearTimeout(timeout); resolve({ baseUrl: match[1], server }); }
    });
  });
}

async function stopProcess(child) {
  if (!child || child.exitCode !== null) return;
  const exited = once(child, "exit");
  child.kill("SIGTERM");
  await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
  if (child.exitCode === null) child.kill("SIGKILL");
}

async function queryPostgres(query) {
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container, "run through scripts/verify-project-scope.sh");
  const { stdout } = await execFileAsync("docker", [
    "exec", container, "psql", "-XAt", "-U", "postgres", "-c", query,
  ]);
  return stdout.trim();
}

async function projectAuthoritySnapshot() {
  const query = `
    SELECT json_build_object(
      'receipts', (SELECT count(*) FROM storyos.domain_receipts
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'admissions', (SELECT count(*) FROM storyos.author_command_admissions
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
          AND command_kind = 'applyAuthorEdit'),
      'consumed_challenges', (SELECT count(*) FROM storyos.project_command_challenges
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
          AND command_kind = 'applyAuthorEdit' AND consumed_at IS NOT NULL),
      'activities', (SELECT count(*) FROM storyos.project_activity_events
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'revision_envelopes', (SELECT count(*) FROM storyos.authoritative_revision_envelopes
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'commits', (SELECT count(*) FROM storyos.authoritative_commits
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'actions', (SELECT count(*) FROM storyos.author_action_entries
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'counter', (SELECT concat(author_action_sequence, '/', authoritative_commit_sequence,
        '/', project_activity_position) FROM storyos.scope_counters
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'zero_authority_activities', (SELECT count(*)
        FROM storyos.domain_receipts AS receipt
        JOIN storyos.project_activity_events AS activity
          ON (activity.owner_user_id, activity.project_id, activity.receipt_id) =
             (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
        WHERE receipt.owner_user_id = '${USER_A}'::uuid
          AND receipt.project_id = '${PROJECT_A}'::uuid
          AND receipt.result_kind <> 'authoritative_applied'),
      'cross_scope_receipts', (SELECT count(*) FROM storyos.domain_receipts
        WHERE owner_user_id <> '${USER_A}'::uuid OR project_id <> '${PROJECT_A}'::uuid),
      'manuscript_body', (SELECT convert_from(payload.canonical_bytes, 'UTF8')
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
              revision.revision_id) =
             (head.owner_user_id, head.project_id, head.manuscript_object_id,
              head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
        WHERE head.owner_user_id = '${USER_A}'::uuid
          AND head.project_id = '${PROJECT_A}'::uuid
          AND head.manuscript_object_id = '018f0000-0000-7001-8000-000000000003'::uuid)
    )::text`;
  return JSON.parse(await queryPostgres(query));
}

async function connectPage(browserWebSocketUrl) {
  const { port } = new URL(browserWebSocketUrl);
  const target = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent("about:blank")}`, {
    method: "PUT", signal: AbortSignal.timeout(5_000),
  }).then((response) => response.json());
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await once(socket, "open");
  let nextId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    if (message.error) entry.reject(new Error(message.error.message));
    else entry.resolve(message.result);
  });
  const command = (method, params = {}) => new Promise((resolve, reject) => {
    nextId += 1;
    const timeout = setTimeout(() => {
      pending.delete(nextId);
      reject(new Error(`Chrome DevTools command timed out: ${method}`));
    }, 20_000);
    pending.set(nextId, {
      resolve(value) { clearTimeout(timeout); resolve(value); },
      reject(error) { clearTimeout(timeout); reject(error); },
    });
    socket.send(JSON.stringify({ id: nextId, method, params }));
  });
  await command("Runtime.enable");
  await command("Network.enable");
  await command("Page.enable");
  await command("IndexedDB.enable");
  return { socket, command };
}

async function evaluate(command, expression) {
  const result = await command("Runtime.evaluate", {
    expression, awaitPromise: true, returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails));
  }
  return result.result.value;
}

async function waitFor(command, expression, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await evaluate(command, expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  const surface = await evaluate(command, SURFACE);
  let journal = null;
  try { journal = await evaluate(command, JOURNAL); } catch (error) { journal = String(error?.message ?? error); }
  let recovery = null;
  try { recovery = await evaluate(command, RECOVERY); } catch (error) { recovery = String(error?.message ?? error); }
  throw new Error(`Browser condition timed out: ${expression}\n${JSON.stringify({ surface, journal, recovery })}`);
}

function remoteObjectHasAuthorEditUnit(entry) {
  const encoded = JSON.stringify(entry ?? {});
  return encoded.includes('"author_edit_unit"');
}

async function waitForBackendUnsettledIntent(command, origin) {
  const databaseName = `storyos-local-edit-journal:${USER_A}:${PROJECT_A}`;
  const deadline = Date.now() + 20_000;
  let last = null;
  while (Date.now() < deadline) {
    last = await command("IndexedDB.requestData", {
      securityOrigin: origin,
      databaseName,
      objectStoreName: "intents",
      indexName: "",
      skipCount: 0,
      pageSize: 50,
    });
    if ((last.objectStoreDataEntries ?? []).some(remoteObjectHasAuthorEditUnit)) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`IndexedDB backend has no unsettled intent before reload\n${JSON.stringify(last)}`);
}

const SURFACE = `({
  bootState: document.querySelector("#app")?.dataset.bootState ?? null,
  heading: document.querySelector("#app h1")?.textContent ?? null,
  chapter: document.querySelector("#app h2")?.textContent ?? null,
  body: document.querySelector("textarea")?.value ?? null,
  saveState: document.querySelector("[data-save-state]")?.dataset.saveState ?? null,
  readOnly: document.querySelector("textarea")?.readOnly ?? null,
  alert: document.querySelector('[role="alert"]') ? true : false
})`;

const JOURNAL = `new Promise((resolve, reject) => {
  const request = indexedDB.open(
    "storyos-local-edit-journal:${USER_A}:${PROJECT_A}", ${JOURNAL_DATABASE_VERSION});
  request.onerror = () => reject(request.error);
  request.onsuccess = () => {
    const db = request.result;
    const tx = db.transaction(["submission_groups", "intents", "metadata"], "readonly");
    const groups = tx.objectStore("submission_groups").getAll();
    const intents = tx.objectStore("intents").getAll();
    const metadata = tx.objectStore("metadata").getAll();
    tx.oncomplete = () => {
      db.close();
      resolve({
        groups: groups.result.map((group) => ({
          collected: group.payload_collection?.kind === "collected",
          settlement: group.settlement?.kind ?? null,
        })),
        intents: intents.result.map((record) => ({
          retainedUnit: record.author_edit_unit !== undefined,
        })),
        fenceReasons: metadata.result
          .filter((row) => String(row.key).startsWith("collection_fences:"))
          .flatMap((row) => row.value ?? [])
          .map((fence) => fence.reason),
      });
    };
    tx.onerror = () => reject(tx.error);
  };
})`;

const RECOVERY = `({
  sessionStorage: sessionStorage.getItem("active_session:${USER_A}:${PROJECT_A}")
})`;

async function focusEnd(command) {
  await evaluate(command, `{
    const editor = document.querySelector("textarea");
    editor.focus();
    editor.setSelectionRange(editor.value.length, editor.value.length);
    true;
  }`);
}

test("S1-JRN-001 runs on the Vite production page, Server HTTP, Core, and PostgreSQL", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 120_000,
}, async () => {
  assert.equal(existsSync(join(distDir, "index.html")), true, "vite production dist is missing");
  assert.ok(process.env.STORYOS_TEST_DATABASE_URL, "run through scripts/verify-project-scope.sh");
  const { baseUrl, server } = await startRealServer();
  const apiOrigin = new URL(baseUrl).origin;
  const proxy = createServer(async (request, response) => {
    try {
      const url = new URL(request.url ?? "/", "http://127.0.0.1");
      if (url.pathname.startsWith("/api/")) {
        const chunks = [];
        for await (const chunk of request) chunks.push(chunk);
        const headers = {};
        for (const name of [
          "accept", "content-type", "cookie", "idempotency-key", "x-storyos-anti-forgery",
        ]) {
          if (request.headers[name]) headers[name] = request.headers[name];
        }
        headers.origin = apiOrigin;
        const init = { method: request.method, headers };
        if (request.method !== "GET" && request.method !== "HEAD") {
          init.body = Buffer.concat(chunks);
        }
        const upstream = await fetch(new URL(request.url, apiOrigin), init);
        const body = Buffer.from(await upstream.arrayBuffer());
        const outbound = { "content-type": upstream.headers.get("content-type") ?? "application/octet-stream" };
        const cacheControl = upstream.headers.get("cache-control");
        if (cacheControl) outbound["cache-control"] = cacheControl;
        response.writeHead(upstream.status, outbound);
        response.end(body);
        return;
      }
      const filePath = distPath(url.pathname);
      if (!filePath) {
        response.writeHead(404).end();
        return;
      }
      const body = await readFile(filePath);
      response.writeHead(200, { "content-type": contentType(filePath) });
      response.end(body);
    } catch (error) {
      response.writeHead(502, { "content-type": "text/plain; charset=utf-8" });
      response.end(String(error?.message ?? error));
    }
  });
  proxy.listen(0, "127.0.0.1");
  await once(proxy, "listening");
  const pageOrigin = `http://127.0.0.1:${proxy.address().port}`;
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-s1-jrn-001-"));
  const browser = spawn(chromeExecutable, [
    "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
    "--disable-dev-shm-usage", "--no-first-run", "--remote-debugging-port=0",
    "--remote-allow-origins=*", `--user-data-dir=${profileDirectory}`, "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  let page;
  try {
    page = await connectPage(await devToolsAddress(browser));
    await page.command("Network.setCookie", {
      name: "storyos_session", value: "session-a", url: `${pageOrigin}/`,
    });
    await page.command("Browser.grantPermissions", {
      origin: pageOrigin, permissions: ["clipboardReadWrite", "clipboardSanitizedWrite"],
    });
    await page.command("Page.navigate", { url: `${pageOrigin}/projects/${PROJECT_A}` });
    await waitFor(page.command, "document.querySelector('#app')?.dataset.bootState === 'project-ready'");
    const open = await evaluate(page.command, SURFACE);
    await focusEnd(page.command);
    await page.command("Input.insertText", { text: " Hello" });
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saving'");
    const input = await evaluate(page.command, SURFACE);
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saved' && document.querySelector('textarea')?.readOnly === false");
    const afterType = await evaluate(page.command, SURFACE);
    await focusEnd(page.command);
    await page.command("Input.insertText", { text: "中文" });
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_IME)}`);
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saved' && document.querySelector('textarea')?.readOnly === false");
    const afterIme = await evaluate(page.command, SURFACE);
    await page.command("Runtime.evaluate", { expression: "navigator.clipboard.writeText(' EN')", awaitPromise: true });
    await focusEnd(page.command);
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "v", code: "KeyV", modifiers: 4, commands: ["Paste"],
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "v", code: "KeyV", modifiers: 4,
    });
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_PASTE)}`);
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saved' && document.querySelector('textarea')?.readOnly === false");
    const settle = await evaluate(page.command, SURFACE);
    const settledJournal = await evaluate(page.command, JOURNAL);
    await focusEnd(page.command);
    await page.command("Input.insertText", { text: "!" });
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_UNSETTLED)}`);
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saving'");
    const interrupt = await evaluate(page.command, SURFACE);
    const unsettledJournal = await evaluate(page.command, JOURNAL);
    await waitFor(page.command, `(${JOURNAL}).then((journal) => journal.intents.some((intent) => intent.retainedUnit))`);
    await waitForBackendUnsettledIntent(page.command, pageOrigin);
    await page.command("Page.reload", { ignoreCache: false });
    await waitFor(page.command, "document.querySelector('#app')?.dataset.bootState === 'project-ready'");
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_UNSETTLED)}`);
    const recovered = await evaluate(page.command, SURFACE);
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saved' && document.querySelector('textarea')?.readOnly === false");
    const recoveredSaved = await evaluate(page.command, SURFACE);
    const collectedJournal = await evaluate(page.command, JOURNAL);
    const authority = await projectAuthoritySnapshot();
    assert.deepEqual({
      id: "S1-JRN-001",
      open,
      input: { body: input.body, saveState: input.saveState },
      afterType: { body: afterType.body, saveState: afterType.saveState },
      afterIme: { body: afterIme.body, saveState: afterIme.saveState },
      settle: { body: settle.body, saveState: settle.saveState, journal: settledJournal },
      interrupt: { body: interrupt.body, saveState: interrupt.saveState, journal: unsettledJournal },
      recover: {
        body: recovered.body, saveState: recovered.saveState,
        saved: recoveredSaved, journal: collectedJournal,
      },
      authority,
    }, {
      id: "S1-JRN-001",
      open: {
        bootState: "project-ready", heading: "Project A", chapter: "Chapter A",
        body: OPEN_BODY, saveState: "clean", readOnly: false, alert: false,
      },
      input: { body: AFTER_TYPE, saveState: "saving" },
      afterType: { body: AFTER_TYPE, saveState: "saved" },
      afterIme: { body: AFTER_IME, saveState: "saved" },
      settle: {
        body: AFTER_PASTE, saveState: "saved",
        journal: {
          groups: [
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
          ],
          intents: [
            { retainedUnit: false }, { retainedUnit: false }, { retainedUnit: false },
          ],
          fenceReasons: [
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
          ],
        },
      },
      interrupt: {
        body: AFTER_UNSETTLED, saveState: "saving",
        journal: {
          groups: [
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
          ],
          intents: [
            { retainedUnit: false }, { retainedUnit: false }, { retainedUnit: false },
            { retainedUnit: true },
          ],
          fenceReasons: [
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
          ],
        },
      },
      recover: {
        body: AFTER_UNSETTLED, saveState: "saving",
        saved: {
          bootState: "project-ready", heading: "Project A", chapter: "Chapter A",
          body: AFTER_UNSETTLED, saveState: "saved", readOnly: false, alert: false,
        },
        journal: {
          groups: [
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
            { collected: true, settlement: "applied_receipt_settled" },
          ],
          intents: [
            { retainedUnit: false }, { retainedUnit: false }, { retainedUnit: false },
            { retainedUnit: false },
          ],
          fenceReasons: [
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
            "applied_receipt_converged_with_durable_successor",
          ],
        },
      },
      authority: {
        receipts: 4, admissions: 4, consumed_challenges: 4, activities: 4,
        revision_envelopes: 4, commits: 4, actions: 4, counter: "4/4/4",
        zero_authority_activities: 0, cross_scope_receipts: 0,
        manuscript_body: AFTER_UNSETTLED,
      },
    });
  } finally {
    page?.socket.close();
    proxy.close();
    await stopProcess(browser);
    await stopProcess(server);
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});

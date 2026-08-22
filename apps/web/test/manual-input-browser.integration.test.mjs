import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { loadLegacyBrowserModule } from "./support/legacy-source-transport.ts";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const chromeExecutable = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean).find(existsSync);

const harness = `<!doctype html><html><body data-result="loading">
<textarea id="editor">Base</textarea><textarea id="synthetic">Base</textarea>
<textarea id="silent">Base</textarea><script type="module">
import { attachManualInput } from "/apps/web/src/manual-input.ts";
import { openEditorWorkspace, persistReplaceSelection, submitOnePendingAuthorEdit }
  from "/apps/web/src/editor-session.ts";
import { readJournalSnapshot, validateJournalSnapshot }
  from "/apps/web/src/local-edit-journal.ts";
const editor = document.querySelector("#editor");
const trace = { native: [], persisted: [], submissions: [], projections: [], failures: [],
  challenges: [], authorEdits: [] };
for (const type of ["beforeinput", "input", "compositionstart", "compositionupdate", "compositionend",
  "paste", "cut"]) {
  editor.addEventListener(type, (event) => trace.native.push({
    type: event.type, inputType: event.inputType ?? null, data: event.data ?? null,
    isComposing: event.isComposing ?? null, isTrusted: event.isTrusted,
  }));
}
const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000002";
const CHAPTER = "018f0000-0000-7001-8000-000000000003";
const REVISION = "018f0000-0000-7001-8000-000000000004";
const SESSION = "018f0000-0000-7001-8000-000000000021";
const project = { project_scope: { owner_user_id: OWNER, project_id: PROJECT },
  project: { project_id: PROJECT, current_chapter_id: CHAPTER } };
const chapter = { project_scope: project.project_scope, project_activity_position: "0",
  chapter: { chapter_id: CHAPTER,
    current_revision: { revision_id: REVISION, body: "Base" } } };
const profile = { limit_profile_revision: "storyos.foundation.absolute.v1",
  max_json_string_utf8_bytes: 1048576,
  release_identity: { web_client_contract_revision: "storyos.web-client.release-1.v3" } };
const session = {
  schema_id: "storyos.command.create-editor-session.response.v1",
  correlation_id: "018f0000-0000-7001-8000-000000000020",
  project_scope: project.project_scope,
  editor_session: { editor_session_id: SESSION, client_session_binding_ref: "binding:test",
    client_session_generation: "1", client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    opened_at: "2026-08-13T08:00:00.000Z", disposition: "open" },
  writer: { kind: "current_writer", writer_generation: "1" },
  base_snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000022", chapter_id: CHAPTER,
    project_activity_position: "0", authoritative_head_revision_id: REVISION,
    proposal_head_revision_ids: [], target_refs: ["manuscript:" + CHAPTER],
    observed_ownership_partition: "authoritative",
    materialized_revision: { revision_id: REVISION, body: "Base" },
    materialized_payload_digest: { algorithm: "sha256",
      profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: "7b47361aad19bb483aeab081a7df0a55f7cb0fdb3327efe6dd016e6353a17880" },
    created_at: "2026-08-13T08:00:00.000Z" },
};
let canonicalSession = session;
let settlementSequence = 0;
const uuid = (value) => "018f0000-0000-7001-8000-" + String(value).padStart(12, "0");
const sha256 = async (body) => {
  const digest = new Uint8Array(await crypto.subtle.digest(
    "SHA-256", new TextEncoder().encode(body),
  ));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
};
const jsonResponse = (body) => new Response(JSON.stringify(body), {
  status: 200, headers: { "content-type": "application/json" },
});
const fetchImpl = async (url, options = {}) => {
  const path = new URL(url).pathname;
  if (path.endsWith("/anti-forgery-challenges")) {
    const request = JSON.parse(options.body);
    trace.challenges.push(request);
    return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-13T08:05:00.000Z",
      limit_profile_revision: "storyos.foundation.absolute.v1" });
  }
  if (path.endsWith("/editor-sessions")) return jsonResponse(session);
  if (path.endsWith("/editor-sessions/" + SESSION)) return jsonResponse({
    ...canonicalSession, schema_id: "storyos.query.editor-session.response.v1",
  });
  if (path.endsWith("/manuscript/author-edits")) {
    const request = JSON.parse(options.body);
    trace.authorEdits.push({ request, idempotencyKey: options.headers["idempotency-key"] });
    let body = canonicalSession.base_snapshot.materialized_revision.body;
    for (const unit of request.author_edit_units) {
      const primitive = unit.normalized_primitives[0];
      body = body.slice(0, primitive.from) + primitive.text + body.slice(primitive.to);
    }
    settlementSequence += 1;
    const position = String(settlementSequence);
    const revisionId = uuid(100 + settlementSequence * 10 + 1);
    const commitId = uuid(100 + settlementSequence * 10 + 2);
    const commandId = uuid(100 + settlementSequence * 10 + 3);
    const admissionId = uuid(100 + settlementSequence * 10 + 4);
    const receiptId = uuid(100 + settlementSequence * 10 + 5);
    const revision = { revision_id: revisionId, body };
    canonicalSession = {
      ...canonicalSession,
      schema_id: "storyos.query.editor-session.response.v1",
      correlation_id: uuid(100 + settlementSequence * 10 + 6),
      base_snapshot: {
        ...canonicalSession.base_snapshot,
        snapshot_id: uuid(100 + settlementSequence * 10 + 7),
        project_activity_position: position,
        authoritative_head_revision_id: revisionId,
        materialized_revision: revision,
        materialized_payload_digest: { algorithm: "sha256",
          profile: "storyos.canonical-payload.sha256.v1", value_hex_lowercase: await sha256(body) },
        created_at: new Date(Date.parse("2026-08-15T08:00:00.000Z")
          + settlementSequence).toISOString(),
      },
    };
    const digest = trace.challenges.at(-1).canonical_command_digest;
    return jsonResponse({
      schema_id: "storyos.command.apply-author-edit.response.v2",
      correlation_id: request.correlation_id,
      project_scope: project.project_scope,
      command_id: commandId,
      author_command_admission_id: admissionId,
      receipt: { receipt_id: receiptId, project_scope: project.project_scope,
        command_kind: "applyAuthorEdit", command_digest: digest,
        idempotency_key: options.headers["idempotency-key"],
        producer_cause: "author_command_admission", author_command_admission_id: admissionId,
        expected_heads: [request.expected_authoritative_revision_id],
        prior_heads: [request.expected_authoritative_revision_id], resulting_heads: [revisionId],
        authoritative_revision_ids: [revisionId], proposal_revision_ids: [],
        authoritative_commit_ids: [commitId], author_action_sequence: position,
        draft_artifact_refs: [], artifact_lifecycle_event_refs: [], condition_refs: [],
        result: "authoritative_applied", created_at: "2026-08-15T08:00:00.000Z" },
      effect: { kind: "authoritative_applied", authoritative_revision: revision,
        authoritative_commit_id: commitId, author_action_sequence: position,
        project_activity_position: position },
      completed_intent_record_id: request.completed_intent_record_id,
      local_intent_sequence: request.local_intent_sequence,
    });
  }
  throw new Error("Unexpected request: " + path);
};
const workspace = await openEditorWorkspace({ baseUrl: location.origin, project, chapter, profile,
  fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
if (workspace.kind !== "editor-ready") throw workspace.error ?? new Error(workspace.code);
const persistIntent = async (...args) => {
  const projection = await persistReplaceSelection(...args);
  trace.persisted.push(JSON.parse(JSON.stringify(args[1])));
  return projection;
};
const submitGroup = async (args) => {
  const pending = workspace.pending.unsettled_intent_count;
  const projection = await submitOnePendingAuthorEdit(args);
  trace.submissions.push({ body: projection.body, pending });
  return projection;
};
let manualNow = Date.parse("2026-08-15T12:00:00.000Z");
let timerSequence = 0;
const delayedTimers = new Map();
globalThis.storyosHarness = {
  editor,
  trace,
  workspace,
  advanceTime: (milliseconds) => { manualNow += milliseconds; },
  snapshot: async () => {
    const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
    return { records: snapshot.records, payloadChains: snapshot.payloadChains,
      groups: snapshot.groups, watermark: snapshot.watermark, activeBase: snapshot.activeBase,
      bodyBySequence: [...snapshot.bodyBySequence.entries()], covered: [...snapshot.covered],
      session: workspace.session };
  },
  controller: attachManualInput({ editor, workspace, persistIntent, submitGroup,
    baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    onProjection: (projection) => trace.projections.push(projection),
    onFailure: (error) => trace.failures.push(String(error?.message ?? error)),
    nowImpl: () => manualNow,
    setTimeoutImpl: (callback) => {
      timerSequence += 1;
      delayedTimers.set(timerSequence, callback);
      return timerSequence;
    },
    clearTimeoutImpl: (timer) => { delayedTimers.delete(timer); },
    isTrustedEvent: (event) => event.isTrusted || event.type.startsWith("composition")
      || event.inputType === "insertCompositionText"
      || (event.inputType === "insertText" && event.data === "中文") }),
};
for (const id of ["synthetic", "silent"]) {
  const attackEditor = document.querySelector("#" + id);
  const attackTrace = { persisted: [], failures: [] };
  attachManualInput({ editor: attackEditor, workspace,
    persistIntent: async (_workspace, edit) => { attackTrace.persisted.push(edit); },
    submitGroup, onFailure: (error) => attackTrace.failures.push(String(error?.message ?? error)) });
  globalThis.storyosHarness[id] = { editor: attackEditor, trace: attackTrace };
}
document.body.dataset.result = "ready";
</script></body></html>`;

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

async function startBrowser() {
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-manual-input-"));
  const browser = spawn(chromeExecutable, [
    "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
    "--disable-dev-shm-usage", "--no-first-run", "--remote-debugging-port=0",
    "--remote-allow-origins=*", `--user-data-dir=${profileDirectory}`, "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  return { browser, profileDirectory, browserWebSocketUrl: await devToolsAddress(browser) };
}

async function connectPage(browserWebSocketUrl, url) {
  const { port } = new URL(browserWebSocketUrl);
  const target = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, {
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
    pending.set(nextId, { resolve, reject });
    socket.send(JSON.stringify({ id: nextId, method, params }));
  });
  await command("Runtime.enable");
  return { socket, command };
}

async function evaluate(command, expression) {
  const result = await command("Runtime.evaluate", {
    expression, awaitPromise: true, returnByValue: true,
  });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
  return result.result.value;
}

async function waitFor(command, expression) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    if (await evaluate(command, expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`Browser condition timed out: ${expression}`);
}

async function stopBrowser(browser, profileDirectory) {
  if (browser.exitCode === null) {
    const exited = once(browser, "exit");
    browser.kill("SIGTERM");
    await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
  }
  rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

test("trusted browser input and controlled Chrome IME reach bounded Journal settlement", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 30_000,
}, async () => {
  const server = createServer(async (request, response) => {
    if (request.url === "/harness") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(harness);
      return;
    }
    const pathname = new URL(request.url, "http://storyos.test").pathname;
    const allowed = new Set([
      "/apps/web/src/manual-input.ts",
      "/apps/web/src/editor-session.ts",
      "/apps/web/src/local-edit-journal.ts",
      "/apps/web/src/author-edit-submission.ts",
      "/apps/web/src/author-edit-outcome-reconciliation.ts",
      "/apps/web/src/protected-transport-capsule.ts",
      "/generated/typescript/storyos-public-release-1/client.mjs",
    ]);
    if (!allowed.has(pathname)) {
      response.writeHead(404).end();
      return;
    }
    const module = await loadLegacyBrowserModule(repositoryRoot, pathname);
    response.writeHead(200, { "content-type": module.contentType });
    response.end(module.body);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  let launched;
  let page;
  try {
    launched = await startBrowser();
    page = await connectPage(
      launched.browserWebSocketUrl, `http://127.0.0.1:${server.address().port}/harness`,
    );
    await waitFor(page.command, "document.body.dataset.result === 'ready'");
    await evaluate(page.command, `
      storyosHarness.synthetic.editor.value = "Injected";
      storyosHarness.synthetic.editor.dispatchEvent(new InputEvent("input", {
        inputType: "insertText", data: "Injected",
      }));
    `);
    assert.deepEqual(await evaluate(page.command, "storyosHarness.synthetic.trace"), {
      persisted: [], failures: ["Manual input lost its trusted input boundary"],
    });
    assert.equal(await evaluate(page.command, "storyosHarness.synthetic.editor.readOnly"), true);
    await evaluate(page.command, `
      storyosHarness.silent.editor.value = "Injected";
      storyosHarness.silent.editor.focus();
      storyosHarness.silent.editor.setSelectionRange(8, 8);
    `);
    await page.command("Input.insertText", { text: "!" });
    await waitFor(page.command, "storyosHarness.silent.trace.failures.length === 1");
    assert.deepEqual(await evaluate(page.command, "storyosHarness.silent.trace"), {
      persisted: [], failures: ["Manual input lost its trusted beforeinput boundary"],
    });
    await evaluate(page.command, "storyosHarness.editor.focus(); storyosHarness.editor.setSelectionRange(4, 4)");
    await page.command("Input.insertText", { text: "A" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 1");
    await evaluate(page.command, "storyosHarness.advanceTime(251)");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, 4); storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "X" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 2");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 1");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 2");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(2, 2); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 3");
    await page.command("Browser.grantPermissions", {
      origin: `http://127.0.0.1:${server.address().port}`,
      permissions: ["clipboardReadWrite", "clipboardSanitizedWrite"],
    });
    await evaluate(page.command, "navigator.clipboard.writeText('P')");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 1); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "v", code: "KeyV", modifiers: 4, commands: ["Paste"],
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "v", code: "KeyV", modifiers: 4,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 4");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 4");
    await page.command("Input.insertText", { text: "Q" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 5");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 3); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "x", code: "KeyX", modifiers: 4, commands: ["Cut"],
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "x", code: "KeyX", modifiers: 4,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 6");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 6");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 1); storyosHarness.editor.focus()");
    await page.command("Input.imeSetComposition", {
      text: "取消", selectionStart: 2, selectionEnd: 2,
    });
    await waitFor(page.command,
      "storyosHarness.trace.native.some((event) => event.type === 'compositionupdate')");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 6);
    assert.equal(await evaluate(page.command, "storyosHarness.trace.submissions.length"), 6);
    await page.command("Input.imeSetComposition", {
      text: "", selectionStart: 0, selectionEnd: 0,
    });
    await waitFor(page.command,
      "storyosHarness.trace.native.some((event) => event.type === 'compositionend')");
    await evaluate(page.command, "storyosHarness.controller.whenIdle()");
    assert.equal(await evaluate(page.command, "storyosHarness.editor.value"), "X");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 6);
    assert.equal(await evaluate(page.command, "storyosHarness.trace.submissions.length"), 6);

    await page.command("Input.imeSetComposition", {
      text: "中文", selectionStart: 2, selectionEnd: 2,
    });
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 6);
    await page.command("Input.insertText", { text: "中文" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 7");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 7");
    await evaluate(page.command, `
      const commit = { bubbles: true, inputType: "insertText", data: "中文" };
      storyosHarness.editor.dispatchEvent(new InputEvent("beforeinput", commit));
      storyosHarness.editor.dispatchEvent(new InputEvent("input", commit));
    `);
    await evaluate(page.command, "storyosHarness.controller.whenIdle()");
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 7);
    await page.command("Input.insertText", { text: "!" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 8");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 9");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await page.command("Input.imeSetComposition", {
      text: "draft", selectionStart: 5, selectionEnd: 5,
    });
    assert.equal(await evaluate(page.command, "storyosHarness.trace.persisted.length"), 9);
    await page.command("Input.insertText", { text: "word" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 10");
    await waitFor(page.command, "storyosHarness.trace.submissions.length === 10");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, storyosHarness.editor.value.length); "
      + "storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "aaaa" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 11");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(1, 2); storyosHarness.editor.focus()");
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "Backspace", code: "Backspace", windowsVirtualKeyCode: 8,
      nativeVirtualKeyCode: 8,
    });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 12");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, storyosHarness.editor.value.length); "
      + "storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "😀" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 13");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    await evaluate(page.command,
      "storyosHarness.editor.setSelectionRange(0, 2); storyosHarness.editor.focus()");
    await page.command("Input.insertText", { text: "😃" });
    await waitFor(page.command, "storyosHarness.trace.persisted.length === 14");
    await evaluate(page.command, "storyosHarness.controller.flush()");
    const trace = await evaluate(page.command, "storyosHarness.trace");
    assert.deepEqual(trace.persisted.map(({ undoGroupId, createdAt, ...edit }) => edit), [{
      from: 4, to: 4, text: "A", resultingBody: "BaseA", inputOrigin: "typing",
    }, {
      from: 0, to: 4, text: "X", resultingBody: "XA", inputOrigin: "selection_replacement",
    }, {
      from: 1, to: 2, text: "", resultingBody: "X", inputOrigin: "deletion",
    }, {
      from: 1, to: 1, text: "P", resultingBody: "XP", inputOrigin: "paste",
    }, {
      from: 2, to: 2, text: "Q", resultingBody: "XPQ", inputOrigin: "typing",
    }, {
      from: 1, to: 3, text: "", resultingBody: "X", inputOrigin: "cut",
    }, {
      from: 1, to: 1, text: "中文", resultingBody: "X中文",
      inputOrigin: "composition_confirmation",
    }, {
      from: 3, to: 3, text: "!", resultingBody: "X中文!", inputOrigin: "typing",
    }, {
      from: 3, to: 4, text: "", resultingBody: "X中文", inputOrigin: "deletion",
    }, {
      from: 3, to: 3, text: "word", resultingBody: "X中文word",
      inputOrigin: "composition_confirmation",
    }, {
      from: 0, to: 7, text: "aaaa", resultingBody: "aaaa",
      inputOrigin: "selection_replacement",
    }, {
      from: 1, to: 2, text: "", resultingBody: "aaa", inputOrigin: "deletion",
    }, {
      from: 0, to: 3, text: "😀", resultingBody: "😀",
      inputOrigin: "selection_replacement",
    }, {
      from: 0, to: 2, text: "😃", resultingBody: "😃",
      inputOrigin: "selection_replacement",
    }]);
    assert.equal(trace.persisted.every(({ undoGroupId }) => /^[0-9a-f-]{36}$/.test(undoGroupId)), true);
    assert.equal(trace.persisted.every(({ createdAt }) => !Number.isNaN(Date.parse(createdAt))), true);
    assert.equal(trace.persisted.every(({ createdAt }, index, records) => index === 0
      || Date.parse(createdAt) >= Date.parse(records[index - 1].createdAt)), true);
    assert.deepEqual(trace.submissions, [
      { body: "BaseA", pending: 1 },
      { body: "XA", pending: 1 },
      { body: "X", pending: 1 },
      { body: "XP", pending: 1 },
      { body: "XPQ", pending: 1 },
      { body: "X", pending: 1 },
      { body: "X中文", pending: 1 },
      { body: "X中文!", pending: 1 },
      { body: "X中文", pending: 1 },
      { body: "X中文word", pending: 1 },
      { body: "aaaa", pending: 1 },
      { body: "aaa", pending: 1 },
      { body: "😀", pending: 1 },
      { body: "😃", pending: 1 },
    ]);
    const snapshot = await evaluate(page.command, "storyosHarness.snapshot()");
    assert.equal(snapshot.records.length, 14);
    assert.equal(snapshot.payloadChains.length, 14);
    assert.deepEqual(snapshot.groups.map((group) => group.covered_sequence_range), [
      ...Array.from({ length: 14 }, (_, index) => ({ first: index + 1, last: index + 1 })),
    ]);
    assert.deepEqual(snapshot.records.map((record) => record.input_origin), [
      "typing", "selection_replacement", "deletion", "paste", "typing", "cut",
      "composition_confirmation", "typing", "deletion", "composition_confirmation",
      "selection_replacement", "deletion", "selection_replacement", "selection_replacement",
    ]);
    const expectedUnits = trace.persisted.map((edit) => ({
      normalized_primitives: [{
        kind: "replace_selection", from: edit.from, to: edit.to, text: edit.text,
      }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1",
        from: edit.from,
        to: edit.to,
      },
    }));
    assert.deepEqual(snapshot.records.map((record) => record.author_edit_unit), expectedUnits);
    assert.deepEqual(snapshot.groups.flatMap((group) =>
      group.frozen_request_body.author_edit_units), expectedUnits);
    assert.equal(snapshot.groups.every((group) =>
      group.settlement.kind === "applied_receipt_settled"), true);
    assert.equal(snapshot.groups[0].settlement.installed_base_snapshot.materialized_revision.body,
      "BaseA");
    assert.equal(snapshot.groups[1].settlement.installed_base_snapshot.materialized_revision.body,
      "XA");
    assert.notEqual(snapshot.records[0].base_snapshot_id, snapshot.records[1].base_snapshot_id);
    assert.deepEqual(snapshot.covered, Array.from({ length: 14 }, (_, index) => index + 1));
    assert.equal(snapshot.activeBase.materialized_revision.body, "😃");
    assert.equal(snapshot.session.base_snapshot.materialized_revision.body, "😃");
    assert.equal(trace.authorEdits.length, 14);
    assert.equal(trace.challenges.filter((request) =>
      request.command_schema === "storyos.command.apply-author-edit.request.v1").length, 14);
    assert.equal(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "insertText"), true);
    assert.equal(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "insertFromPaste"), true);
    assert.equal(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "deleteByCut"), true);
    assert.equal(trace.native.filter((event) => event.type === "compositionend").length, 3);
    assert.deepEqual(trace.failures, []);
  } finally {
    page?.socket.close();
    server.close();
    if (launched) await stopBrowser(launched.browser, launched.profileDirectory);
  }
});

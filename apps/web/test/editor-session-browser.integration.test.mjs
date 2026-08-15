import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const chromeCandidates = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean);
const chromeExecutable = chromeCandidates.find(existsSync);

const harness = `<!doctype html>
<html><body data-result="running"><script type="module">
import { openEditorWorkspace, persistReplaceSelection, rebuildPendingProjection }
  from "/apps/web/src/editor-session.mjs";

const assert = {
  equal(actual, expected) {
    if (actual !== expected) throw new Error("Expected " + JSON.stringify(expected)
      + " but received " + JSON.stringify(actual));
  },
  deepEqual(actual, expected) {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error("Expected "
      + JSON.stringify(expected) + " but received " + JSON.stringify(actual));
  },
  async rejects(action, pattern) {
    try { await action; } catch (error) {
      if (pattern.test(String(error?.message ?? error))) return;
      throw error;
    }
    throw new Error("Expected the operation to reject");
  },
};

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000002";
const CHAPTER = "018f0000-0000-7001-8000-000000000003";
const REVISION = "018f0000-0000-7001-8000-000000000004";
const SESSION = "018f0000-0000-7001-8000-000000000021";
const project = { project_scope: { owner_user_id: OWNER, project_id: PROJECT },
  project: { project_id: PROJECT, current_chapter_id: CHAPTER } };
const chapter = { project_scope: project.project_scope,
  project_activity_position: "0",
  chapter: { chapter_id: CHAPTER, current_revision: { revision_id: REVISION, body: "Base" } } };
const profile = { limit_profile_revision: "storyos.foundation.absolute.v1",
  max_json_string_utf8_bytes: 1048576,
  release_identity: { web_client_contract_revision: "storyos.web-client.release-1.v1" } };
const session = {
  schema_id: "storyos.command.create-editor-session.response.v1",
  correlation_id: "018f0000-0000-7001-8000-000000000020",
  project_scope: project.project_scope,
  editor_session: { editor_session_id: SESSION, client_session_binding_ref: "binding:test",
    client_session_generation: "1", client_contract_revision: "storyos.web-client.release-1.v1",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    opened_at: "2026-08-13T08:00:00.000Z", disposition: "open" },
  writer: { kind: "current_writer", writer_generation: "1" },
  base_snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000022", chapter_id: CHAPTER,
    project_activity_position: "0", authoritative_head_revision_id: REVISION,
    proposal_head_revision_ids: [], target_refs: ["manuscript:" + CHAPTER],
    observed_ownership_partition: "authoritative",
    materialized_revision: { revision_id: REVISION, body: "Base" },
    materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: "7b47361aad19bb483aeab081a7df0a55f7cb0fdb3327efe6dd016e6353a17880" },
    created_at: "2026-08-13T08:00:00.000Z" }
};
const requestPaths = [];
const jsonResponse = (body) => new Response(JSON.stringify(body), {
  status: 200, headers: { "content-type": "application/json" },
});
const fetchImpl = async (url) => {
  const path = new URL(url).pathname;
  requestPaths.push(path);
  if (path.endsWith("/anti-forgery-challenges")) return jsonResponse({
    nonce: "a".repeat(64), expires_at: "2026-08-13T08:05:00.000Z",
    limit_profile_revision: "storyos.foundation.absolute.v1",
  });
  if (path.endsWith("/editor-sessions")) return jsonResponse(session);
  if (path.endsWith("/editor-sessions/" + SESSION)) return jsonResponse({
    ...session, schema_id: "storyos.query.editor-session.response.v1",
  });
  throw new Error("Unexpected request: " + path);
};
const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
});
const transactionResult = (transaction) => new Promise((resolve, reject) => {
  transaction.oncomplete = resolve;
  transaction.onabort = () => reject(transaction.error);
  transaction.onerror = () => reject(transaction.error);
});
const intentCount = (database) => requestResult(database.transaction("intents", "readonly")
  .objectStore("intents").count());

async function run() {
  const workspace = await openEditorWorkspace({ baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  assert.equal(workspace.kind, "editor-ready");
  if (!sessionStorage.getItem("storyos-browser-reload")) {
    const partitionTransaction = workspace.database.transaction("partitions", "readwrite");
    partitionTransaction.objectStore("partitions").delete(workspace.partition.journal_partition_id);
    await transactionResult(partitionTransaction);
    await assert.rejects(persistReplaceSelection(workspace, {
      from: 4, to: 4, text: "!", resultingBody: "Base!",
    }), /partition is incompatible/);
    assert.equal(await intentCount(workspace.database), 0);
    assert.equal((await rebuildPendingProjection(workspace)).save_state, "clean");

    workspace.database.close();
    const reopened = await openEditorWorkspace({ baseUrl: location.origin, project, chapter, profile,
      fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    assert.equal(reopened.kind, "editor-ready");

    const pending = await persistReplaceSelection(reopened, {
      from: 4, to: 4, text: "!", resultingBody: "Base!",
    });
    assert.deepEqual(pending, { body: "Base!", save_state: "saving",
      unsettled_intent_count: 1, authoritative_revision_id: REVISION });
    assert.equal(await intentCount(reopened.database), 1);
    assert.equal(requestPaths.some((path) => path.includes("author-edits")), false);
    reopened.database.close();
    sessionStorage.setItem("storyos-browser-reload", "pending");
    location.reload();
    return;
  }

  assert.equal(workspace.session.editor_session.editor_session_id, SESSION);
  assert.deepEqual(workspace.pending, { body: "Base!", save_state: "saving",
    unsettled_intent_count: 1, authoritative_revision_id: REVISION });
  assert.equal(await intentCount(workspace.database), 1);
  assert.deepEqual(requestPaths, ["/api/v1/projects/" + PROJECT + "/editor-sessions/" + SESSION]);

  await assert.rejects(persistReplaceSelection(workspace, {
    from: 5, to: 5, text: "?", resultingBody: "x".repeat(1048577),
  }), /limit failed/);
  assert.equal(await intentCount(workspace.database), 1);

  workspace.partition.disposition = "read_only_observer";
  await assert.rejects(persistReplaceSelection(workspace, {
    from: 5, to: 5, text: "?", resultingBody: "Base!?",
  }), /read only/);
  workspace.partition.disposition = "current_writer_open";
  assert.equal(await intentCount(workspace.database), 1);

  const corruptTransaction = workspace.database.transaction(["intents", "payload_chains"], "readwrite");
  const intents = corruptTransaction.objectStore("intents");
  const key = [workspace.partition.journal_partition_id, 1];
  const record = await requestResult(intents.get(key));
  assert.equal(record.payload_digest.profile, "storyos.local-edit-journal.payload.sha256.v1");
  const payloadChain = await requestResult(corruptTransaction.objectStore("payload_chains")
    .get(record.payload_chain_ref));
  payloadChain.ordered_patch_refs[0].resulting_payload_digest.value_hex_lowercase = "0".repeat(64);
  corruptTransaction.objectStore("payload_chains").put(payloadChain);
  await transactionResult(corruptTransaction);
  await assert.rejects(rebuildPendingProjection(workspace), /corrupt/);

  const schemaTransaction = workspace.database.transaction("metadata", "readwrite");
  schemaTransaction.objectStore("metadata").put({ key: "schema", version: 999 });
  await transactionResult(schemaTransaction);
  workspace.database.close();
  const recovery = await openEditorWorkspace({ baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  assert.equal(recovery.kind, "editor-read-only-recovery");
  document.body.dataset.result = "pass";
  document.body.textContent = "real IndexedDB reload passed";
}
run().catch((error) => {
  document.body.dataset.result = "fail";
  document.body.textContent = error?.stack ?? String(error);
});
</script></body></html>`;

function contentType(pathname) {
  return extname(pathname) === ".mjs" ? "text/javascript; charset=utf-8" : "text/plain";
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
      reject(new Error(
        `Chrome exited before DevTools started (${code ?? signal ?? "unknown"}): ${stderr}`,
      ));
    });
  });
}

async function waitForHarness(browserWebSocketUrl, harnessUrl) {
  const { port } = new URL(browserWebSocketUrl);
  const target = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(harnessUrl)}`, {
    method: "PUT",
  }).then((response) => response.json());
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let commandId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    const command = pending.get(message.id);
    if (!command) return;
    pending.delete(message.id);
    if (message.error) command.reject(new Error(message.error.message));
    else command.resolve(message.result);
  });
  const command = (method, params = {}) => new Promise((resolve, reject) => {
    commandId += 1;
    pending.set(commandId, { resolve, reject });
    socket.send(JSON.stringify({ id: commandId, method, params }));
  });
  await command("Runtime.enable");
  const deadline = Date.now() + 20_000;
  try {
    while (Date.now() < deadline) {
      const evaluation = await command("Runtime.evaluate", {
        expression: "({ result: document.body?.dataset.result, text: document.body?.textContent })",
        returnByValue: true,
      });
      const state = evaluation.result.value;
      if (state?.result === "pass") return state;
      if (state?.result === "fail") throw new Error(state.text);
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error("Real-browser IndexedDB harness timed out");
  } finally {
    socket.close();
  }
}

async function terminateBrowser(browser) {
  if (browser.exitCode !== null) return;
  const exited = once(browser, "exit");
  browser.kill("SIGTERM");
  await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
  if (browser.exitCode === null) {
    browser.kill("SIGKILL");
    await Promise.race([exited, new Promise((resolve) => setTimeout(resolve, 2_000))]);
  }
}

async function startBrowser() {
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-editor-browser-"));
    const browser = spawn(chromeExecutable, [
      "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
      "--disable-dev-shm-usage", "--no-first-run", "--remote-debugging-port=0",
      "--remote-allow-origins=*", `--user-data-dir=${profileDirectory}`, "about:blank",
    ], { stdio: ["ignore", "ignore", "pipe"] });
    try {
      const browserWebSocketUrl = await devToolsAddress(browser);
      return { browser, browserWebSocketUrl, profileDirectory };
    } catch (error) {
      lastError = error;
      await terminateBrowser(browser);
      rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    }
  }
  throw lastError;
}

test("a real browser reload rebuilds one durable pending intent from IndexedDB", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 60_000,
}, async () => {
  const server = createServer(async (request, response) => {
    if (request.url === "/harness") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      response.end(harness);
      return;
    }
    const pathname = new URL(request.url, "http://storyos.test").pathname;
    const allowed = pathname === "/apps/web/src/editor-session.mjs"
      || pathname === "/apps/web/src/author-edit-submission.mjs"
      || pathname === "/generated/typescript/storyos-public-release-1/client.mjs";
    if (!allowed) {
      response.writeHead(404).end();
      return;
    }
    const bytes = await import("node:fs/promises").then(({ readFile }) =>
      readFile(join(repositoryRoot, pathname)));
    response.writeHead(200, { "content-type": contentType(pathname) });
    response.end(bytes);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = server.address().port;
  let launchedBrowser;
  try {
    launchedBrowser = await startBrowser();
    const state = await waitForHarness(
      launchedBrowser.browserWebSocketUrl,
      `http://127.0.0.1:${port}/harness`,
    );
    assert.equal(state.text, "real IndexedDB reload passed");
  } finally {
    server.close();
    if (launchedBrowser) {
      await terminateBrowser(launchedBrowser.browser);
      rmSync(launchedBrowser.profileDirectory, {
        recursive: true, force: true, maxRetries: 10, retryDelay: 100,
      });
    }
  }
});

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
const chromeExecutable = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean).find(existsSync);

const harness = `<!doctype html>
<html><body data-result="running"><script type="module">
import { openEditorWorkspace, persistReplaceSelection, submitOnePendingAuthorEdit }
  from "/apps/web/src/editor-session.mjs";
import { readJournalSnapshot, validateJournalSnapshot }
  from "/apps/web/src/local-edit-journal.mjs";

const fail = (message) => { throw new Error(message); };
const equal = (actual, expected, label) => {
  if (actual !== expected) fail(label + ": expected " + JSON.stringify(expected)
    + " but received " + JSON.stringify(actual));
};
const deepEqual = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail(label + ": expected "
    + JSON.stringify(expected) + " but received " + JSON.stringify(actual));
};
const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
});

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000002";
const CHAPTER = "018f0000-0000-7001-8000-000000000003";
const REVISION = "018f0000-0000-7001-8000-000000000004";
const SESSION = "018f0000-0000-7001-8000-000000000021";
const NONCE = "a".repeat(64);
const EXPIRES = "2026-08-13T08:05:00.000Z";
const COMMAND = "018f0000-0000-7001-8000-000000000031";
const ADMISSION = "018f0000-0000-7001-8000-000000000032";
const RECEIPT = "018f0000-0000-7001-8000-000000000033";
const NEXT_REVISION = "018f0000-0000-7001-8000-000000000034";
const NEXT_COMMIT = "018f0000-0000-7001-8000-000000000035";
const journalName = "storyos-local-edit-journal:" + OWNER + ":" + PROJECT;
const project = { project_scope: { owner_user_id: OWNER, project_id: PROJECT },
  project: { project_id: PROJECT, current_chapter_id: CHAPTER } };
const chapter = { project_scope: project.project_scope, project_activity_position: "0",
  chapter: { chapter_id: CHAPTER, current_revision: { revision_id: REVISION, body: "Base" } } };
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
    materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: "7b47361aad19bb483aeab081a7df0a55f7cb0fdb3327efe6dd016e6353a17880" },
    created_at: "2026-08-13T08:00:00.000Z" },
};
const freshSession = {
  ...session,
  schema_id: "storyos.query.editor-session.response.v1",
  correlation_id: "018f0000-0000-7001-8000-000000000037",
  base_snapshot: {
    ...session.base_snapshot,
    snapshot_id: "018f0000-0000-7001-8000-000000000038",
    project_activity_position: "1",
    authoritative_head_revision_id: NEXT_REVISION,
    materialized_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
    materialized_payload_digest: {
      algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
    },
    created_at: "2026-08-15T08:00:00.000Z",
  },
};

const jsonResponse = (body) => new Response(JSON.stringify(body), {
  status: 200, headers: { "content-type": "application/json" },
});

let outcomeMode = "unavailable";
let commandDigest;
let lastEdit;
let lastIdempotencyKey;
let canonicalSession = session;
const counts = { authorEdits: 0, outcomes: 0 };

const fetchImpl = async (url, options = {}) => {
  const parsed = new URL(url);
  if (parsed.href.includes(NONCE)) fail("nonce entered the URL");
  const path = parsed.pathname;
  if (path.endsWith("/anti-forgery-challenges")) {
    const request = JSON.parse(options.body);
    if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
      commandDigest = request.canonical_command_digest;
    }
    return jsonResponse({ nonce: NONCE, expires_at: EXPIRES,
      limit_profile_revision: "storyos.foundation.absolute.v1" });
  }
  if (path.endsWith("/editor-sessions")) return jsonResponse(session);
  if (path.endsWith("/editor-sessions/" + SESSION)) {
    return jsonResponse({ ...canonicalSession, schema_id: "storyos.query.editor-session.response.v1" });
  }
  if (path.endsWith("/manuscript/author-edits")) {
    lastEdit = JSON.parse(options.body);
    lastIdempotencyKey = options.headers["idempotency-key"];
    counts.authorEdits += 1;
    throw new Error("simulated acknowledgement loss");
  }
  if (path.includes("/manuscript/author-edit-outcomes/")) {
    counts.outcomes += 1;
    if (options.method !== "GET" || options.body !== undefined) fail("outcome Query shape changed");
    if (options.headers["x-storyos-anti-forgery"] !== NONCE) fail("proof header mismatch");
    if (outcomeMode === "unavailable") throw new Error("simulated query unavailable");
    canonicalSession = freshSession;
    return jsonResponse({
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000081",
      project_scope: project.project_scope,
      outcome: {
        outcome_kind: "committed",
        response: {
          schema_id: "storyos.command.apply-author-edit.response.v2",
          correlation_id: lastEdit.correlation_id,
          project_scope: project.project_scope,
          command_id: COMMAND,
          author_command_admission_id: ADMISSION,
          receipt: {
            receipt_id: RECEIPT,
            project_scope: project.project_scope, command_kind: "applyAuthorEdit",
            command_digest: commandDigest, idempotency_key: lastIdempotencyKey,
            producer_cause: "author_command_admission",
            author_command_admission_id: ADMISSION,
            expected_heads: [REVISION], prior_heads: [REVISION],
            resulting_heads: [NEXT_REVISION], authoritative_revision_ids: [NEXT_REVISION],
            proposal_revision_ids: [], authoritative_commit_ids: [NEXT_COMMIT],
            author_action_sequence: "1", draft_artifact_refs: [],
            artifact_lifecycle_event_refs: [], condition_refs: [],
            result: "authoritative_applied", created_at: "2026-08-15T08:00:00.000Z",
          },
          effect: {
            kind: "authoritative_applied",
            authoritative_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
            authoritative_commit_id: NEXT_COMMIT,
            author_action_sequence: "1", project_activity_position: "1",
          },
          completed_intent_record_id: lastEdit.completed_intent_record_id,
          local_intent_sequence: lastEdit.local_intent_sequence,
        },
      },
    });
  }
  fail("Unexpected request: " + path);
};

async function deleteJournal(workspace) {
  workspace.database.close();
  await requestResult(indexedDB.deleteDatabase(journalName));
  sessionStorage.removeItem("active_session:" + OWNER + ":" + PROJECT);
}

async function openReady() {
  const workspace = await openEditorWorkspace({
    baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto,
  });
  if (workspace.kind !== "editor-ready") {
    fail("workspace unavailable: " + (workspace.error?.stack ?? workspace.code));
  }
  equal(workspace.database.version, 3, "IndexedDB hard cut");
  deepEqual([...workspace.database.objectStoreNames].sort(), [
    "intents", "metadata", "outcome_query_attempts", "outcome_query_observations",
    "partitions", "payload_chains", "protocol_observations", "submission_groups",
    "transport_attempts", "transport_capsules",
  ], "journal stores");
  return workspace;
}

const report = (result, text) => fetch("/result", {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ result, text }),
});

try {
  {
    const request = indexedDB.open(journalName, 2);
    request.onupgradeneeded = () => {
      const database = request.result;
      database.createObjectStore("metadata", { keyPath: "key" });
      database.createObjectStore("partitions", { keyPath: "journal_partition_id" });
      database.createObjectStore("payload_chains", { keyPath: "payload_chain_id" })
        .createIndex("partition", "journal_partition_id", { unique: false });
      database.createObjectStore("intents", {
        keyPath: ["journal_partition_id", "local_intent_sequence"],
      }).createIndex("partition", "journal_partition_id", { unique: false });
      database.createObjectStore("submission_groups", {
        keyPath: "journal_submission_group_id",
      }).createIndex("partition", "journal_partition_id", { unique: false });
      request.transaction.objectStore("metadata").put({ key: "schema", version: 2 });
    };
    const stale = await requestResult(request);
    stale.close();
    const refused = await openEditorWorkspace({
      baseUrl: location.origin, project, chapter, profile,
      fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto,
    });
    equal(refused.kind, "editor-read-only-recovery", "v2 kind");
    equal(refused.code, "local_journal_unavailable", "v2 code");
    await requestResult(indexedDB.deleteDatabase(journalName));
    sessionStorage.removeItem("active_session:" + OWNER + ":" + PROJECT);
  }

  {
    canonicalSession = session;
    outcomeMode = "unavailable";
    counts.authorEdits = 0;
    counts.outcomes = 0;
    const workspace = await openReady();
    await persistReplaceSelection(workspace, {
      from: 4, to: 4, text: "!?", resultingBody: "Base!?",
      inputOrigin: "paste", undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    const first = await submitOnePendingAuthorEdit({
      workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    });
    deepEqual(first, {
      body: "Base!?", save_state: "saving", unsettled_intent_count: 1,
      authoritative_revision_id: REVISION,
    }, "unresolved after lost acknowledgement");
    const snapshot = await validateJournalSnapshot(workspace,
      await readJournalSnapshot(workspace));
    equal(JSON.stringify(snapshot.groups[0]).includes(NONCE), false,
      "nonce stayed out of the Journal group");
    equal(snapshot.groups[0].reconciliation?.kind, "outcome_query_unresolved",
      "unresolved group");
    equal(counts.authorEdits, 1, "one POST before reload");
    equal(counts.outcomes, 1, "one GET before reload");
    workspace.database.close();

    canonicalSession = session;
    outcomeMode = "committed";
    const reopened = await openReady();
    const recovered = await submitOnePendingAuthorEdit({
      workspace: reopened, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    });
    deepEqual(recovered, {
      body: "Base!?", save_state: "saved", unsettled_intent_count: 0,
      authoritative_revision_id: NEXT_REVISION,
    }, "reload recovered from the persisted capsule");
    equal(counts.authorEdits, 1, "reload did not POST again");
    equal(counts.outcomes, 2, "reload queried the original outcome");
    await deleteJournal(reopened);
  }

  await report("pass", "reload recovery passed");
  document.body.dataset.result = "pass";
  document.body.textContent = "reload recovery passed";
} catch (error) {
  await report("fail", error?.stack ?? String(error));
  document.body.dataset.result = "fail";
  document.body.textContent = error?.stack ?? String(error);
}
</script></body></html>`;

function contentType(pathname) {
  return extname(pathname) === ".mjs" ? "text/javascript; charset=utf-8" : "text/plain";
}

test("reload recovers ApplyAuthorEdit from a persisted capsule without a second POST", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 60_000,
}, async () => {
  let resolveReport;
  const reported = new Promise((resolve) => { resolveReport = resolve; });
  const allowed = new Set([
    "/apps/web/src/editor-session.mjs",
    "/apps/web/src/author-edit-submission.mjs",
    "/apps/web/src/author-edit-outcome-reconciliation.mjs",
    "/apps/web/src/local-edit-journal.mjs",
    "/apps/web/src/protected-transport-capsule.mjs",
    "/generated/typescript/storyos-public-release-1/client.mjs",
  ]);
  const server = createServer(async (request, response) => {
    if (request.url === "/harness") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" }).end(harness);
      return;
    }
    if (request.url === "/result" && request.method === "POST") {
      let body = "";
      for await (const chunk of request) body += chunk;
      response.writeHead(204).end();
      resolveReport(JSON.parse(body));
      return;
    }
    const pathname = new URL(request.url, "http://storyos.test").pathname;
    if (!allowed.has(pathname)) {
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
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-reload-recovery-browser-"));
  const url = `http://127.0.0.1:${server.address().port}/harness`;
  const browser = spawn(chromeExecutable, [
    "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
    "--disable-dev-shm-usage", "--no-first-run", `--user-data-dir=${profileDirectory}`, url,
  ], { stdio: "ignore" });
  try {
    const result = await Promise.race([
      reported,
      once(browser, "exit").then(([code]) => { throw new Error(`Chrome exited early: ${code}`); }),
      new Promise((_, reject) => setTimeout(
        () => reject(new Error("real-browser reload recovery timed out")), 20_000,
      ).unref()),
    ]);
    assert.deepEqual(result, {
      result: "pass",
      text: "reload recovery passed",
    });
  } finally {
    server.close();
    if (browser.exitCode === null) browser.kill("SIGTERM");
    await Promise.race([once(browser, "exit"), new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});

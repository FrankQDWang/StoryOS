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
import { openEditorWorkspace } from "/apps/web/src/editor-session.mjs";
import { rebuildPendingProjection } from "/apps/web/src/local-edit-journal.mjs";
import { ingestProjectActivityFrames, readProjectActivityIngest }
  from "/apps/web/src/project-activity-ingest.mjs";

const fail = (message) => { throw new Error(message); };
const deepEqual = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail(label + ": expected "
    + JSON.stringify(expected) + " but received " + JSON.stringify(actual));
};

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000002";
const CHAPTER = "018f0000-0000-7001-8000-000000000003";
const REVISION = "018f0000-0000-7001-8000-000000000004";
const SESSION = "018f0000-0000-7001-8000-000000000021";
const EVENT_A = "018f0000-0000-7001-8000-000000000371";
const EVENT_B = "018f0000-0000-7001-8000-000000000372";
const COMMAND_A = "018f0000-0000-7001-8000-000000000373";
const COMMAND_B = "018f0000-0000-7001-8000-000000000374";
const RECEIPT_A = "018f0000-0000-7001-8000-000000000375";
const RECEIPT_B = "018f0000-0000-7001-8000-000000000376";
const REVISION_A = "018f0000-0000-7001-8000-000000000377";
const REVISION_B = "018f0000-0000-7001-8000-000000000378";
const COMMIT_A = "018f0000-0000-7001-8000-000000000379";
const COMMIT_B = "018f0000-0000-7001-8000-000000000380";
const CORRELATION_A = "018f0000-0000-7001-8000-000000000381";
const CORRELATION_B = "018f0000-0000-7001-8000-000000000382";
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

const jsonResponse = (body) => new Response(JSON.stringify(body), {
  status: 200, headers: { "content-type": "application/json" },
});
const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
});

const journalName = "storyos-local-edit-journal:" + OWNER + ":" + PROJECT;
const fetchImpl = async (url, options = {}) => {
  const parsed = new URL(url);
  const path = parsed.pathname;
  if (path.endsWith("/anti-forgery-challenges")) {
    return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-13T08:05:00.000Z",
      limit_profile_revision: "storyos.foundation.absolute.v1" });
  }
  if (path.endsWith("/editor-sessions")) return jsonResponse(session);
  if (path.endsWith("/editor-sessions/" + SESSION)) {
    return jsonResponse({ ...session, schema_id: "storyos.query.editor-session.response.v1" });
  }
  fail("unexpected fetch " + (options.method ?? "GET") + " " + path);
};

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonicalJson(value[key])]));
  }
  return value;
}

async function eventPayloadDigest(payload) {
  const digest = new Uint8Array(await crypto.subtle.digest(
    "SHA-256", new TextEncoder().encode(JSON.stringify(canonicalJson(payload))),
  ));
  return {
    algorithm: "sha256",
    profile: "storyos.event-payload.jcs.v1",
    value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

async function appliedEvent({
  eventId, commandId, receiptId, correlationId, revisionId, commitId, sequence, occurredAt,
}) {
  const payload = {
    chapter_id: CHAPTER,
    authoritative_revision_id: revisionId,
    authoritative_commit_id: commitId,
    author_action_sequence: sequence,
  };
  return {
    envelope_version: 1,
    activity_profile: "storyos.project-activity.v1",
    event_id: eventId,
    event_schema: "storyos.event.authoritative-author-edit-applied.v1",
    event_kind: "authoritative_author_edit_applied",
    project_scope: project.project_scope,
    requester_user_id: OWNER,
    actor: { kind: "author", id: OWNER },
    project_sequence: sequence,
    stream_sequence: sequence,
    agent_run_id: null,
    run_step_id: null,
    run_sequence: null,
    aggregate_ref: { kind: "chapter", id: CHAPTER },
    correlation_id: correlationId,
    causation: { kind: "command", id: commandId },
    command_id: commandId,
    receipt_ref: { kind: "domain_receipt", id: receiptId },
    occurred_at: occurredAt,
    recorded_at: occurredAt,
    payload,
    payload_digest: await eventPayloadDigest(payload),
    application_wire_record_ref: eventId,
    limit_profile_revision: "storyos.foundation.absolute.v1",
  };
}

const report = (result, text) => fetch("/result", {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ result, text }),
});

try {
  await requestResult(indexedDB.deleteDatabase(journalName));
  const workspace = await openEditorWorkspace({
    baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto,
  });
  if (workspace.kind !== "editor-ready") {
    fail("workspace unavailable: " + (workspace.error?.stack ?? workspace.code));
  }
  const eventA = await appliedEvent({
    eventId: EVENT_A, commandId: COMMAND_A, receiptId: RECEIPT_A, correlationId: CORRELATION_A,
    revisionId: REVISION_A, commitId: COMMIT_A, sequence: "1",
    occurredAt: "2026-08-20T03:00:00.000Z",
  });
  const eventB = await appliedEvent({
    eventId: EVENT_B, commandId: COMMAND_B, receiptId: RECEIPT_B, correlationId: CORRELATION_B,
    revisionId: REVISION_B, commitId: COMMIT_B, sequence: "2",
    occurredAt: "2026-08-20T03:00:00.001Z",
  });
  const frame = (id, data) => ({ id, event: "storyos.project-activity", data });
  const expectedProjection = {
    body: "Base", save_state: "clean", unsettled_intent_count: 0,
    authoritative_revision_id: REVISION,
  };
  await ingestProjectActivityFrames(workspace, [frame("cursor-b", eventB)]);
  const heldB = {
    replay_generation: "1",
    processed_through_stream_sequence: "0",
    events: [],
    held: [eventB],
  };
  deepEqual(await readProjectActivityIngest(workspace), heldB, "out-of-order Event is held");
  await ingestProjectActivityFrames(workspace, [
    frame("cursor-a", eventA), frame("cursor-a", eventA),
  ]);
  const contiguous = {
    replay_generation: "1",
    processed_through_stream_sequence: "2",
    events: [eventA, eventB],
    held: [],
  };
  deepEqual(await readProjectActivityIngest(workspace), contiguous,
    "duplicate and reordered frames converge");
  await ingestProjectActivityFrames(workspace, [
    frame("cursor-b", eventB), frame("cursor-a", eventA),
  ]);
  deepEqual(await readProjectActivityIngest(workspace), contiguous,
    "later duplicates leave the durable ingest unchanged");
  deepEqual(await rebuildPendingProjection(workspace), expectedProjection,
    "Event arrival does not settle the local group");
  workspace.database.close();
  await requestResult(indexedDB.deleteDatabase(journalName));
  const inOrder = await openEditorWorkspace({
    baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto,
  });
  if (inOrder.kind !== "editor-ready") {
    fail("in-order workspace unavailable: " + (inOrder.error?.stack ?? inOrder.code));
  }
  await ingestProjectActivityFrames(inOrder, [
    frame("cursor-a", eventA), frame("cursor-b", eventB),
  ]);
  deepEqual(await readProjectActivityIngest(inOrder), contiguous,
    "in-order ingest matches reordered durable ingest");
  inOrder.database.close();
  await requestResult(indexedDB.deleteDatabase(journalName));
  sessionStorage.removeItem("active_session:" + OWNER + ":" + PROJECT);
  await report("pass", "activity reorder ingest passed");
  document.body.dataset.result = "pass";
  document.body.textContent = "activity reorder ingest passed";
} catch (error) {
  await report("fail", error?.stack ?? String(error));
  document.body.dataset.result = "fail";
  document.body.textContent = error?.stack ?? String(error);
}
</script></body></html>`;

function contentType(pathname) {
  return extname(pathname) === ".mjs" ? "text/javascript; charset=utf-8" : "text/plain";
}

test("duplicate and reordered Activity frames converge to one durable ingest", {
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
    "/apps/web/src/project-activity-ingest.mjs",
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
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-activity-reorder-browser-"));
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
        () => reject(new Error("real-browser activity reorder ingest timed out")), 20_000,
      ).unref()),
    ]);
    assert.deepEqual(result, {
      result: "pass",
      text: "activity reorder ingest passed",
    });
  } finally {
    server.close();
    if (browser.exitCode === null) browser.kill("SIGTERM");
    await Promise.race([once(browser, "exit"), new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});

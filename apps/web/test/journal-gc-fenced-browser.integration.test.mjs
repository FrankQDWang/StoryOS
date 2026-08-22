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

const harness = `<!doctype html>
<html><body data-result="running"><script type="module">
import { openEditorWorkspace, persistReplaceSelection, submitOnePendingAuthorEdit }
  from "/apps/web/src/editor-session.ts";
import { readJournalSnapshot, rebuildPendingProjection }
  from "/apps/web/src/local-edit-journal.ts";
import { collectEligibleJournalPayload } from "/apps/web/src/journal-payload-collection.ts";

const fail = (message) => { throw new Error(message); };
const equal = (actual, expected, label) => {
  if (actual !== expected) fail(label + ": expected " + JSON.stringify(expected)
    + " but received " + JSON.stringify(actual));
};
const deepEqual = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail(label + ": expected "
    + JSON.stringify(expected) + " but received " + JSON.stringify(actual));
};

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
const NEXT_SNAPSHOT = "018f0000-0000-7001-8000-000000000038";
const SUCCESSOR_DIGEST = {
  algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
  value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
};
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
const fencedBase = {
  snapshot_id: NEXT_SNAPSHOT, chapter_id: CHAPTER,
  project_activity_position: "1", authoritative_head_revision_id: NEXT_REVISION,
  proposal_head_revision_ids: [], target_refs: ["manuscript:" + CHAPTER],
  observed_ownership_partition: "authoritative",
  materialized_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
  materialized_payload_digest: SUCCESSOR_DIGEST,
  created_at: "2026-08-15T08:00:00.000Z",
};
const fencedSession = {
  ...session,
  schema_id: "storyos.query.editor-session.response.v1",
  correlation_id: "018f0000-0000-7001-8000-000000000091",
  writer: { kind: "read_only", observed_writer_generation: "2", reason: "superseded_by_takeover" },
  base_snapshot: fencedBase,
};

const jsonResponse = (body) => new Response(JSON.stringify(body), {
  status: 200, headers: { "content-type": "application/json" },
});
const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
});

const journalName = "storyos-local-edit-journal:" + OWNER + ":" + PROJECT;
let commandDigest;
let lastEdit;
let lastIdempotencyKey;
let canonicalSession = session;
const counts = { authorEdits: 0, outcomes: 0 };

const fetchImpl = async (url, options = {}) => {
  const parsed = new URL(url);
  const path = parsed.pathname;
  if (/draft/i.test(path)) fail("fenced collection created a Recovery Draft route: " + path);
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
    canonicalSession = fencedSession;
    throw new Error("simulated acknowledgement loss");
  }
  if (path.includes("/manuscript/author-edit-outcomes/")) {
    counts.outcomes += 1;
    if (options.method !== "GET" || options.body !== undefined) fail("outcome Query shape changed");
    if (options.headers["x-storyos-anti-forgery"] !== NONCE) fail("proof header mismatch");
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
  fail("unexpected fetch " + (options.method ?? "GET") + " " + path);
};

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
  const openPartition = { ...workspace.partition };
  await persistReplaceSelection(workspace, {
    from: 4, to: 4, text: "!?", resultingBody: "Base!?",
    inputOrigin: "paste", undoGroupId: "018f0000-0000-7001-8000-000000000040",
    createdAt: "2026-08-15T08:00:00.000Z",
  });
  const projection = await submitOnePendingAuthorEdit({
    workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
  });
  deepEqual(projection, {
    body: "Base!?", save_state: "saved", unsettled_intent_count: 0,
    authoritative_revision_id: NEXT_REVISION,
  }, "fenced late settlement projection");
  const expectedPartition = { ...openPartition, disposition: "read_only_observer" };
  deepEqual(workspace.partition, expectedPartition, "in-memory fenced partition");
  deepEqual(workspace.session.writer, fencedSession.writer, "recorded resulting generation");
  const settled = await readJournalSnapshot(workspace);
  const collection = await collectEligibleJournalPayload(workspace);
  const fence = collection.fences[0];
  const record = settled.records[0];
  const chain = settled.payloadChains[0];
  const group = settled.groups[0];
  const expectedFence = {
    collection_fence_id: fence.collection_fence_id,
    journal_partition_id: workspace.partition.journal_partition_id,
    project_scope: project.project_scope,
    writer_generation: "1",
    partition_disposition: "read_only_observer",
    resulting_writer_generation: "2",
    collected_groups: [{
      journal_submission_group_id: group.journal_submission_group_id,
      covered_sequence_range: { first: 1, last: 1 },
      payload_digests: [record.payload_digest],
      settlement_kind: "applied_receipt_settled",
      command_id: COMMAND,
      author_command_admission_id: ADMISSION,
      receipt_id: RECEIPT,
      project_activity_position: "1",
    }],
    successor: {
      kind: "authoritative_revision",
      snapshot_id: NEXT_SNAPSHOT,
      revision_id: NEXT_REVISION,
      materialized_payload_digest: SUCCESSOR_DIGEST,
    },
    collected_intent_sequences: [1],
    collected_payload_chain_ids: [chain.payload_chain_id],
    reason: "applied_receipt_converged_with_durable_successor",
  };
  deepEqual(collection, { fences: [expectedFence] },
    "fenced partition collects under its immutable old generation");
  const collected = await readJournalSnapshot(workspace);
  deepEqual(collected.fences, [expectedFence], "durable fenced collection fence remains");
  equal(collected.records[0].author_edit_unit, undefined, "intent payload bytes are collected");
  equal(collected.payloadChains[0].checkpoint_ref.materialized_payload, undefined,
    "checkpoint payload bytes are collected");
  deepEqual(collected.groups[0].payload_collection, {
    kind: "collected", collection_fence_id: fence.collection_fence_id,
  }, "collected group keeps compact identity");
  equal(workspace.partition.writer_generation, "1", "old generation is not rewritten");
  deepEqual(await rebuildPendingProjection(workspace), {
    body: "Base!?", save_state: "saved", unsettled_intent_count: 0,
    authoritative_revision_id: NEXT_REVISION,
  }, "successor projection remains after fenced collection");
  let blocked = false;
  try {
    await persistReplaceSelection(workspace, {
      from: 6, to: 6, text: "+", resultingBody: "Base!?+",
      inputOrigin: "typing", undoGroupId: "018f0000-0000-7001-8000-000000000041",
      createdAt: "2026-08-15T08:00:01.000Z",
    });
  } catch (error) {
    blocked = /read only/.test(String(error?.message ?? error));
  }
  if (!blocked) fail("fenced partition accepted later input");
  equal(counts.authorEdits, 1, "no second POST");
  workspace.database.close();
  await requestResult(indexedDB.deleteDatabase(journalName));
  sessionStorage.removeItem("active_session:" + OWNER + ":" + PROJECT);
  await report("pass", "fenced journal payload collection passed");
  document.body.dataset.result = "pass";
  document.body.textContent = "fenced journal payload collection passed";
} catch (error) {
  await report("fail", error?.stack ?? String(error));
  document.body.dataset.result = "fail";
  document.body.textContent = error?.stack ?? String(error);
}
</script></body></html>`;


test("a fenced partition collects applied payload under its immutable old generation", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 60_000,
}, async () => {
  let resolveReport;
  const reported = new Promise((resolve) => { resolveReport = resolve; });
  const allowed = new Set([
    "/apps/web/src/editor-session.ts",
    "/apps/web/src/author-edit-submission.ts",
    "/apps/web/src/author-edit-outcome-reconciliation.ts",
    "/apps/web/src/local-edit-journal.ts",
    "/apps/web/src/protected-transport-capsule.ts",
    "/apps/web/src/journal-payload-collection.ts",
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
    const module = await loadLegacyBrowserModule(repositoryRoot, pathname);
    response.writeHead(200, { "content-type": module.contentType });
    response.end(module.body);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-journal-gc-fenced-browser-"));
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
        () => reject(new Error("real-browser fenced journal payload collection timed out")), 20_000,
      ).unref()),
    ]);
    assert.deepEqual(result, {
      result: "pass",
      text: "fenced journal payload collection passed",
    });
  } finally {
    server.close();
    if (browser.exitCode === null) browser.kill("SIGTERM");
    await Promise.race([once(browser, "exit"), new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});

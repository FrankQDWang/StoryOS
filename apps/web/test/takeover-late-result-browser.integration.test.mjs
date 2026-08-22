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
import { readJournalSnapshot, validateJournalSnapshot }
  from "/apps/web/src/local-edit-journal.ts";

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
  snapshot_id: "018f0000-0000-7001-8000-000000000038",
  chapter_id: CHAPTER,
  project_activity_position: "1",
  authoritative_head_revision_id: NEXT_REVISION,
  proposal_head_revision_ids: [],
  target_refs: ["manuscript:" + CHAPTER],
  observed_ownership_partition: "authoritative",
  materialized_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
  materialized_payload_digest: {
    algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
    value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
  },
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
  fail("Unexpected request: " + path);
};

const report = (result, text) => fetch("/result", {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ result, text }),
});

try {
  const workspace = await openEditorWorkspace({
    baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto,
  });
  if (workspace.kind !== "editor-ready") {
    fail("workspace unavailable: " + (workspace.error?.stack ?? workspace.code));
  }
  const openPartition = { ...workspace.partition };
  deepEqual(openPartition, {
    ...openPartition,
    writer_generation: "1",
    disposition: "current_writer_open",
  }, "open partition");
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
  equal(counts.authorEdits, 1, "one POST");
  equal(counts.outcomes, 1, "one GET outcome");
  const expectedPartition = { ...openPartition, disposition: "read_only_observer" };
  deepEqual(workspace.partition, expectedPartition, "in-memory fenced partition");
  deepEqual(workspace.session.writer, fencedSession.writer, "session writer after fence");
  const durablePartition = await requestResult(
    workspace.database.transaction("partitions").objectStore("partitions")
      .get(openPartition.journal_partition_id),
  );
  deepEqual(durablePartition, expectedPartition, "durable fenced partition");
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const group = snapshot.groups[0];
  const records = snapshot.records;
  deepEqual(group, {
    journal_submission_group_id: group.journal_submission_group_id,
    journal_partition_id: openPartition.journal_partition_id,
    project_scope: project.project_scope,
    editor_session_id: SESSION,
    writer_generation: "1",
    batch_policy_revision: "storyos.author-edit-batch.release-1.preview.v1",
    ordered_coverage: records.map((record) => ({
      local_intent_sequence: record.local_intent_sequence,
      intent_record_ref: record.completed_intent_record_id,
      payload_digest: record.payload_digest,
    })),
    covered_sequence_range: {
      first: records[0].local_intent_sequence, last: records.at(-1).local_intent_sequence,
    },
    action_class: "direct_editor_action",
    api_major: 1,
    method: "POST",
    route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
    command_schema: "storyos.command.apply-author-edit.request.v1",
    command_kind: "applyAuthorEdit",
    digest_profile: "storyos.command.applyAuthorEdit.jcs.v1",
    idempotency_key: group.idempotency_key,
    frozen_request_body: {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: group.frozen_request_body.correlation_id,
      editor_session_id: SESSION,
      writer_generation: "1",
      chapter_id: CHAPTER,
      expected_authoritative_revision_id: records[0].expected_authoritative_heads[0],
      expected_proposal_head_revision_ids: [],
      target_refs: records[0].target_refs,
      observed_ownership_partition: "authoritative",
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: records[0].undo_group_binding.undo_group_id,
      completed_intent_record_id: records[0].completed_intent_record_id,
      local_intent_sequence: String(records[0].local_intent_sequence),
      author_edit_units: records.map((record) => record.author_edit_unit),
    },
    frozen_request_digest: group.frozen_request_digest,
    frozen_payload_coverage_digest: group.frozen_payload_coverage_digest,
    settlement: {
      kind: "applied_receipt_settled",
      command_id: COMMAND,
      author_command_admission_id: ADMISSION,
      receipt: {
        receipt_id: RECEIPT,
        project_scope: project.project_scope,
        command_kind: "applyAuthorEdit",
        command_digest: group.frozen_request_digest,
        idempotency_key: group.idempotency_key,
        producer_cause: "author_command_admission",
        author_command_admission_id: ADMISSION,
        expected_heads: [REVISION], prior_heads: [REVISION],
        resulting_heads: [NEXT_REVISION], authoritative_revision_ids: [NEXT_REVISION],
        proposal_revision_ids: [], authoritative_commit_ids: [NEXT_COMMIT],
        author_action_sequence: "1", draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [], condition_refs: [],
        result: "authoritative_applied", created_at: "2026-08-15T08:00:00.000Z",
      },
      authoritative_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
      authoritative_commit_id: NEXT_COMMIT,
      author_action_sequence: "1",
      project_activity_position: "1",
      installed_base_snapshot: fencedBase,
    },
    frozen_at: group.frozen_at,
  }, "complete durable group");
  equal(snapshot.groups.length, 1, "one partition group");
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
  await report("pass", "fenced late settlement passed");
  document.body.dataset.result = "pass";
  document.body.textContent = "fenced late settlement passed";
} catch (error) {
  await report("fail", error?.stack ?? String(error));
  document.body.dataset.result = "fail";
  document.body.textContent = error?.stack ?? String(error);
}
</script></body></html>`;


test("a fenced writer settles a matching late ApplyAuthorEdit result without new writing", {
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
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-takeover-late-browser-"));
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
        () => reject(new Error("real-browser fenced late settlement timed out")), 20_000,
      ).unref()),
    ]);
    assert.deepEqual(result, {
      result: "pass",
      text: "fenced late settlement passed",
    });
  } finally {
    server.close();
    if (browser.exitCode === null) browser.kill("SIGTERM");
    await Promise.race([once(browser, "exit"), new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});

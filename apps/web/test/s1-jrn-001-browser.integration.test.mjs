import assert from "node:assert/strict";
import { createHash } from "node:crypto";
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

import { activityStream } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { JOURNAL_DATABASE_VERSION } from "../src/local-edit-journal.ts";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const distDir = join(repositoryRoot, "apps/web/dist");
const serverBinary = join(
  repositoryRoot, "target", "debug",
  process.platform === "win32" ? "storyos-server.exe" : "storyos-server",
);
const execFileAsync = promisify(execFile);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const CHAPTER = "018f0000-0000-7001-8000-000000000003";
const INITIAL_REVISION = "018f0000-0000-7001-8000-000000000005";
const SCOPE = { owner_user_id: USER_A, project_id: PROJECT_A };
const TARGET = [`manuscript:${CHAPTER}`];
const CLIENT_CONTRACT = "storyos.web-client.release-1.v3";
const SECURITY_POLICY = "storyos.web-security-policy.release-1.v1";
const EDITOR_CONTRACT = "storyos.editor-contract.release-1.v2";
const BATCH_POLICY = "storyos.author-edit-batch.release-1.preview.v1";
const LIMIT_PROFILE = "storyos.foundation.absolute.v1";
const PARTITION = [
  USER_A, PROJECT_A, "editor-session", "1", "client-session", "1",
  CLIENT_CONTRACT, SECURITY_POLICY, LIMIT_PROFILE,
].join(":");
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
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
const BODIES = [OPEN_BODY, AFTER_TYPE, AFTER_IME, AFTER_PASTE, AFTER_UNSETTLED];
const TEXTS = [" Hello", "中文", " EN", "!"];
const ORIGINS = ["typing", "typing", "paste", "typing"];

function contentType(pathname) {
  const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
    ".css": "text/css; charset=utf-8" };
  return types[extname(pathname)] ?? "application/octet-stream";
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

function sha256Hex(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function digestValue(profile, value, json = true) {
  const bytes = new TextEncoder().encode(json ? JSON.stringify(value) : value);
  return { algorithm: "sha256", profile, value_hex_lowercase: sha256Hex(bytes) };
}

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonicalJson(value[key])]));
  }
  return value;
}

const JOURNAL_PAYLOAD = "storyos.local-edit-journal.payload.sha256.v1";
const JOURNAL_COVERAGE = "storyos.local-edit-journal.submission-coverage.sha256.v1";
const CANONICAL_PAYLOAD = "storyos.canonical-payload.sha256.v1";
const COMMAND_DIGEST = "storyos.command.applyAuthorEdit.jcs.v1";
const COMMAND_SCHEMA = "storyos.command.apply-author-edit.request.v1";
const journalDigest = (value) => digestValue(JOURNAL_PAYLOAD, value);
const bodyDigest = (body) => digestValue(CANONICAL_PAYLOAD, body, false);
const eventPayloadDigest = (payload) => digestValue(
  "storyos.event-payload.jcs.v1", canonicalJson(payload),
);
const pendingProjection = (body, save_state, unsettled_intent_count, authoritative_revision_id) =>
  ({ body, save_state, unsettled_intent_count, authoritative_revision_id });
const priorRevision = (index) => (index === 0 ? INITIAL_REVISION : `revision-${index}`);
const replaceUnit = (from, text) => ({
  normalized_primitives: [{ kind: "replace_selection", from, to: from, text }],
  selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from },
});
const slot = (kind, n) => `${kind}-${n}`;
const commandDigestValue = (n) => ({
  algorithm: "sha256", profile: COMMAND_DIGEST, value_hex_lowercase: slot("digest-command", n),
});

function appliedReceipt(index) {
  const n = index + 1;
  const expected = priorRevision(index);
  const resulting = slot("revision", n);
  return {
    receipt_id: slot("receipt", n), project_scope: SCOPE, command_kind: "applyAuthorEdit",
    command_digest: commandDigestValue(n), idempotency_key: slot("idempotency", n),
    producer_cause: "author_command_admission", author_command_admission_id: slot("admission", n),
    expected_heads: [expected], prior_heads: [expected], resulting_heads: [resulting],
    authoritative_revision_ids: [resulting], proposal_revision_ids: [],
    authoritative_commit_ids: [slot("commit", n)], author_action_sequence: String(n),
    draft_artifact_refs: [], artifact_lifecycle_event_refs: [], condition_refs: [],
    result: "authoritative_applied", created_at: "instant",
  };
}

function appliedEffect(index) {
  const n = index + 1;
  return {
    kind: "authoritative_applied",
    authoritative_revision: { revision_id: slot("revision", n), body: BODIES[n] },
    authoritative_commit_id: slot("commit", n), author_action_sequence: String(n),
    project_activity_position: String(n),
  };
}

function appliedActivity(index) {
  const n = index + 1;
  const payload = {
    chapter_id: CHAPTER, authoritative_revision_id: slot("revision", n),
    authoritative_commit_id: slot("commit", n), author_action_sequence: String(n),
  };
  return {
    envelope_version: 1, activity_profile: "storyos.project-activity.v1",
    event_id: slot("event", n), event_schema: "storyos.event.authoritative-author-edit-applied.v1",
    event_kind: "authoritative_author_edit_applied", project_scope: SCOPE,
    requester_user_id: USER_A, actor: { kind: "author", id: USER_A },
    project_sequence: String(n), stream_sequence: String(n),
    agent_run_id: null, run_step_id: null, run_sequence: null,
    aggregate_ref: { kind: "chapter", id: CHAPTER }, correlation_id: slot("correlation", n),
    causation: { kind: "command", id: slot("command", n) }, command_id: slot("command", n),
    receipt_ref: { kind: "domain_receipt", id: slot("receipt", n) },
    occurred_at: "instant", recorded_at: "instant", payload,
    payload_digest: { algorithm: "sha256", profile: "storyos.event-payload.jcs.v1",
      value_hex_lowercase: slot("digest-event", n) },
    application_wire_record_ref: slot("event", n), limit_profile_revision: LIMIT_PROFILE,
  };
}

function intentRecord(index, retainedUnit) {
  const n = index + 1;
  const unit = replaceUnit(BODIES[index].length, TEXTS[index]);
  const snapshot = slot("snapshot", index);
  const record = {
    completed_intent_record_id: slot("intent", n), local_intent_sequence: n,
    journal_partition_id: PARTITION, project_scope: SCOPE, editor_session_id: "editor-session",
    writer_generation: "1", limit_profile_revision: LIMIT_PROFILE, batch_policy_revision: BATCH_POLICY,
    input_origin: ORIGINS[index], chapter_object_id: CHAPTER, base_snapshot_id: snapshot,
    base_activity_position: String(index), target_refs: TARGET,
    expected_authoritative_heads: [priorRevision(index)], expected_proposal_heads: [],
    proposal_anchors: [], observed_ownership_partition: "authoritative",
    retry_source: { kind: "fresh_editor_intent" }, editor_contract_revision: EDITOR_CONTRACT,
    undo_group_binding: { kind: "direct_author_input", undo_group_id: slot("undo", n) },
    payload_chain_ref: slot("chain", n), payload_digest: journalDigest(unit),
    projection_dependency: { snapshot_id: snapshot, prior_sequence: index }, created_at: "instant",
  };
  if (retainedUnit) record.author_edit_unit = unit;
  return record;
}

function installedSnapshot(index) {
  const n = index + 1;
  const revision = slot("revision", n);
  const body = BODIES[n];
  return {
    snapshot_id: slot("snapshot", n), chapter_id: CHAPTER, project_activity_position: String(n),
    authoritative_head_revision_id: revision, proposal_head_revision_ids: [], target_refs: TARGET,
    observed_ownership_partition: "authoritative",
    materialized_revision: { revision_id: revision, body },
    materialized_payload_digest: bodyDigest(body), created_at: "instant",
  };
}

function collectedGroup(index) {
  const n = index + 1;
  const unit = replaceUnit(BODIES[index].length, TEXTS[index]);
  const orderedCoverage = [{
    local_intent_sequence: n, intent_record_ref: slot("intent", n), payload_digest: journalDigest(unit),
  }];
  const range = { first: n, last: n };
  return {
    journal_submission_group_id: slot("group", n), journal_partition_id: PARTITION,
    project_scope: SCOPE, editor_session_id: "editor-session", writer_generation: "1",
    batch_policy_revision: BATCH_POLICY, ordered_coverage: orderedCoverage,
    covered_sequence_range: range, action_class: "direct_editor_action", api_major: 1,
    method: "POST", route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
    command_schema: COMMAND_SCHEMA, command_kind: "applyAuthorEdit", digest_profile: COMMAND_DIGEST,
    idempotency_key: slot("idempotency", n),
    frozen_request_body: {
      command_schema: COMMAND_SCHEMA, client_contract_revision: CLIENT_CONTRACT,
      security_policy_revision: SECURITY_POLICY, correlation_id: slot("correlation", n),
      editor_session_id: "editor-session", writer_generation: "1", chapter_id: CHAPTER,
      expected_authoritative_revision_id: priorRevision(index),
      expected_proposal_head_revision_ids: [], target_refs: TARGET,
      observed_ownership_partition: "authoritative", editor_contract_revision: EDITOR_CONTRACT,
      undo_group_id: slot("undo", n), completed_intent_record_id: slot("intent", n),
      local_intent_sequence: String(n), author_edit_units: [],
    },
    frozen_request_digest: commandDigestValue(n),
    frozen_payload_coverage_digest: {
      algorithm: "sha256", profile: JOURNAL_COVERAGE, value_hex_lowercase: slot("digest-coverage", n),
    },
    settlement: {
      kind: "applied_receipt_settled", command_id: slot("command", n),
      author_command_admission_id: slot("admission", n), receipt: appliedReceipt(index),
      authoritative_revision: { revision_id: slot("revision", n), body: BODIES[n] },
      authoritative_commit_id: slot("commit", n), author_action_sequence: String(n),
      project_activity_position: String(n), installed_base_snapshot: installedSnapshot(index),
    },
    frozen_at: "instant", payload_collection: { kind: "collected", collection_fence_id: slot("fence", n) },
  };
}

function collectedFence(index) {
  const n = index + 1;
  const unit = replaceUnit(BODIES[index].length, TEXTS[index]);
  return {
    collection_fence_id: slot("fence", n), journal_partition_id: PARTITION, project_scope: SCOPE,
    writer_generation: "1", partition_disposition: "current_writer_open",
    collected_groups: [{
      journal_submission_group_id: slot("group", n), covered_sequence_range: { first: n, last: n },
      payload_digests: [journalDigest(unit)], settlement_kind: "applied_receipt_settled",
      command_id: slot("command", n), author_command_admission_id: slot("admission", n),
      receipt_id: slot("receipt", n), project_activity_position: String(n),
    }],
    successor: {
      kind: "authoritative_revision", snapshot_id: slot("snapshot", n),
      revision_id: slot("revision", n), materialized_payload_digest: bodyDigest(BODIES[n]),
    },
    collected_intent_sequences: [n], collected_payload_chain_ids: [slot("chain", n)],
    reason: "applied_receipt_converged_with_durable_successor",
  };
}
function journalSnapshot(count, extraIntent) {
  return {
    groups: Array.from({ length: count }, (_, index) => collectedGroup(index)),
    intents: [
      ...Array.from({ length: count }, (_, index) => intentRecord(index, false)),
      ...(extraIntent ? [intentRecord(count, true)] : []),
    ],
    fences: Array.from({ length: count }, (_, index) => collectedFence(index)),
  };
}
function bind(map, from, to) {
  if (typeof from === "string" && from.length > 0 && !map.has(from)) map.set(from, to);
}

function rewrite(value, ids, digests) {
  if (Array.isArray(value)) return value.map((item) => rewrite(item, ids, digests));
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, rewrite(item, ids, digests)]));
  }
  if (typeof value !== "string") return value;
  if (INSTANT.test(value)) return "instant";
  let next = value;
  for (const [from, to] of ids) {
    if (from !== to) next = next.split(from).join(to);
  }
  for (const [from, to] of digests) next = next.split(from).join(to);
  return next;
}

function bindJourney(observed) {
  const ids = new Map();
  const digests = new Map();
  observed.authority.receipts.forEach((receipt, index) => {
    const n = index + 1;
    bind(ids, receipt.receipt_id, slot("receipt", n));
    bind(ids, receipt.author_command_admission_id, slot("admission", n));
    bind(ids, receipt.idempotency_key, slot("idempotency", n));
    bind(ids, receipt.authoritative_revision_ids[0], slot("revision", n));
    bind(ids, receipt.authoritative_commit_ids[0], slot("commit", n));
    bind(digests, receipt.command_digest.value_hex_lowercase, slot("digest-command", n));
  });
  observed.authority.activities.forEach((activity, index) => {
    const n = index + 1;
    bind(ids, activity.event_id, slot("event", n));
    bind(ids, activity.command_id, slot("command", n));
    bind(ids, activity.correlation_id, slot("correlation", n));
    bind(digests, activity.payload_digest.value_hex_lowercase, slot("digest-event", n));
  });
  const journal = observed.recover.journal;
  const parts = journal.groups[0].journal_partition_id.split(":");
  bind(ids, parts[2], "editor-session");
  bind(ids, parts[4], "client-session");
  journal.groups.forEach((group, index) => {
    const n = index + 1;
    bind(ids, group.journal_submission_group_id, slot("group", n));
    bind(ids, group.payload_collection?.collection_fence_id, slot("fence", n));
    bind(ids, group.settlement?.installed_base_snapshot?.snapshot_id, slot("snapshot", n));
    bind(ids, group.frozen_request_body?.undo_group_id, slot("undo", n));
    bind(ids, group.frozen_request_body?.completed_intent_record_id, slot("intent", n));
    bind(digests, group.frozen_payload_coverage_digest?.value_hex_lowercase, slot("digest-coverage", n));
  });
  journal.intents.forEach((intent, index) => {
    bind(ids, intent.completed_intent_record_id, slot("intent", index + 1));
    bind(ids, intent.payload_chain_ref, slot("chain", index + 1));
    bind(ids, intent.undo_group_binding?.undo_group_id, slot("undo", index + 1));
    bind(ids, intent.base_snapshot_id, slot("snapshot", index));
  });
  return rewrite(observed, ids, digests);
}

function devToolsAddress(browser) {
  return new Promise((resolve, reject) => {
    let stderr = "";
    const timeout = setTimeout(() => reject(new Error(`Chrome DevTools did not start: ${stderr}`)), 10_000);
    browser.stderr.setEncoding("utf8");
    browser.stderr.on("data", (chunk) => {
      stderr += chunk;
      const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
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
        ...process.env, STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL,
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

function commandDigest(stored) {
  const match = /^sha256:(storyos\.command\.applyAuthorEdit\.jcs\.v1):([0-9a-f]{64})$/.exec(stored);
  assert.ok(match, `command digest is not the Apply Author Edit profile: ${stored}`);
  return { algorithm: "sha256", profile: match[1], value_hex_lowercase: match[2] };
}

async function projectAuthoritySnapshot() {
  const scoped = (alias) =>
    `${alias}.owner_user_id = '${USER_A}'::uuid AND ${alias}.project_id = '${PROJECT_A}'::uuid`;
  const snapshot = JSON.parse(await queryPostgres(`
    SELECT json_build_object(
      'receipts', (SELECT coalesce(json_agg(json_build_object(
        'receipt_id', receipt.receipt_id::text,
        'project_scope', json_build_object('owner_user_id', receipt.owner_user_id::text,
          'project_id', receipt.project_id::text),
        'command_kind', receipt.command_kind, 'command_digest', receipt.command_digest,
        'idempotency_key', receipt.idempotency_key::text,
        'producer_cause', receipt.producer_cause,
        'author_command_admission_id', receipt.author_command_admission_id::text,
        'expected_heads', to_json(receipt.expected_heads), 'prior_heads', to_json(receipt.prior_heads),
        'resulting_heads', to_json(receipt.resulting_heads),
        'authoritative_revision_ids', to_json(receipt.authoritative_revision_ids),
        'proposal_revision_ids', to_json(receipt.proposal_revision_ids),
        'authoritative_commit_ids', to_json(receipt.authoritative_commit_ids),
        'author_action_sequence', action.author_action_sequence::text,
        'draft_artifact_refs', to_json(receipt.draft_artifact_refs),
        'artifact_lifecycle_event_refs', to_json(receipt.artifact_lifecycle_event_refs),
        'condition_refs', to_json(receipt.condition_refs), 'result', receipt.result_kind,
        'created_at', to_char(receipt.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
      ) ORDER BY action.author_action_sequence), '[]'::json)
        FROM storyos.domain_receipts AS receipt
        JOIN storyos.author_action_entries AS action
          ON (action.owner_user_id, action.project_id, action.receipt_id) =
             (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
        WHERE ${scoped("receipt")}),
      'effects', (SELECT coalesce(json_agg(json_build_object(
        'kind', 'authoritative_applied',
        'authoritative_revision', json_build_object('revision_id', activity.resulting_revision_id::text,
          'body', convert_from(payload.canonical_bytes, 'UTF8')),
        'authoritative_commit_id', activity.authoritative_commit_id::text,
        'author_action_sequence', activity.author_action_sequence::text,
        'project_activity_position', activity.project_activity_position::text
      ) ORDER BY activity.project_activity_position), '[]'::json)
        FROM storyos.project_activity_events AS activity
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
             (activity.owner_user_id, activity.project_id, activity.resulting_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
        WHERE ${scoped("activity")}),
      'manuscript', (SELECT json_build_object('revision_id', head.current_revision_id::text,
          'body', convert_from(payload.canonical_bytes, 'UTF8'))
        FROM storyos.authoritative_heads AS head
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
              revision.revision_id) =
             (head.owner_user_id, head.project_id, head.manuscript_object_id, head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
        WHERE ${scoped("head")} AND head.manuscript_object_id = '${CHAPTER}'::uuid),
      'cross_scope_receipts', (SELECT count(*) FROM storyos.domain_receipts
        WHERE owner_user_id <> '${USER_A}'::uuid OR project_id <> '${PROJECT_A}'::uuid)
    )::text`));
  snapshot.receipts = snapshot.receipts.map((receipt) => ({
    ...receipt, command_digest: commandDigest(receipt.command_digest),
  }));
  return snapshot;
}

function parseSseFrames(body) {
  return body.split("\n\n").filter((block) => block.trim().length > 0).map((block) => {
    const frame = { id: "", event: "", data: null };
    for (const line of block.split("\n")) {
      if (line.startsWith("id:")) frame.id = line.slice(3).trim();
      if (line.startsWith("event:")) frame.event = line.slice(6).trim();
      if (line.startsWith("data:")) frame.data = JSON.parse(line.slice(5).trim());
    }
    return frame;
  });
}

async function projectActivities(baseUrl, snapshotId) {
  const body = await activityStream({
    baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
    fetchImpl: (url, options = {}) => fetch(url, {
      ...options, headers: { ...options.headers, origin: baseUrl, cookie: "storyos_session=session-a" },
    }),
  });
  return parseSseFrames(body).map((frame) => {
    assert.equal(frame.event, "storyos.project-activity");
    assert.deepEqual(frame.data.payload_digest, eventPayloadDigest(frame.data.payload));
    return frame.data;
  });
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
  const snapshot = await dumpJourney(command, "timeout");
  throw new Error(`Browser condition timed out: ${expression}\n${JSON.stringify(snapshot)}`);
}

function collectedGroups(count) {
  return `(${JOURNAL}).then((journal) => journal.groups.filter((group) => group.payload_collection?.kind === "collected").length === ${count} && journal.intents.length === ${count} && journal.intents.every((intent) => intent.author_edit_unit === undefined))`;
}

async function dumpJourney(command, label) {
  const pick = (expression) => evaluate(command, expression).catch((error) => String(error?.message ?? error));
  const snapshot = {
    label, userAgent: await pick("navigator.userAgent"), surface: await pick(SURFACE),
    pending: await pick(PENDING), journal: await pick(JOURNAL), recovery: await pick(RECOVERY),
  };
  console.error(`S1-JRN-001 ${label} ${JSON.stringify(snapshot)}`);
  return snapshot;
}

const SURFACE = `({
  bootState: document.querySelector("#app")?.dataset.bootState ?? null,
  heading: document.querySelector("#app h1")?.textContent ?? null,
  chapter: document.querySelector("#app h2")?.textContent ?? null,
  readOnly: document.querySelector("textarea")?.readOnly ?? null,
  alert: document.querySelector('[role="alert"]') ? true : false })`;
const PENDING = `({
  body: document.querySelector("textarea")?.value ?? null,
  save_state: document.querySelector("[data-save-state]")?.dataset.saveState ?? null,
  unsettled_intent_count: Number(document.querySelector("[data-save-state]")?.dataset.unsettledIntentCount),
  authoritative_revision_id: document.querySelector("[data-save-state]")?.dataset.authoritativeRevisionId ?? null })`;
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
        groups: groups.result.slice().sort((left, right) =>
          left.covered_sequence_range.first - right.covered_sequence_range.first),
        intents: intents.result.slice().sort((left, right) =>
          left.local_intent_sequence - right.local_intent_sequence),
        fences: metadata.result.filter((row) => String(row.key).startsWith("collection_fences:"))
          .flatMap((row) => row.value ?? []),
      });
    };
    tx.onerror = () => reject(tx.error);
  };
})`;
const RECOVERY = `({ sessionStorage: sessionStorage.getItem("active_session:${USER_A}:${PROJECT_A}") })`;

async function focusEnd(command) {
  await evaluate(command, `{
    const editor = document.querySelector("textarea");
    editor.focus(); editor.setSelectionRange(editor.value.length, editor.value.length); true;
  }`);
}

const PAGE = { bootState: "project-ready", heading: "Project A", chapter: "Chapter A", readOnly: false, alert: false };

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
    const open = { ...await evaluate(page.command, SURFACE), pending: await evaluate(page.command, PENDING) };
    await focusEnd(page.command);
    await page.command("Input.insertText", { text: " Hello" });
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saving'");
    const input = await evaluate(page.command, PENDING);
    await waitFor(page.command, collectedGroups(1));
    const afterType = await evaluate(page.command, PENDING);
    await focusEnd(page.command);
    await page.command("Input.insertText", { text: "中文" });
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_IME)} && document.querySelector('[data-save-state]')?.dataset.saveState === 'saving'`);
    const afterImeInput = await evaluate(page.command, PENDING);
    await waitFor(page.command, collectedGroups(2));
    const afterIme = await evaluate(page.command, PENDING);
    const pasteModifier = process.platform === "darwin" ? 4 : 2;
    await page.command("Runtime.evaluate", { expression: "navigator.clipboard.writeText(' EN')", awaitPromise: true });
    await focusEnd(page.command);
    await page.command("Input.dispatchKeyEvent", {
      type: "keyDown", key: "v", code: "KeyV", modifiers: pasteModifier, commands: ["Paste"],
    });
    await page.command("Input.dispatchKeyEvent", {
      type: "keyUp", key: "v", code: "KeyV", modifiers: pasteModifier,
    });
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_PASTE)} && document.querySelector('[data-save-state]')?.dataset.saveState === 'saving'`);
    const afterPasteInput = await evaluate(page.command, PENDING);
    await waitFor(page.command, collectedGroups(3));
    const settlePending = await evaluate(page.command, PENDING);
    const settledJournal = await evaluate(page.command, JOURNAL);
    await dumpJourney(page.command, "settle");
    await focusEnd(page.command);
    await page.command("Input.insertText", { text: "!" });
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_UNSETTLED)} && document.querySelector('[data-save-state]')?.dataset.saveState === 'saving'`);
    const interruptPending = await evaluate(page.command, PENDING);
    const unsettledJournal = await evaluate(page.command, JOURNAL);
    await waitFor(page.command, `(${JOURNAL}).then((journal) => journal.intents.some((intent) => intent.author_edit_unit !== undefined))`);
    await dumpJourney(page.command, "interrupt");
    await page.command("Page.reload", { ignoreCache: false });
    await waitFor(page.command, "document.querySelector('#app')?.dataset.bootState === 'project-ready'");
    await dumpJourney(page.command, "recovered-boot");
    await waitFor(page.command, `document.querySelector('textarea')?.value === ${JSON.stringify(AFTER_UNSETTLED)}`);
    const recoveredPending = await evaluate(page.command, PENDING);
    await waitFor(page.command, "document.querySelector('[data-save-state]')?.dataset.saveState === 'saved' && document.querySelector('textarea')?.readOnly === false");
    const recoveredSaved = {
      ...await evaluate(page.command, SURFACE),
      pending: await evaluate(page.command, PENDING),
    };
    const collectedJournal = await evaluate(page.command, JOURNAL);
    const authority = await projectAuthoritySnapshot();
    authority.activities = await projectActivities(apiOrigin, collectedJournal.intents[0].base_snapshot_id);
    assert.deepEqual(bindJourney({
      id: "S1-JRN-001",
      open,
      input,
      afterType,
      afterImeInput,
      afterIme,
      afterPasteInput,
      settle: { pending: settlePending, journal: settledJournal },
      interrupt: { pending: interruptPending, journal: unsettledJournal },
      recover: { pending: recoveredPending, saved: recoveredSaved, journal: collectedJournal },
      authority,
    }), {
      id: "S1-JRN-001",
      open: { ...PAGE, pending: pendingProjection(OPEN_BODY, "clean", 0, INITIAL_REVISION) },
      input: pendingProjection(AFTER_TYPE, "saving", 1, INITIAL_REVISION),
      afterType: pendingProjection(AFTER_TYPE, "saved", 0, "revision-1"),
      afterImeInput: pendingProjection(AFTER_IME, "saving", 1, "revision-1"),
      afterIme: pendingProjection(AFTER_IME, "saved", 0, "revision-2"),
      afterPasteInput: pendingProjection(AFTER_PASTE, "saving", 1, "revision-2"),
      settle: {
        pending: pendingProjection(AFTER_PASTE, "saved", 0, "revision-3"),
        journal: journalSnapshot(3),
      },
      interrupt: {
        pending: pendingProjection(AFTER_UNSETTLED, "saving", 1, "revision-3"),
        journal: journalSnapshot(3, true),
      },
      recover: {
        pending: pendingProjection(AFTER_UNSETTLED, "saving", 1, "revision-3"),
        saved: { ...PAGE, pending: pendingProjection(AFTER_UNSETTLED, "saved", 0, "revision-4") },
        journal: journalSnapshot(4),
      },
      authority: {
        receipts: [0, 1, 2, 3].map(appliedReceipt),
        effects: [0, 1, 2, 3].map(appliedEffect),
        activities: [0, 1, 2, 3].map(appliedActivity),
        manuscript: { revision_id: "revision-4", body: AFTER_UNSETTLED },
        cross_scope_receipts: 0,
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

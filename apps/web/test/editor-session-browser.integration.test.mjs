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
import { freezeOneIntentSubmission, openEditorWorkspace, persistReplaceSelection,
  rebuildPendingProjection, submitOnePendingAuthorEdit }
  from "/apps/web/src/editor-session.mjs";
import { digestJournalValue, readJournalSnapshot, validateJournalSnapshot }
  from "/apps/web/src/local-edit-journal.mjs";

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
    created_at: "2026-08-13T08:00:00.000Z" }
};
const freshSession = {
  ...session,
  schema_id: "storyos.query.editor-session.response.v1",
  correlation_id: "018f0000-0000-7001-8000-000000000037",
  base_snapshot: {
    ...session.base_snapshot,
    snapshot_id: "018f0000-0000-7001-8000-000000000038",
    project_activity_position: "1",
    authoritative_head_revision_id: "018f0000-0000-7001-8000-000000000034",
    materialized_revision: {
      revision_id: "018f0000-0000-7001-8000-000000000034", body: "Base!?",
    },
    materialized_payload_digest: {
      algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
    },
    created_at: "2026-08-15T08:00:00.000Z",
  },
};
const secondFreshSession = {
  ...freshSession,
  correlation_id: "018f0000-0000-7001-8000-000000000047",
  base_snapshot: {
    ...freshSession.base_snapshot,
    snapshot_id: "018f0000-0000-7001-8000-000000000048",
    project_activity_position: "2",
    authoritative_head_revision_id: "018f0000-0000-7001-8000-000000000044",
    materialized_revision: {
      revision_id: "018f0000-0000-7001-8000-000000000044", body: "Base!?+",
    },
    materialized_payload_digest: {
      algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: "342b8d7a51ca015a1e91d09858779299e742f72020ae6c5441cd6101d69720cc",
    },
    created_at: "2026-08-15T08:01:00.000Z",
  },
};
const requestPaths = [];
const authorEditRequests = [];
const challengeRequests = [];
let failAuthorEditChallenge = true;
let corruptAuthorEditAcknowledgement = true;
let authorEditDigest;
let canonicalSession = session;
let nextZeroAuthorityResult;
const jsonResponse = (body) => new Response(JSON.stringify(body), {
  status: 200, headers: { "content-type": "application/json" },
});
const fetchImpl = async (url, options = {}) => {
  const path = new URL(url).pathname;
  requestPaths.push(path);
  if (path.endsWith("/anti-forgery-challenges")) {
    const request = JSON.parse(options.body);
    challengeRequests.push(request);
    if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
      authorEditDigest = request.canonical_command_digest;
      if (failAuthorEditChallenge) {
        failAuthorEditChallenge = false;
        throw new Error("simulated pre-send challenge failure");
      }
    }
    return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-13T08:05:00.000Z",
      limit_profile_revision: "storyos.foundation.absolute.v1" });
  }
  if (path.endsWith("/editor-sessions")) return jsonResponse(session);
  if (path.endsWith("/editor-sessions/" + SESSION)) return jsonResponse({
    ...canonicalSession, schema_id: "storyos.query.editor-session.response.v1",
  });
  if (path.endsWith("/manuscript/author-edits")) {
    const request = JSON.parse(options.body);
    const zeroAuthorityResult = nextZeroAuthorityResult;
    const zeroEffect = zeroAuthorityResult !== undefined;
    const secondEdit = request.expected_authoritative_revision_id
      === freshSession.base_snapshot.authoritative_head_revision_id;
    const revisionId = secondEdit
      ? "018f0000-0000-7001-8000-000000000044"
      : "018f0000-0000-7001-8000-000000000034";
    const commitId = secondEdit
      ? "018f0000-0000-7001-8000-000000000045"
      : "018f0000-0000-7001-8000-000000000035";
    const zeroIds = {
      no_effect: ["018f0000-0000-7001-8000-000000000051",
        "018f0000-0000-7001-8000-000000000052", "018f0000-0000-7001-8000-000000000053"],
      conflicted: ["018f0000-0000-7001-8000-000000000061",
        "018f0000-0000-7001-8000-000000000062", "018f0000-0000-7001-8000-000000000063"],
      refused: ["018f0000-0000-7001-8000-000000000071",
        "018f0000-0000-7001-8000-000000000072", "018f0000-0000-7001-8000-000000000073"],
    };
    const commandId = zeroEffect
      ? zeroIds[zeroAuthorityResult][0]
      : secondEdit
      ? "018f0000-0000-7001-8000-000000000041"
      : "018f0000-0000-7001-8000-000000000031";
    const admissionId = zeroEffect
      ? zeroIds[zeroAuthorityResult][1]
      : secondEdit
      ? "018f0000-0000-7001-8000-000000000042"
      : "018f0000-0000-7001-8000-000000000032";
    const receiptId = zeroEffect
      ? zeroIds[zeroAuthorityResult][2]
      : secondEdit
      ? "018f0000-0000-7001-8000-000000000043"
      : "018f0000-0000-7001-8000-000000000033";
    const finalBody = secondEdit ? "Base!?+" : "Base!?";
    const position = secondEdit ? "2" : "1";
    authorEditRequests.push({
      body: options.body,
      idempotencyKey: options.headers["idempotency-key"],
    });
    const responseDigest = corruptAuthorEditAcknowledgement
      ? { ...authorEditDigest, value_hex_lowercase: "0".repeat(64) }
      : authorEditDigest;
    corruptAuthorEditAcknowledgement = false;
    if (!zeroEffect) canonicalSession = secondEdit ? secondFreshSession : freshSession;
    const zeroCurrentHead = zeroAuthorityResult === "conflicted"
      ? "018f0000-0000-7001-8000-000000000064"
      : request.expected_authoritative_revision_id;
    const zeroReason = zeroAuthorityResult === "no_effect"
      ? "content_unchanged"
      : zeroAuthorityResult === "conflicted" ? "stale_authoritative_head" : "invalid_selection";
    return jsonResponse({
      schema_id: "storyos.command.apply-author-edit.response.v2",
      correlation_id: request.correlation_id,
      project_scope: project.project_scope,
      command_id: commandId,
      author_command_admission_id: admissionId,
      receipt: {
        receipt_id: receiptId,
        project_scope: project.project_scope, command_kind: "applyAuthorEdit",
        command_digest: responseDigest,
        idempotency_key: options.headers["idempotency-key"],
        producer_cause: "author_command_admission",
        author_command_admission_id: admissionId,
        expected_heads: [request.expected_authoritative_revision_id],
        prior_heads: [zeroEffect ? zeroCurrentHead : request.expected_authoritative_revision_id],
        resulting_heads: zeroEffect
          ? [zeroCurrentHead] : [revisionId],
        authoritative_revision_ids: zeroEffect ? [] : [revisionId],
        proposal_revision_ids: [],
        authoritative_commit_ids: zeroEffect ? [] : [commitId],
        author_action_sequence: zeroEffect ? null : position, draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [], condition_refs: [],
        result: zeroEffect ? zeroAuthorityResult : "authoritative_applied",
        created_at: "2026-08-15T08:00:00.000Z",
      },
      effect: zeroEffect
        ? zeroAuthorityResult === "conflicted"
          ? { kind: "conflicted", reason: zeroReason,
            current_authoritative_revision_id: zeroCurrentHead }
          : { kind: zeroAuthorityResult, reason: zeroReason }
        : { kind: "authoritative_applied",
          authoritative_revision: { revision_id: revisionId, body: finalBody },
          authoritative_commit_id: commitId,
          author_action_sequence: position, project_activity_position: position },
      completed_intent_record_id: request.completed_intent_record_id,
      local_intent_sequence: request.local_intent_sequence,
    });
  }
  if (path.includes("/manuscript/author-edit-outcomes/")) {
    const last = authorEditRequests.at(-1);
    if (!last || options.method !== "GET" || options.body !== undefined) {
      throw new Error("Unexpected outcome Query");
    }
    const request = JSON.parse(last.body);
    const secondEdit = request.expected_authoritative_revision_id
      === freshSession.base_snapshot.authoritative_head_revision_id;
    const revisionId = secondEdit
      ? "018f0000-0000-7001-8000-000000000044"
      : "018f0000-0000-7001-8000-000000000034";
    const commitId = secondEdit
      ? "018f0000-0000-7001-8000-000000000045"
      : "018f0000-0000-7001-8000-000000000035";
    const commandId = secondEdit
      ? "018f0000-0000-7001-8000-000000000041"
      : "018f0000-0000-7001-8000-000000000031";
    const admissionId = secondEdit
      ? "018f0000-0000-7001-8000-000000000042"
      : "018f0000-0000-7001-8000-000000000032";
    const receiptId = secondEdit
      ? "018f0000-0000-7001-8000-000000000043"
      : "018f0000-0000-7001-8000-000000000033";
    const finalBody = secondEdit ? "Base!?+" : "Base!?";
    const position = secondEdit ? "2" : "1";
    canonicalSession = secondEdit ? secondFreshSession : freshSession;
    return jsonResponse({
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000081",
      project_scope: project.project_scope,
      outcome: {
        outcome_kind: "committed",
        response: {
          schema_id: "storyos.command.apply-author-edit.response.v2",
          correlation_id: request.correlation_id,
          project_scope: project.project_scope,
          command_id: commandId,
          author_command_admission_id: admissionId,
          receipt: {
            receipt_id: receiptId,
            project_scope: project.project_scope, command_kind: "applyAuthorEdit",
            command_digest: authorEditDigest,
            idempotency_key: last.idempotencyKey,
            producer_cause: "author_command_admission",
            author_command_admission_id: admissionId,
            expected_heads: [request.expected_authoritative_revision_id],
            prior_heads: [request.expected_authoritative_revision_id],
            resulting_heads: [revisionId],
            authoritative_revision_ids: [revisionId],
            proposal_revision_ids: [],
            authoritative_commit_ids: [commitId],
            author_action_sequence: position, draft_artifact_refs: [],
            artifact_lifecycle_event_refs: [], condition_refs: [],
            result: "authoritative_applied",
            created_at: "2026-08-15T08:00:00.000Z",
          },
          effect: {
            kind: "authoritative_applied",
            authoritative_revision: { revision_id: revisionId, body: finalBody },
            authoritative_commit_id: commitId,
            author_action_sequence: position, project_activity_position: position,
          },
          completed_intent_record_id: request.completed_intent_record_id,
          local_intent_sequence: request.local_intent_sequence,
        },
      },
    });
  }
  if (path.endsWith("/chapters/" + CHAPTER)) return jsonResponse({
    schema_id: "storyos.query.chapter.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000036",
    project_scope: project.project_scope,
    project_activity_position: "1",
    chapter: { chapter_id: CHAPTER, title: "Chapter",
      current_revision: { revision_id: "018f0000-0000-7001-8000-000000000034",
        body: "Base!?" } },
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
const journalDatabaseName = "storyos-local-edit-journal:" + OWNER + ":" + PROJECT;
const deleteJournalDatabase = async (workspace) => {
  workspace.database.close();
  const request = indexedDB.deleteDatabase(journalDatabaseName);
  await requestResult(request);
};
const chapterForSession = (editorSession) => ({
  project_scope: project.project_scope,
  project_activity_position: editorSession.base_snapshot.project_activity_position,
  chapter: {
    chapter_id: CHAPTER,
    current_revision: editorSession.base_snapshot.materialized_revision,
  },
});
const openCurrentWorkspace = () => openEditorWorkspace({
  baseUrl: location.origin,
  project,
  chapter: chapterForSession(canonicalSession),
  profile,
  fetchImpl,
  indexedDBImpl: indexedDB,
  cryptoImpl: crypto,
});
const intentCount = (database) => requestResult(database.transaction("intents", "readonly")
  .objectStore("intents").count());

async function run() {
  let workspace = await openEditorWorkspace({ baseUrl: location.origin, project, chapter, profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  assert.equal(workspace.kind, "editor-ready");
  {
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
      inputOrigin: "typing", undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    assert.deepEqual(pending, { body: "Base!", save_state: "saving",
      unsettled_intent_count: 1, authoritative_revision_id: REVISION });
    await assert.rejects(persistReplaceSelection(reopened, {
      from: 5, to: 5, text: "?", resultingBody: "Base!?",
      inputOrigin: "typing", undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.251Z",
    }), /requires prior group settlement/);
    assert.equal(await intentCount(reopened.database), 1);
    await deleteJournalDatabase(reopened);
    const regrouped = await openCurrentWorkspace();
    await persistReplaceSelection(regrouped, {
      from: 4, to: 4, text: "!", resultingBody: "Base!",
      inputOrigin: "typing", undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    const secondPending = await persistReplaceSelection(regrouped, {
      from: 5, to: 5, text: "?", resultingBody: "Base!?",
      inputOrigin: "typing", undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.250Z",
    });
    regrouped.pending = secondPending;
    assert.equal(await intentCount(regrouped.database), 2);
    assert.equal(requestPaths.some((path) => path.includes("author-edits")), false);
    workspace = regrouped;
  }

  assert.equal(workspace.session.editor_session.editor_session_id, SESSION);
  assert.deepEqual(workspace.pending, { body: "Base!?", save_state: "saving",
    unsettled_intent_count: 2, authoritative_revision_id: REVISION });
  assert.equal(await intentCount(workspace.database), 2);
  assert.equal(requestPaths.some((path) => path.includes("author-edits")), false);

  await assert.rejects(submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
    fetchImpl, cryptoImpl: crypto }), /simulated pre-send challenge failure/);
  await assert.rejects(submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
    fetchImpl, cryptoImpl: crypto }), /acknowledgement does not converge/);
  const settled = await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
    fetchImpl, cryptoImpl: crypto });
  assert.deepEqual(settled, { body: "Base!?", save_state: "saved",
    unsettled_intent_count: 0,
    authoritative_revision_id: "018f0000-0000-7001-8000-000000000034" });
  assert.equal(authorEditRequests.length, 1);
  assert.deepEqual(JSON.parse(authorEditRequests[0].body).author_edit_units, [{
    normalized_primitives: [{ kind: "replace_selection", from: 4, to: 4, text: "!" }],
    selection_snapshot: {
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 4, to: 4,
    },
  }, {
    normalized_primitives: [{ kind: "replace_selection", from: 5, to: 5, text: "?" }],
    selection_snapshot: {
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 5, to: 5,
    },
  }]);
  const editChallenges = challengeRequests.filter((request) =>
    request.command_schema === "storyos.command.apply-author-edit.request.v1");
  assert.equal(editChallenges.length, 2);
  assert.equal(editChallenges[0].idempotency_key, editChallenges[1].idempotency_key);
  assert.deepEqual(editChallenges[0].canonical_command_digest,
    editChallenges[1].canonical_command_digest);
  assert.deepEqual(workspace.session, freshSession);

  const secondPending = await persistReplaceSelection(workspace, {
    from: 6, to: 6, text: "+", resultingBody: "Base!?+",
    inputOrigin: "paste", undoGroupId: "018f0000-0000-7001-8000-000000000049",
    createdAt: "2026-08-15T08:00:00.500Z",
  });
  assert.deepEqual(secondPending, { body: "Base!?+", save_state: "saving",
    unsettled_intent_count: 1,
    authoritative_revision_id: "018f0000-0000-7001-8000-000000000034" });
  const secondSettled = await submitOnePendingAuthorEdit({
    workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
  });
  assert.deepEqual(secondSettled, { body: "Base!?+", save_state: "saved",
    unsettled_intent_count: 0,
    authoritative_revision_id: "018f0000-0000-7001-8000-000000000044" });
  assert.deepEqual(workspace.session, secondFreshSession);
  const finalJournal = await validateJournalSnapshot(
    workspace, await readJournalSnapshot(workspace),
  );
  const expectedRecord = (record, { sequence, origin, base, unit, undoGroupId, createdAt }) => ({
    completed_intent_record_id: record.completed_intent_record_id,
    local_intent_sequence: sequence,
    journal_partition_id: workspace.partition.journal_partition_id,
    project_scope: project.project_scope,
    editor_session_id: SESSION,
    writer_generation: "1",
    limit_profile_revision: "storyos.foundation.absolute.v1",
    batch_policy_revision: "storyos.author-edit-batch.release-1.preview.v1",
    input_origin: origin,
    chapter_object_id: CHAPTER,
    base_snapshot_id: base.snapshot_id,
    base_activity_position: base.project_activity_position,
    target_refs: base.target_refs,
    expected_authoritative_heads: [base.authoritative_head_revision_id],
    expected_proposal_heads: [],
    proposal_anchors: [],
    observed_ownership_partition: "authoritative",
    author_edit_unit: unit,
    retry_source: { kind: "fresh_editor_intent" },
    editor_contract_revision: "storyos.editor-contract.release-1.v2",
    undo_group_binding: { kind: "direct_author_input", undo_group_id: undoGroupId },
    payload_chain_ref: record.payload_chain_ref,
    payload_digest: record.payload_digest,
    projection_dependency: { snapshot_id: base.snapshot_id, prior_sequence: sequence - 1 },
    created_at: createdAt,
  });
  const expectedRecords = [
    expectedRecord(finalJournal.records[0], {
      sequence: 1, origin: "typing", base: session.base_snapshot,
      unit: {
        normalized_primitives: [{ kind: "replace_selection", from: 4, to: 4, text: "!" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 4, to: 4,
        },
      },
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    }),
    expectedRecord(finalJournal.records[1], {
      sequence: 2, origin: "typing", base: session.base_snapshot,
      unit: {
        normalized_primitives: [{ kind: "replace_selection", from: 5, to: 5, text: "?" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 5, to: 5,
        },
      },
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.250Z",
    }),
    expectedRecord(finalJournal.records[2], {
      sequence: 3, origin: "paste", base: freshSession.base_snapshot,
      unit: {
        normalized_primitives: [{ kind: "replace_selection", from: 6, to: 6, text: "+" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 6, to: 6,
        },
      },
      undoGroupId: "018f0000-0000-7001-8000-000000000049",
      createdAt: "2026-08-15T08:00:00.500Z",
    }),
  ];
  assert.deepEqual(finalJournal.records, expectedRecords);
  const expectedPayloadChains = [];
  for (const chain of finalJournal.payloadChains) {
    const records = expectedRecords.filter((record) =>
      record.payload_chain_ref === chain.payload_chain_id);
    const base = records[0].base_snapshot_id === session.base_snapshot.snapshot_id
      ? session.base_snapshot : freshSession.base_snapshot;
    expectedPayloadChains.push({
      payload_chain_id: chain.payload_chain_id,
      journal_partition_id: workspace.partition.journal_partition_id,
      checkpoint_ref: {
        chapter_object_id: CHAPTER,
        materialized_payload: base.materialized_revision.body,
        materialized_payload_digest: base.materialized_payload_digest,
        source_snapshot_id: base.snapshot_id,
        source_heads: [base.authoritative_head_revision_id],
      },
      ordered_patch_refs: await Promise.all(records.map(async (record, index) => ({
        patch_id: chain.ordered_patch_refs[index].patch_id,
        completed_intent_record_id: record.completed_intent_record_id,
        local_intent_sequence: record.local_intent_sequence,
        normalized_primitives: record.author_edit_unit.normalized_primitives,
        resulting_payload_digest: await digestJournalValue(
          finalJournal.bodyBySequence.get(record.local_intent_sequence), crypto,
        ),
      }))),
    });
  }
  assert.deepEqual(finalJournal.payloadChains, expectedPayloadChains);
  const expectedAppliedSettlement = (group, {
    commandId, admissionId, receiptId, priorHead, revision, commitId, sequence,
    position, installedBase,
  }) => ({
    kind: "applied_receipt_settled",
    command_id: commandId,
    author_command_admission_id: admissionId,
    receipt: {
      receipt_id: receiptId,
      project_scope: project.project_scope,
      command_kind: "applyAuthorEdit",
      command_digest: group.frozen_request_digest,
      idempotency_key: group.idempotency_key,
      producer_cause: "author_command_admission",
      author_command_admission_id: admissionId,
      expected_heads: [priorHead], prior_heads: [priorHead],
      resulting_heads: [revision.revision_id],
      authoritative_revision_ids: [revision.revision_id], proposal_revision_ids: [],
      authoritative_commit_ids: [commitId], author_action_sequence: position,
      draft_artifact_refs: [], artifact_lifecycle_event_refs: [], condition_refs: [],
      result: "authoritative_applied", created_at: "2026-08-15T08:00:00.000Z",
    },
    authoritative_revision: revision,
    authoritative_commit_id: commitId,
    author_action_sequence: position,
    project_activity_position: position,
    installed_base_snapshot: installedBase,
  });
  const expectedGroup = (group, records, settlement) => ({
    journal_submission_group_id: group.journal_submission_group_id,
    journal_partition_id: workspace.partition.journal_partition_id,
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
    settlement,
    frozen_at: group.frozen_at,
  });
  const expectedGroups = [
    expectedGroup(finalJournal.groups[0], expectedRecords.slice(0, 2),
      expectedAppliedSettlement(finalJournal.groups[0], {
        commandId: "018f0000-0000-7001-8000-000000000031",
        admissionId: "018f0000-0000-7001-8000-000000000032",
        receiptId: "018f0000-0000-7001-8000-000000000033",
        priorHead: REVISION,
        revision: freshSession.base_snapshot.materialized_revision,
        commitId: "018f0000-0000-7001-8000-000000000035",
        position: "1", installedBase: freshSession.base_snapshot,
      })),
    expectedGroup(finalJournal.groups[1], expectedRecords.slice(2),
      expectedAppliedSettlement(finalJournal.groups[1], {
        commandId: "018f0000-0000-7001-8000-000000000041",
        admissionId: "018f0000-0000-7001-8000-000000000042",
        receiptId: "018f0000-0000-7001-8000-000000000043",
        priorHead: freshSession.base_snapshot.authoritative_head_revision_id,
        revision: secondFreshSession.base_snapshot.materialized_revision,
        commitId: "018f0000-0000-7001-8000-000000000045",
        position: "2", installedBase: secondFreshSession.base_snapshot,
      })),
  ];
  assert.deepEqual(finalJournal.groups, expectedGroups);
  assert.deepEqual(finalJournal.watermark, {
    key: "durable_high_watermark:" + workspace.partition.journal_partition_id, value: 3,
  });
  assert.deepEqual(finalJournal.activeBase, secondFreshSession.base_snapshot);
  assert.deepEqual([...finalJournal.bodyBySequence.entries()], [
    [1, "Base!"], [2, "Base!?"], [3, "Base!?+"],
  ]);
  assert.deepEqual([...finalJournal.covered], [1, 2, 3]);

  const challengeCountBeforeCoverageFailures = challengeRequests.length;
  const replaceGroups = async (groups) => {
    const transaction = workspace.database.transaction("submission_groups", "readwrite");
    const store = transaction.objectStore("submission_groups");
    store.clear();
    for (const group of groups) store.put(group);
    await transactionResult(transaction);
  };
  const replaceRecords = async (records) => {
    const transaction = workspace.database.transaction("intents", "readwrite");
    const store = transaction.objectStore("intents");
    store.clear();
    for (const record of records) store.put(record);
    await transactionResult(transaction);
  };
  const originalGroups = finalJournal.groups.map((group) => JSON.parse(JSON.stringify(group)));
  const coverageFailures = [
    (groups) => { groups.shift(); },
    (groups) => { groups[0].ordered_coverage.pop(); },
    (groups) => { groups[0].ordered_coverage[1] = groups[0].ordered_coverage[0]; },
    (groups) => { groups[0].ordered_coverage[1].local_intent_sequence = 3; },
    (groups) => { groups[0].ordered_coverage.reverse(); },
    (groups) => {
      groups[1].covered_sequence_range.first = 2;
      groups[1].ordered_coverage.unshift(groups[0].ordered_coverage[1]);
    },
    (groups) => { groups[1].batch_policy_revision = "storyos.author-edit-batch.invalid"; },
    (groups) => {
      groups[1].frozen_request_body.editor_contract_revision =
        "storyos.editor-contract.release-1.invalid";
    },
    (groups) => { groups[0].frozen_payload_coverage_digest.value_hex_lowercase = "0".repeat(64); },
  ];
  for (const corrupt of coverageFailures) {
    const groups = originalGroups.map((group) => JSON.parse(JSON.stringify(group)));
    corrupt(groups);
    await replaceGroups(groups);
    await assert.rejects(submitOnePendingAuthorEdit({
      workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    }), /corrupt/);
    assert.equal(challengeRequests.length, challengeCountBeforeCoverageFailures);
    await replaceGroups(originalGroups);
  }

  await assert.rejects(persistReplaceSelection(workspace, {
    from: 5, to: 5, text: "?", resultingBody: "x".repeat(1048577),
  }), /limit failed/);
  assert.equal(await intentCount(workspace.database), 3);

  workspace.partition.disposition = "read_only_observer";
  await assert.rejects(persistReplaceSelection(workspace, {
    from: 5, to: 5, text: "?", resultingBody: "Base!?",
  }), /read only/);
  workspace.partition.disposition = "current_writer_open";
  assert.equal(await intentCount(workspace.database), 3);

  workspace.pending = await persistReplaceSelection(workspace, {
    from: 7, to: 7, text: "Z", resultingBody: "Base!?+Z",
    inputOrigin: "typing", undoGroupId: "018f0000-0000-7001-8000-000000000050",
    createdAt: "2026-08-15T08:00:00.501Z",
  });
  const policyJournal = await validateJournalSnapshot(
    workspace, await readJournalSnapshot(workspace),
  );
  const originalRecords = policyJournal.records.map((record) => JSON.parse(JSON.stringify(record)));
  const historicalPolicyFailures = [
    (records) => { records[1].created_at = "2026-08-15T08:00:00.251Z"; },
    (records) => { records[1].input_origin = "paste"; },
    (records) => {
      records[1].undo_group_binding.undo_group_id = "018f0000-0000-7001-8000-000000000099";
    },
  ];
  const challengeCountBeforeHistoricalPolicyFailures = challengeRequests.length;
  for (const corrupt of historicalPolicyFailures) {
    const records = originalRecords.map((record) => JSON.parse(JSON.stringify(record)));
    corrupt(records);
    await replaceRecords(records);
    await assert.rejects(submitOnePendingAuthorEdit({
      workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    }), /corrupt/);
    assert.equal(challengeRequests.length, challengeCountBeforeHistoricalPolicyFailures);
    await replaceRecords(originalRecords);
  }

  await deleteJournalDatabase(workspace);
  workspace = await openCurrentWorkspace();
  assert.equal(workspace.kind, "editor-ready");
  const limitUndoGroupId = "018f0000-0000-7001-8000-000000000081";
  let limitBody = canonicalSession.base_snapshot.materialized_revision.body;
  for (let index = 0; index < 240; index += 1) {
    const nextBody = limitBody + "a";
    workspace.pending = await persistReplaceSelection(workspace, {
      from: limitBody.length, to: limitBody.length, text: "a", resultingBody: nextBody,
      inputOrigin: "typing", undoGroupId: limitUndoGroupId,
      createdAt: new Date(Date.parse("2026-08-15T09:00:00.000Z") + index).toISOString(),
    });
    limitBody = nextBody;
  }
  assert.equal(await intentCount(workspace.database), 240);
  await assert.rejects(persistReplaceSelection(workspace, {
    from: limitBody.length, to: limitBody.length, text: "a", resultingBody: limitBody + "a",
    inputOrigin: "typing", undoGroupId: limitUndoGroupId,
    createdAt: "2026-08-15T09:00:00.240Z",
  }), /limit failed/);
  const challengeCountBeforeLimitFreeze = challengeRequests.length;
  const limitGroup = await freezeOneIntentSubmission(workspace, crypto);
  assert.deepEqual(limitGroup.covered_sequence_range, { first: 1, last: 240 });
  assert.equal(limitGroup.ordered_coverage.length, 240);
  assert.equal(limitGroup.frozen_request_body.author_edit_units.length, 240);
  assert.equal(challengeRequests.length, challengeCountBeforeLimitFreeze);
  await deleteJournalDatabase(workspace);

  const zeroCases = [{
    result: "no_effect", text: "", suffix: "", reason: "content_unchanged",
    commandId: "018f0000-0000-7001-8000-000000000051",
    admissionId: "018f0000-0000-7001-8000-000000000052",
    receiptId: "018f0000-0000-7001-8000-000000000053",
  }, {
    result: "conflicted", text: "C", suffix: "C", reason: "stale_authoritative_head",
    commandId: "018f0000-0000-7001-8000-000000000061",
    admissionId: "018f0000-0000-7001-8000-000000000062",
    receiptId: "018f0000-0000-7001-8000-000000000063",
  }, {
    result: "refused", text: "R", suffix: "R", reason: "invalid_selection",
    commandId: "018f0000-0000-7001-8000-000000000071",
    admissionId: "018f0000-0000-7001-8000-000000000072",
    receiptId: "018f0000-0000-7001-8000-000000000073",
  }];
  for (const [index, zeroCase] of zeroCases.entries()) {
    workspace = await openCurrentWorkspace();
    assert.equal(workspace.kind, "editor-ready");
    const baseBeforeZero = JSON.parse(JSON.stringify(workspace.session));
    const baseBody = baseBeforeZero.base_snapshot.materialized_revision.body;
    const localBody = baseBody + zeroCase.suffix;
    workspace.pending = await persistReplaceSelection(workspace, {
      from: baseBody.length, to: baseBody.length, text: zeroCase.text,
      resultingBody: localBody,
      inputOrigin: zeroCase.result === "no_effect" ? "deletion" : "typing",
      undoGroupId: "018f0000-0000-7001-8000-00000000008" + (index + 2),
      createdAt: "2026-08-15T10:00:00.00" + index + "Z",
    });
    assert.deepEqual(workspace.pending, { body: localBody, save_state: "saving",
      unsettled_intent_count: 1,
      authoritative_revision_id: baseBeforeZero.base_snapshot.authoritative_head_revision_id });
    const challengeCountBeforeZero = challengeRequests.length;
    nextZeroAuthorityResult = zeroCase.result;
    const projection = await submitOnePendingAuthorEdit({
      workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
    });
    assert.deepEqual(projection, { body: localBody, save_state: "needs_attention",
      unsettled_intent_count: 1,
      authoritative_revision_id: baseBeforeZero.base_snapshot.authoritative_head_revision_id });
    assert.equal(challengeRequests.length, challengeCountBeforeZero + 1);
    assert.deepEqual(workspace.session, baseBeforeZero);
    const zeroJournal = await validateJournalSnapshot(
      workspace, await readJournalSnapshot(workspace),
    );
    assert.equal(zeroJournal.records.length, 1);
    assert.equal(zeroJournal.payloadChains.length, 1);
    assert.equal(zeroJournal.groups.length, 1);
    assert.deepEqual(zeroJournal.activeBase, baseBeforeZero.base_snapshot);
    assert.deepEqual([...zeroJournal.bodyBySequence.entries()], [[1, localBody]]);
    assert.deepEqual([...zeroJournal.covered], [1]);
    const group = zeroJournal.groups[0];
    const currentHead = zeroCase.result === "conflicted"
      ? "018f0000-0000-7001-8000-000000000064"
      : baseBeforeZero.base_snapshot.authoritative_head_revision_id;
    const expectedEffect = zeroCase.result === "conflicted"
      ? { kind: "conflicted", reason: zeroCase.reason,
        current_authoritative_revision_id: currentHead }
      : { kind: zeroCase.result, reason: zeroCase.reason };
    assert.deepEqual(group.settlement, {
      kind: "zero_authority_receipt_settled",
      command_id: zeroCase.commandId,
      author_command_admission_id: zeroCase.admissionId,
      receipt: {
        receipt_id: zeroCase.receiptId,
        project_scope: project.project_scope,
        command_kind: "applyAuthorEdit",
        command_digest: group.frozen_request_digest,
        idempotency_key: group.idempotency_key,
        producer_cause: "author_command_admission",
        author_command_admission_id: zeroCase.admissionId,
        expected_heads: [baseBeforeZero.base_snapshot.authoritative_head_revision_id],
        prior_heads: [currentHead], resulting_heads: [currentHead],
        authoritative_revision_ids: [], proposal_revision_ids: [],
        authoritative_commit_ids: [], author_action_sequence: null,
        draft_artifact_refs: [], artifact_lifecycle_event_refs: [], condition_refs: [],
        result: zeroCase.result, created_at: "2026-08-15T08:00:00.000Z",
      },
      effect: expectedEffect,
    });
    if (index < zeroCases.length - 1) await deleteJournalDatabase(workspace);
  }

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
  const recovery = await openCurrentWorkspace();
  assert.equal(recovery.kind, "editor-read-only-recovery");
  document.body.dataset.result = "pass";
  document.body.textContent = "bounded IndexedDB journal passed";
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
    signal: AbortSignal.timeout(5_000),
  }).then((response) => response.json());
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await Promise.race([new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  }), new Promise((_, reject) => setTimeout(
    () => reject(new Error("Chrome DevTools WebSocket timed out")), 5_000,
  ))]);
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
    const id = commandId;
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`Chrome DevTools command timed out: ${method}`));
    }, 5_000);
    pending.set(id, {
      resolve(value) {
        clearTimeout(timeout);
        resolve(value);
      },
      reject(error) {
        clearTimeout(timeout);
        reject(error);
      },
    });
    socket.send(JSON.stringify({ id: commandId, method, params }));
  });
  await command("Runtime.enable");
  const deadline = Date.now() + 90_000;
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

test("a real browser journal settles bounded groups and installs the complete next base", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable",
  timeout: 120_000,
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
      || pathname === "/apps/web/src/author-edit-outcome-reconciliation.mjs"
      || pathname === "/apps/web/src/local-edit-journal.mjs"
      || pathname === "/apps/web/src/protected-transport-capsule.mjs"
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
    assert.equal(state.text, "bounded IndexedDB journal passed");
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

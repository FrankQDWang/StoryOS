import { expect, it } from "vitest";

import type {
  ApplyAuthorEditEffect,
  ApplyAuthorEditResponse,
  DigestValue,
  DomainReceiptResult,
  GetApplyAuthorEditOutcomeResponse,
  GetChapterResponse,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  freezeOneIntentSubmission,
  openEditorWorkspace,
  persistReplaceSelection,
  rebuildPendingProjection,
  submitOnePendingAuthorEdit,
} from "../../src/editor-session.ts";
import type {
  EditorReadyState,
  JournalIntentRecord,
  JournalPayloadChain,
  JournalSubmissionGroup,
  SubmissionSettlement,
} from "../../src/editor-types.ts";
import {
  digestJournalValue,
  readJournalSnapshot,
  validateJournalSnapshot,
} from "../../src/local-edit-journal.ts";
import { withDigest } from "./local-edit-journal-append-fixture.ts";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  chapterRevision,
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requestHeaders,
  requestResult,
  requireDigestValue,
  requireEditorReady,
  requireRequestBody,
  requireString,
} from "./scenario.ts";

type ZeroAuthorityResult = Exclude<DomainReceiptResult, "authoritative_applied">;

const FIRST_REVISION = "018f0000-0000-7001-8000-000000000034";
const SECOND_REVISION = "018f0000-0000-7001-8000-000000000044";

it("preserves the complete Journal when a retained Session Snapshot does not match the Chapter", async () => {
  const scenario = createBrowserScenario();
  const activeSessionKey = `active_session:${OWNER}:${PROJECT}`;
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(String(input)).pathname;
    requests.push(`${init?.method ?? "GET"} ${path}`);
    if (path === `/api/v1/projects/${PROJECT}/editor-sessions/${SESSION}`
      && (init?.method ?? "GET") === "GET") {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    throw new Error(`unexpected request: ${path}`);
  };
  let database: IDBDatabase | undefined;
  await deleteJournal(scenario.journalName);
  sessionStorage.setItem(activeSessionKey, SESSION);
  try {
    const input = { baseUrl: location.origin, project: scenario.project,
      chapter: scenario.chapter, profile: scenario.profile, fetchImpl };
    const workspace = await openEditorWorkspace(input);
    requireEditorReady(workspace);
    const journal = workspace.database;
    database = journal;
    expect(workspace.session.base_snapshot.project_activity_position).toBe("0");
    await persistReplaceSelection(workspace,
      { from: 4, to: 4, text: " retained", resultingBody: "Base retained" });
    const stores = Array.from(journal.objectStoreNames);
    async function readCompleteJournal() {
      const transaction = journal.transaction(stores, "readonly");
      return Promise.all(stores.map((name) => requestResult(transaction.objectStore(name).getAll())));
    }
    const before = await readCompleteJournal();
    requests.length = 0;
    const reopened = await openEditorWorkspace({ ...input,
      chapter: { ...scenario.chapter, project_activity_position: "3" } });
    if (reopened.kind === "editor-ready") reopened.database.close();
    expect(reopened).toMatchObject({ kind: "editor-read-only-recovery", code: "local_journal_unavailable" });
    expect(await readCompleteJournal()).toEqual(before);
    expect(sessionStorage.getItem(activeSessionKey)).toBe(SESSION);
    expect(requests).toEqual([`GET /api/v1/projects/${PROJECT}/editor-sessions/${SESSION}`]);
  } finally {
    database?.close();
    sessionStorage.removeItem(activeSessionKey);
    await deleteJournal(scenario.journalName);
  }
});

function transactionResult(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(
      transaction.error ?? new Error("IndexedDB transaction aborted"),
    );
    transaction.onerror = () => reject(
      transaction.error ?? new Error("IndexedDB transaction failed"),
    );
  });
}

function groupAt(groups: JournalSubmissionGroup[], index: number): JournalSubmissionGroup {
  const group = groups[index];
  if (!group) throw new Error(`Journal Submission Group ${index} is unavailable`);
  return group;
}

function recordAt(records: JournalIntentRecord[], index: number): JournalIntentRecord {
  const record = records[index];
  if (!record) throw new Error(`Journal intent ${index} is unavailable`);
  return record;
}

function requireAuthorEditUnit(
  record: JournalIntentRecord,
): NonNullable<JournalIntentRecord["author_edit_unit"]> {
  if (record.author_edit_unit === undefined) {
    throw new Error(`Journal intent ${record.local_intent_sequence} has no Author Edit Unit`);
  }
  return record.author_edit_unit;
}

it("keeps the bounded IndexedDB Journal valid through batching and settlement", async () => {
  const scenario = createBrowserScenario();
  const freshSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000037",
    base_snapshot: {
      ...scenario.session.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000038",
      project_activity_position: "1",
      authoritative_head_revision_id: FIRST_REVISION,
      materialized_revision: chapterRevision(FIRST_REVISION, "Base!?"),
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
      },
      created_at: "2026-08-15T08:00:00.000Z",
    },
  };
  const secondFreshSession: GetEditorSessionResponse = {
    ...freshSession,
    correlation_id: "018f0000-0000-7001-8000-000000000047",
    base_snapshot: {
      ...freshSession.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000048",
      project_activity_position: "2",
      authoritative_head_revision_id: SECOND_REVISION,
      materialized_revision: chapterRevision(SECOND_REVISION, "Base!?+"),
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "342b8d7a51ca015a1e91d09858779299e742f72020ae6c5441cd6101d69720cc",
      },
      created_at: "2026-08-15T08:01:00.000Z",
    },
  };
  const requestPaths: string[] = [];
  const authorEditRequests: Array<{
    readonly idempotencyKey: string;
    readonly request: Record<string, unknown>;
  }> = [];
  const challengeRequests: Record<string, unknown>[] = [];
  let failAuthorEditChallenge = true;
  let corruptAuthorEditAcknowledgement = true;
  let authorEditDigest: DigestValue | undefined;
  let canonicalSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
  };
  let nextZeroAuthorityResult: ZeroAuthorityResult | undefined;
  const zeroIds: Record<ZeroAuthorityResult, readonly [string, string, string]> = {
    no_effect: [
      "018f0000-0000-7001-8000-000000000051",
      "018f0000-0000-7001-8000-000000000052",
      "018f0000-0000-7001-8000-000000000053",
    ],
    conflicted: [
      "018f0000-0000-7001-8000-000000000061",
      "018f0000-0000-7001-8000-000000000062",
      "018f0000-0000-7001-8000-000000000063",
    ],
    refused: [
      "018f0000-0000-7001-8000-000000000071",
      "018f0000-0000-7001-8000-000000000072",
      "018f0000-0000-7001-8000-000000000073",
    ],
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requestPaths.push(path);
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      challengeRequests.push(request);
      if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
        authorEditDigest = requireDigestValue(request.canonical_command_digest, "command digest");
        if (failAuthorEditChallenge) {
          failAuthorEditChallenge = false;
          throw new Error("simulated pre-send challenge failure");
        }
      }
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(canonicalSession);
    if (path.endsWith("/manuscript/author-edits")) {
      const request = requireRequestBody(init);
      const idempotencyKey = requestHeaders(init).get("idempotency-key");
      if (!idempotencyKey || !authorEditDigest) {
        throw new Error("the admitted Author Edit identity is unavailable");
      }
      const zeroAuthorityResult = nextZeroAuthorityResult;
      const secondEdit = request.expected_authoritative_revision_id === FIRST_REVISION;
      const revisionId = secondEdit ? SECOND_REVISION : FIRST_REVISION;
      const commitId = secondEdit
        ? "018f0000-0000-7001-8000-000000000045"
        : "018f0000-0000-7001-8000-000000000035";
      const position = secondEdit ? "2" : "1";
      const body = secondEdit ? "Base!?+" : "Base!?";
      authorEditRequests.push({ idempotencyKey, request });
      const responseDigest: DigestValue = corruptAuthorEditAcknowledgement
        ? { ...authorEditDigest, value_hex_lowercase: "0".repeat(64) }
        : authorEditDigest;
      corruptAuthorEditAcknowledgement = false;
      let response: ApplyAuthorEditResponse;
      if (zeroAuthorityResult === undefined) {
        canonicalSession = secondEdit ? secondFreshSession : freshSession;
        response = createAppliedAuthorEditResponse({
          request,
          commandDigest: responseDigest,
          idempotencyKey,
          commandId: secondEdit
            ? "018f0000-0000-7001-8000-000000000041"
            : "018f0000-0000-7001-8000-000000000031",
          authorCommandAdmissionId: secondEdit
            ? "018f0000-0000-7001-8000-000000000042"
            : "018f0000-0000-7001-8000-000000000032",
          receiptId: secondEdit
            ? "018f0000-0000-7001-8000-000000000043"
            : "018f0000-0000-7001-8000-000000000033",
          authoritativeRevisionId: revisionId,
          authoritativeCommitId: commitId,
          projectActivityPosition: position,
          priorRevisionId: secondEdit ? FIRST_REVISION : REVISION,
          body,
        });
      } else {
        const [commandId, admissionId, receiptId] = zeroIds[zeroAuthorityResult];
        const priorRevision = requireString(
          request.expected_authoritative_revision_id,
          "expected authoritative revision",
        );
        const currentHead = zeroAuthorityResult === "conflicted"
          ? "018f0000-0000-7001-8000-000000000064"
          : priorRevision;
        let effect: Exclude<ApplyAuthorEditEffect, { kind: "authoritative_applied" }>;
        if (zeroAuthorityResult === "conflicted") {
          effect = {
            kind: "conflicted",
            reason: "stale_authoritative_head",
            current_authoritative_revision_id: currentHead,
          };
        } else if (zeroAuthorityResult === "no_effect") {
          effect = { kind: "no_effect", reason: "content_unchanged" };
        } else {
          effect = { kind: "refused", reason: "invalid_selection" };
        }
        response = {
          schema_id: "storyos.command.apply-author-edit.response.v2",
          correlation_id: requireString(request.correlation_id, "correlation_id"),
          project_scope: scenario.project.project_scope,
          command_id: commandId,
          author_command_admission_id: admissionId,
          receipt: {
            receipt_id: receiptId,
            project_scope: scenario.project.project_scope,
            command_kind: "applyAuthorEdit",
            command_digest: responseDigest,
            idempotency_key: idempotencyKey,
            producer_cause: "author_command_admission",
            author_command_admission_id: admissionId,
            expected_heads: [priorRevision],
            prior_heads: [currentHead],
            resulting_heads: [currentHead],
            authoritative_revision_ids: [],
            proposal_revision_ids: [],
            authoritative_commit_ids: [],
            author_action_sequence: null,
            draft_artifact_refs: [],
            artifact_lifecycle_event_refs: [],
            condition_refs: [],
            result: zeroAuthorityResult,
            created_at: "2026-08-15T08:00:00.000Z",
          },
          effect,
          completed_intent_record_id: requireString(
            request.completed_intent_record_id,
            "completed_intent_record_id",
          ),
          local_intent_sequence: requireString(
            request.local_intent_sequence,
            "local_intent_sequence",
          ),
        };
      }
      return jsonResponse(response);
    }
    if (path.includes("/manuscript/author-edit-outcomes/")) {
      const last = authorEditRequests.at(-1);
      if (!last || init?.method !== "GET" || init.body !== undefined || !authorEditDigest) {
        throw new Error("unexpected outcome Query");
      }
      const secondEdit = last.request.expected_authoritative_revision_id === FIRST_REVISION;
      canonicalSession = secondEdit ? secondFreshSession : freshSession;
      const response: GetApplyAuthorEditOutcomeResponse = {
        schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000081",
        project_scope: scenario.project.project_scope,
        outcome: {
          outcome_kind: "committed",
          response: createAppliedAuthorEditResponse({
            request: last.request,
            commandDigest: authorEditDigest,
            idempotencyKey: last.idempotencyKey,
            commandId: secondEdit
              ? "018f0000-0000-7001-8000-000000000041"
              : "018f0000-0000-7001-8000-000000000031",
            authorCommandAdmissionId: secondEdit
              ? "018f0000-0000-7001-8000-000000000042"
              : "018f0000-0000-7001-8000-000000000032",
            receiptId: secondEdit
              ? "018f0000-0000-7001-8000-000000000043"
              : "018f0000-0000-7001-8000-000000000033",
            authoritativeRevisionId: secondEdit ? SECOND_REVISION : FIRST_REVISION,
            authoritativeCommitId: secondEdit
              ? "018f0000-0000-7001-8000-000000000045"
              : "018f0000-0000-7001-8000-000000000035",
            projectActivityPosition: secondEdit ? "2" : "1",
            priorRevisionId: secondEdit ? FIRST_REVISION : REVISION,
            body: secondEdit ? "Base!?+" : "Base!?",
          }),
        },
      };
      return jsonResponse(response);
    }
    if (path.endsWith(`/chapters/${CHAPTER}`)) {
      return jsonResponse({
        ...scenario.chapter,
        correlation_id: "018f0000-0000-7001-8000-000000000036",
        project_activity_position: "1",
        chapter: {
          ...scenario.chapter.chapter,
          current_revision: chapterRevision(FIRST_REVISION, "Base!?"),
        },
      });
    }
    throw new Error(`unexpected request: ${path}`);
  };

  function chapterForSession(editorSession: GetEditorSessionResponse): GetChapterResponse {
    return {
      ...scenario.chapter,
      project_activity_position: editorSession.base_snapshot.project_activity_position,
      chapter: {
        ...scenario.chapter.chapter,
        current_revision: editorSession.base_snapshot.materialized_revision,
      },
    };
  }

  async function openCurrentWorkspace(): Promise<EditorReadyState> {
    const workspace = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: chapterForSession(canonicalSession),
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(workspace);
    openDatabase = workspace.database;
    return workspace;
  }

  async function intentCount(database: IDBDatabase): Promise<number> {
    return requestResult(
      database.transaction("intents", "readonly").objectStore("intents").count(),
    );
  }

  async function deleteWorkspace(workspace: EditorReadyState): Promise<void> {
    workspace.database.close();
    await deleteJournal(scenario.journalName);
  }

  async function replaceGroups(
    workspace: EditorReadyState,
    groups: JournalSubmissionGroup[],
  ): Promise<void> {
    const transaction = workspace.database.transaction("submission_groups", "readwrite");
    const store = transaction.objectStore("submission_groups");
    store.clear();
    for (const group of groups) store.put(group);
    await transactionResult(transaction);
  }

  async function replaceRecords(
    workspace: EditorReadyState,
    records: JournalIntentRecord[],
  ): Promise<void> {
    const transaction = workspace.database.transaction("intents", "readwrite");
    const store = transaction.objectStore("intents");
    store.clear();
    for (const record of records) store.put(record);
    await transactionResult(transaction);
  }

  await deleteJournal(scenario.journalName);
  let openDatabase: IDBDatabase | undefined;
  try {
    let state = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(state);
    let workspace = state;
    openDatabase = workspace.database;
    const partitionTransaction = workspace.database.transaction("partitions", "readwrite");
    partitionTransaction.objectStore("partitions")
      .delete(workspace.partition.journal_partition_id);
    await transactionResult(partitionTransaction);
    await expect(persistReplaceSelection(workspace, {
      from: 4,
      to: 4,
      text: "!",
      resultingBody: "Base!",
    })).rejects.toThrow(/partition is incompatible/);
    expect(await intentCount(workspace.database)).toBe(0);
    expect((await rebuildPendingProjection(workspace)).save_state).toBe("clean");

    workspace.database.close();
    state = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(state);
    let reopened = state;
    openDatabase = reopened.database;
    await persistReplaceSelection(reopened, {
      from: 4,
      to: 4,
      text: "!",
      resultingBody: "Base!",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    await expect(persistReplaceSelection(reopened, {
      from: 5,
      to: 5,
      text: "?",
      resultingBody: "Base!?",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.251Z",
    })).rejects.toThrow(/requires prior group settlement/);
    expect(await intentCount(reopened.database)).toBe(1);
    await deleteWorkspace(reopened);

    workspace = await openCurrentWorkspace();
    await persistReplaceSelection(workspace, {
      from: 4,
      to: 4,
      text: "!",
      resultingBody: "Base!",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    workspace.pending = await persistReplaceSelection(workspace, {
      from: 5,
      to: 5,
      text: "?",
      resultingBody: "Base!?",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.250Z",
    });
    expect(workspace.pending).toEqual({
      body: "Base!?",
      save_state: "saving",
      unsettled_intent_count: 2,
      authoritative_revision_id: REVISION,
    });
    expect(await intentCount(workspace.database)).toBe(2);
    expect(requestPaths.some((path) => path.includes("author-edits"))).toBe(false);

    await expect(submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).rejects.toThrow(/simulated pre-send challenge failure/);
    await expect(submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).rejects.toThrow(/acknowledgement does not converge/);
    expect(await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toEqual({
      body: "Base!?",
      save_state: "saved",
      unsettled_intent_count: 0,
      authoritative_revision_id: FIRST_REVISION,
    });
    expect(authorEditRequests).toHaveLength(1);
    expect(authorEditRequests[0]?.request.author_edit_units).toEqual([{
      normalized_primitives: [{ kind: "replace_selection", from: 4, to: 4, text: "!" }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1",
        from: 4,
        to: 4,
      },
    }, {
      normalized_primitives: [{ kind: "replace_selection", from: 5, to: 5, text: "?" }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1",
        from: 5,
        to: 5,
      },
    }]);
    const editChallenges = challengeRequests.filter(
      (request) => request.command_schema === "storyos.command.apply-author-edit.request.v1",
    );
    expect(editChallenges).toHaveLength(2);
    expect(editChallenges[0]?.idempotency_key).toBe(editChallenges[1]?.idempotency_key);
    expect(editChallenges[0]?.canonical_command_digest)
      .toEqual(editChallenges[1]?.canonical_command_digest);
    expect(workspace.session).toEqual(freshSession);

    workspace.pending = await persistReplaceSelection(workspace, {
      from: 6,
      to: 6,
      text: "+",
      resultingBody: "Base!?+",
      inputOrigin: "paste",
      undoGroupId: "018f0000-0000-7001-8000-000000000049",
      createdAt: "2026-08-15T08:00:00.500Z",
    });
    expect(workspace.pending).toEqual({
      body: "Base!?+",
      save_state: "saving",
      unsettled_intent_count: 1,
      authoritative_revision_id: FIRST_REVISION,
    });
    expect(await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toEqual({
      body: "Base!?+",
      save_state: "saved",
      unsettled_intent_count: 0,
      authoritative_revision_id: SECOND_REVISION,
    });
    expect(workspace.session).toEqual(secondFreshSession);
    const finalJournal = await validateJournalSnapshot(
      workspace,
      await readJournalSnapshot(workspace),
    );
    const expectedRecord = (
      record: JournalIntentRecord,
      options: {
        readonly base: GetEditorSessionResponse["base_snapshot"];
        readonly createdAt: string;
        readonly origin: JournalIntentRecord["input_origin"];
        readonly sequence: number;
        readonly undoGroupId: string;
        readonly unit: NonNullable<JournalIntentRecord["author_edit_unit"]>;
      },
    ): JournalIntentRecord => ({
      completed_intent_record_id: record.completed_intent_record_id,
      local_intent_sequence: options.sequence,
      journal_partition_id: workspace.partition.journal_partition_id,
      project_scope: scenario.project.project_scope,
      editor_session_id: SESSION,
      writer_generation: "1",
      limit_profile_revision: "storyos.foundation.absolute.v1",
      batch_policy_revision: "storyos.author-edit-batch.release-1.preview.v1",
      input_origin: options.origin,
      chapter_object_id: CHAPTER,
      base_snapshot_id: options.base.snapshot_id,
      base_activity_position: options.base.project_activity_position,
      target_refs: options.base.target_refs,
      expected_authoritative_heads: [options.base.authoritative_head_revision_id],
      expected_proposal_heads: [],
      proposal_anchors: [],
      observed_ownership_partition: "authoritative",
      author_edit_unit: options.unit,
      retry_source: { kind: "fresh_editor_intent" },
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_binding: {
        kind: "direct_author_input",
        undo_group_id: options.undoGroupId,
      },
      payload_chain_ref: record.payload_chain_ref,
      payload_digest: record.payload_digest,
      projection_dependency: {
        snapshot_id: options.base.snapshot_id,
        prior_sequence: options.sequence - 1,
      },
      created_at: options.createdAt,
    });
    const expectedRecords = [
      expectedRecord(recordAt(finalJournal.records, 0), {
        sequence: 1,
        origin: "typing",
        base: scenario.session.base_snapshot,
        unit: {
          normalized_primitives: [{ kind: "replace_selection", from: 4, to: 4, text: "!" }],
          selection_snapshot: {
            coordinate_profile: "storyos.editor.utf16-code-unit.v1",
            from: 4,
            to: 4,
          },
        },
        undoGroupId: "018f0000-0000-7001-8000-000000000040",
        createdAt: "2026-08-15T08:00:00.000Z",
      }),
      expectedRecord(recordAt(finalJournal.records, 1), {
        sequence: 2,
        origin: "typing",
        base: scenario.session.base_snapshot,
        unit: {
          normalized_primitives: [{ kind: "replace_selection", from: 5, to: 5, text: "?" }],
          selection_snapshot: {
            coordinate_profile: "storyos.editor.utf16-code-unit.v1",
            from: 5,
            to: 5,
          },
        },
        undoGroupId: "018f0000-0000-7001-8000-000000000040",
        createdAt: "2026-08-15T08:00:00.250Z",
      }),
      expectedRecord(recordAt(finalJournal.records, 2), {
        sequence: 3,
        origin: "paste",
        base: freshSession.base_snapshot,
        unit: {
          normalized_primitives: [{ kind: "replace_selection", from: 6, to: 6, text: "+" }],
          selection_snapshot: {
            coordinate_profile: "storyos.editor.utf16-code-unit.v1",
            from: 6,
            to: 6,
          },
        },
        undoGroupId: "018f0000-0000-7001-8000-000000000049",
        createdAt: "2026-08-15T08:00:00.500Z",
      }),
    ];
    expect(finalJournal.records).toEqual(expectedRecords);

    const expectedPayloadChains: JournalPayloadChain[] = [];
    for (const chain of finalJournal.payloadChains) {
      const records = expectedRecords.filter(
        (record) => record.payload_chain_ref === chain.payload_chain_id,
      );
      const firstRecord = recordAt(records, 0);
      const base = firstRecord.base_snapshot_id === scenario.session.base_snapshot.snapshot_id
        ? scenario.session.base_snapshot
        : freshSession.base_snapshot;
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
        ordered_patch_refs: await Promise.all(records.map(async (record, index) => {
          const patch = chain.ordered_patch_refs[index];
          if (!patch) throw new Error(`Journal payload patch ${index} is unavailable`);
          return {
            patch_id: patch.patch_id,
            completed_intent_record_id: record.completed_intent_record_id,
            local_intent_sequence: record.local_intent_sequence,
            normalized_primitives: requireAuthorEditUnit(record).normalized_primitives,
            resulting_payload_digest: await digestJournalValue(
              finalJournal.bodyBySequence.get(record.local_intent_sequence),
              crypto,
            ),
          };
        })),
      });
    }
    expect(finalJournal.payloadChains).toEqual(expectedPayloadChains);

    const expectedAppliedSettlement = (
      group: JournalSubmissionGroup,
      options: {
        readonly admissionId: string;
        readonly commandId: string;
        readonly commitId: string;
        readonly installedBase: GetEditorSessionResponse["base_snapshot"];
        readonly position: string;
        readonly priorHead: string;
        readonly receiptId: string;
        readonly revision: GetEditorSessionResponse["base_snapshot"]["materialized_revision"];
      },
    ): SubmissionSettlement => ({
      kind: "applied_receipt_settled",
      command_id: options.commandId,
      author_command_admission_id: options.admissionId,
      receipt: {
        receipt_id: options.receiptId,
        project_scope: scenario.project.project_scope,
        command_kind: "applyAuthorEdit",
        command_digest: group.frozen_request_digest,
        idempotency_key: group.idempotency_key,
        producer_cause: "author_command_admission",
        author_command_admission_id: options.admissionId,
        expected_heads: [options.priorHead],
        prior_heads: [options.priorHead],
        resulting_heads: [options.revision.revision_id],
        authoritative_revision_ids: [options.revision.revision_id],
        proposal_revision_ids: [],
        authoritative_commit_ids: [options.commitId],
        author_action_sequence: options.position,
        draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [],
        condition_refs: [],
        result: "authoritative_applied",
        created_at: "2026-08-15T08:00:00.000Z",
      },
      authoritative_revision: options.revision,
      authoritative_commit_id: options.commitId,
      author_action_sequence: options.position,
      project_activity_position: options.position,
      installed_base_snapshot: options.installedBase,
    });
    const expectedGroup = (
      group: JournalSubmissionGroup,
      records: JournalIntentRecord[],
      settlement: SubmissionSettlement,
    ): JournalSubmissionGroup => {
      const first = recordAt(records, 0);
      const last = recordAt(records, records.length - 1);
      const expectedHead = first.expected_authoritative_heads[0];
      if (expectedHead === undefined) throw new Error("the expected Authoritative Head is missing");
      return {
        journal_submission_group_id: group.journal_submission_group_id,
        journal_partition_id: workspace.partition.journal_partition_id,
        project_scope: scenario.project.project_scope,
        editor_session_id: SESSION,
        writer_generation: "1",
        batch_policy_revision: "storyos.author-edit-batch.release-1.preview.v1",
        ordered_coverage: records.map((record) => ({
          local_intent_sequence: record.local_intent_sequence,
          intent_record_ref: record.completed_intent_record_id,
          payload_digest: record.payload_digest,
        })),
        covered_sequence_range: {
          first: first.local_intent_sequence,
          last: last.local_intent_sequence,
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
          expected_authoritative_revision_id: expectedHead,
          expected_proposal_head_revision_ids: [],
          target_refs: first.target_refs,
          observed_ownership_partition: "authoritative",
          editor_contract_revision: "storyos.editor-contract.release-1.v2",
          undo_group_id: first.undo_group_binding.undo_group_id,
          completed_intent_record_id: first.completed_intent_record_id,
          local_intent_sequence: String(first.local_intent_sequence),
          author_edit_units: records.map(requireAuthorEditUnit),
        },
        frozen_request_digest: group.frozen_request_digest,
        frozen_payload_coverage_digest: group.frozen_payload_coverage_digest,
        settlement,
        frozen_at: group.frozen_at,
      };
    };
    const firstGroup = groupAt(finalJournal.groups, 0);
    const secondGroup = groupAt(finalJournal.groups, 1);
    const expectedGroups = [
      expectedGroup(firstGroup, expectedRecords.slice(0, 2), expectedAppliedSettlement(
        firstGroup,
        {
          commandId: "018f0000-0000-7001-8000-000000000031",
          admissionId: "018f0000-0000-7001-8000-000000000032",
          receiptId: "018f0000-0000-7001-8000-000000000033",
          priorHead: REVISION,
          revision: freshSession.base_snapshot.materialized_revision,
          commitId: "018f0000-0000-7001-8000-000000000035",
          position: "1",
          installedBase: freshSession.base_snapshot,
        },
      )),
      expectedGroup(secondGroup, expectedRecords.slice(2), expectedAppliedSettlement(
        secondGroup,
        {
          commandId: "018f0000-0000-7001-8000-000000000041",
          admissionId: "018f0000-0000-7001-8000-000000000042",
          receiptId: "018f0000-0000-7001-8000-000000000043",
          priorHead: FIRST_REVISION,
          revision: secondFreshSession.base_snapshot.materialized_revision,
          commitId: "018f0000-0000-7001-8000-000000000045",
          position: "2",
          installedBase: secondFreshSession.base_snapshot,
        },
      )),
    ];
    expect(finalJournal.groups).toEqual(expectedGroups);
    expect(finalJournal.watermark).toEqual({
      key: `durable_high_watermark:${workspace.partition.journal_partition_id}`,
      value: 3,
    });
    expect(finalJournal.activeBase).toEqual(secondFreshSession.base_snapshot);
    expect([...finalJournal.bodyBySequence.entries()]).toEqual([
      [1, "Base!"],
      [2, "Base!?"],
      [3, "Base!?+"],
    ]);
    expect([...finalJournal.covered]).toEqual([1, 2, 3]);

    const challengeCountBeforeCoverageFailures = challengeRequests.length;
    const originalGroups = structuredClone(finalJournal.groups);
    const coverageFailures: Array<(groups: JournalSubmissionGroup[]) => void> = [
      (groups) => { groups.shift(); },
      (groups) => { groupAt(groups, 0).ordered_coverage.pop(); },
      (groups) => {
        const group = groupAt(groups, 0);
        const first = group.ordered_coverage[0];
        if (!first) throw new Error("the first coverage row is unavailable");
        group.ordered_coverage[1] = first;
      },
      (groups) => {
        const second = groupAt(groups, 0).ordered_coverage[1];
        if (!second) throw new Error("the second coverage row is unavailable");
        second.local_intent_sequence = 3;
      },
      (groups) => { groupAt(groups, 0).ordered_coverage.reverse(); },
      (groups) => {
        const firstGroup = groupAt(groups, 0);
        const secondGroup = groupAt(groups, 1);
        const coverage = firstGroup.ordered_coverage[1];
        if (!coverage) throw new Error("the appended coverage row is unavailable");
        secondGroup.covered_sequence_range.first = 2;
        secondGroup.ordered_coverage.unshift(coverage);
      },
      (groups) => {
        groupAt(groups, 1).batch_policy_revision = "storyos.author-edit-batch.invalid";
      },
      (groups) => {
        groupAt(groups, 1).frozen_request_body.editor_contract_revision =
          "storyos.editor-contract.release-1.invalid";
      },
      (groups) => {
        groupAt(groups, 0).frozen_payload_coverage_digest.value_hex_lowercase = "0".repeat(64);
      },
    ];
    for (const corrupt of coverageFailures) {
      const groups = structuredClone(originalGroups);
      corrupt(groups);
      await replaceGroups(workspace, groups);
      await expect(submitOnePendingAuthorEdit({
        workspace,
        baseUrl: location.origin,
        fetchImpl,
        cryptoImpl: crypto,
      })).rejects.toThrow(/corrupt/);
      expect(challengeRequests).toHaveLength(challengeCountBeforeCoverageFailures);
      await replaceGroups(workspace, originalGroups);
    }

    await expect(persistReplaceSelection(workspace, {
      from: 5,
      to: 5,
      text: "?",
      resultingBody: "x".repeat(1_048_577),
    })).rejects.toThrow(/limit failed/);
    expect(await intentCount(workspace.database)).toBe(3);
    workspace.partition.disposition = "read_only_observer";
    await expect(persistReplaceSelection(workspace, {
      from: 5,
      to: 5,
      text: "?",
      resultingBody: "Base!?",
    })).rejects.toThrow(/read only/);
    workspace.partition.disposition = "current_writer_open";
    expect(await intentCount(workspace.database)).toBe(3);

    workspace.pending = await persistReplaceSelection(workspace, {
      from: 7,
      to: 7,
      text: "Z",
      resultingBody: "Base!?+Z",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000050",
      createdAt: "2026-08-15T08:00:00.501Z",
    });
    const policyJournal = await validateJournalSnapshot(
      workspace,
      await readJournalSnapshot(workspace),
    );
    const originalRecords = structuredClone(policyJournal.records);
    const historicalPolicyFailures: Array<(records: JournalIntentRecord[]) => void> = [
      (records) => { recordAt(records, 1).created_at = "2026-08-15T08:00:00.251Z"; },
      (records) => { recordAt(records, 1).input_origin = "paste"; },
      (records) => {
        recordAt(records, 1).undo_group_binding.undo_group_id =
          "018f0000-0000-7001-8000-000000000099";
      },
    ];
    const challengeCountBeforeHistoricalFailures = challengeRequests.length;
    for (const corrupt of historicalPolicyFailures) {
      const records = structuredClone(originalRecords);
      corrupt(records);
      await replaceRecords(workspace, records);
      await expect(submitOnePendingAuthorEdit({
        workspace,
        baseUrl: location.origin,
        fetchImpl,
        cryptoImpl: crypto,
      })).rejects.toThrow(/corrupt/);
      expect(challengeRequests).toHaveLength(challengeCountBeforeHistoricalFailures);
      await replaceRecords(workspace, originalRecords);
    }

    await deleteWorkspace(workspace);
    workspace = await openCurrentWorkspace();
    const limitUndoGroupId = "018f0000-0000-7001-8000-000000000081";
    let limitBody = canonicalSession.base_snapshot.materialized_revision.body;
    for (let index = 0; index < 240; index += 1) {
      const nextBody = `${limitBody}a`;
      workspace.pending = await persistReplaceSelection(workspace, {
        from: limitBody.length,
        to: limitBody.length,
        text: "a",
        resultingBody: nextBody,
        inputOrigin: "typing",
        undoGroupId: limitUndoGroupId,
        createdAt: new Date(Date.parse("2026-08-15T09:00:00.000Z") + index).toISOString(),
      });
      limitBody = nextBody;
    }
    expect(await intentCount(workspace.database)).toBe(240);
    await expect(persistReplaceSelection(workspace, {
      from: limitBody.length,
      to: limitBody.length,
      text: "a",
      resultingBody: `${limitBody}a`,
      inputOrigin: "typing",
      undoGroupId: limitUndoGroupId,
      createdAt: "2026-08-15T09:00:00.240Z",
    })).rejects.toThrow(/limit failed/);
    const challengeCountBeforeLimitFreeze = challengeRequests.length;
    // Count Journal validation separately from the frozen request and coverage hashes.
    let journalDigestCalls = 0;
    workspace.cryptoImpl = withDigest(crypto, (algorithm, data) => {
      journalDigestCalls += 1;
      return crypto.subtle.digest(algorithm, data);
    });
    const limitGroup = await freezeOneIntentSubmission(workspace, crypto);
    workspace.cryptoImpl = crypto;
    expect(journalDigestCalls).toBe(480);
    expect(limitGroup.covered_sequence_range).toEqual({ first: 1, last: 240 });
    expect(limitGroup.ordered_coverage).toHaveLength(240);
    expect(limitGroup.frozen_request_body.author_edit_units).toHaveLength(240);
    expect(challengeRequests).toHaveLength(challengeCountBeforeLimitFreeze);
    await deleteWorkspace(workspace);

    const zeroCases = [{
      result: "no_effect",
      text: "",
      suffix: "",
    }, {
      result: "conflicted",
      text: "C",
      suffix: "C",
    }, {
      result: "refused",
      text: "R",
      suffix: "R",
    }] satisfies ReadonlyArray<{
      result: ZeroAuthorityResult;
      suffix: string;
      text: string;
    }>;
    for (const [index, zeroCase] of zeroCases.entries()) {
      workspace = await openCurrentWorkspace();
      const baseBeforeZero = structuredClone(workspace.session);
      const baseBody = baseBeforeZero.base_snapshot.materialized_revision.body;
      const localBody = `${baseBody}${zeroCase.suffix}`;
      workspace.pending = await persistReplaceSelection(workspace, {
        from: baseBody.length,
        to: baseBody.length,
        text: zeroCase.text,
        resultingBody: localBody,
        inputOrigin: zeroCase.result === "no_effect" ? "deletion" : "typing",
        undoGroupId: `018f0000-0000-7001-8000-00000000008${index + 2}`,
        createdAt: `2026-08-15T10:00:00.00${index}Z`,
      });
      expect(workspace.pending).toEqual({
        body: localBody,
        save_state: "saving",
        unsettled_intent_count: 1,
        authoritative_revision_id: baseBeforeZero.base_snapshot.authoritative_head_revision_id,
      });
      const challengeCountBeforeZero = challengeRequests.length;
      nextZeroAuthorityResult = zeroCase.result;
      expect(await submitOnePendingAuthorEdit({
        workspace,
        baseUrl: location.origin,
        fetchImpl,
        cryptoImpl: crypto,
      })).toEqual({
        body: localBody,
        save_state: "needs_attention",
        unsettled_intent_count: 1,
        authoritative_revision_id: baseBeforeZero.base_snapshot.authoritative_head_revision_id,
      });
      expect(workspace.session).toEqual(baseBeforeZero);
      expect(challengeRequests).toHaveLength(challengeCountBeforeZero + 1);
      const zeroJournal = await validateJournalSnapshot(
        workspace,
        await readJournalSnapshot(workspace),
      );
      expect(zeroJournal.records).toHaveLength(1);
      expect(zeroJournal.payloadChains).toHaveLength(1);
      expect(zeroJournal.groups).toHaveLength(1);
      expect(zeroJournal.activeBase).toEqual(baseBeforeZero.base_snapshot);
      expect([...zeroJournal.bodyBySequence.entries()]).toEqual([[1, localBody]]);
      expect([...zeroJournal.covered]).toEqual([1]);
      const zeroGroup = groupAt(zeroJournal.groups, 0);
      const [commandId, admissionId, receiptId] = zeroIds[zeroCase.result];
      const currentHead = zeroCase.result === "conflicted"
        ? "018f0000-0000-7001-8000-000000000064"
        : baseBeforeZero.base_snapshot.authoritative_head_revision_id;
      let expectedEffect: Exclude<ApplyAuthorEditEffect, { kind: "authoritative_applied" }>;
      if (zeroCase.result === "conflicted") {
        expectedEffect = {
          kind: "conflicted",
          reason: "stale_authoritative_head",
          current_authoritative_revision_id: currentHead,
        };
      } else if (zeroCase.result === "no_effect") {
        expectedEffect = { kind: "no_effect", reason: "content_unchanged" };
      } else {
        expectedEffect = { kind: "refused", reason: "invalid_selection" };
      }
      expect(zeroGroup.settlement).toEqual({
        kind: "zero_authority_receipt_settled",
        command_id: commandId,
        author_command_admission_id: admissionId,
        receipt: {
          receipt_id: receiptId,
          project_scope: scenario.project.project_scope,
          command_kind: "applyAuthorEdit",
          command_digest: zeroGroup.frozen_request_digest,
          idempotency_key: zeroGroup.idempotency_key,
          producer_cause: "author_command_admission",
          author_command_admission_id: admissionId,
          expected_heads: [baseBeforeZero.base_snapshot.authoritative_head_revision_id],
          prior_heads: [currentHead],
          resulting_heads: [currentHead],
          authoritative_revision_ids: [],
          proposal_revision_ids: [],
          authoritative_commit_ids: [],
          author_action_sequence: null,
          draft_artifact_refs: [],
          artifact_lifecycle_event_refs: [],
          condition_refs: [],
          result: zeroCase.result,
          created_at: "2026-08-15T08:00:00.000Z",
        },
        effect: expectedEffect,
      });
      if (index < zeroCases.length - 1) await deleteWorkspace(workspace);
    }

    const validated = await validateJournalSnapshot(
      workspace,
      await readJournalSnapshot(workspace),
    );
    const corruptChain = structuredClone(validated.payloadChains[0]);
    if (!corruptChain) throw new Error("the payload chain is unavailable");
    const corruptPatch = corruptChain.ordered_patch_refs[0];
    if (!corruptPatch) throw new Error("the payload patch is unavailable");
    corruptPatch.resulting_payload_digest.value_hex_lowercase = "0".repeat(64);
    const corruptTransaction = workspace.database.transaction("payload_chains", "readwrite");
    corruptTransaction.objectStore("payload_chains").put(corruptChain);
    await transactionResult(corruptTransaction);
    await expect(rebuildPendingProjection(workspace)).rejects.toThrow(/corrupt/);

    const schemaTransaction = workspace.database.transaction("metadata", "readwrite");
    schemaTransaction.objectStore("metadata").put({ key: "schema", version: 999 });
    await transactionResult(schemaTransaction);
    workspace.database.close();
    const recovery = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: chapterForSession(canonicalSession),
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    if (recovery.kind === "editor-ready") openDatabase = recovery.database;
    expect(recovery).toMatchObject({
      kind: "editor-read-only-recovery",
      code: "local_journal_unavailable",
    });
    openDatabase = undefined;
  } finally {
    openDatabase?.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

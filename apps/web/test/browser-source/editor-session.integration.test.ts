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
  JournalSubmissionGroup,
} from "../../src/editor-types.ts";
import {
  readJournalSnapshot,
  validateJournalSnapshot,
} from "../../src/local-edit-journal.ts";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
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
      materialized_revision: { revision_id: FIRST_REVISION, body: "Base!?" },
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
      materialized_revision: { revision_id: SECOND_REVISION, body: "Base!?+" },
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
          current_revision: { revision_id: FIRST_REVISION, body: "Base!?" },
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
    expect(finalJournal.records).toHaveLength(3);
    expect(finalJournal.records.map((record) => ({
      sequence: record.local_intent_sequence,
      origin: record.input_origin,
      base: record.base_snapshot_id,
      unit: record.author_edit_unit,
      undo: record.undo_group_binding.undo_group_id,
      createdAt: record.created_at,
    }))).toEqual([{
      sequence: 1,
      origin: "typing",
      base: scenario.session.base_snapshot.snapshot_id,
      unit: {
        normalized_primitives: [{ kind: "replace_selection", from: 4, to: 4, text: "!" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1",
          from: 4,
          to: 4,
        },
      },
      undo: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    }, {
      sequence: 2,
      origin: "typing",
      base: scenario.session.base_snapshot.snapshot_id,
      unit: {
        normalized_primitives: [{ kind: "replace_selection", from: 5, to: 5, text: "?" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1",
          from: 5,
          to: 5,
        },
      },
      undo: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.250Z",
    }, {
      sequence: 3,
      origin: "paste",
      base: freshSession.base_snapshot.snapshot_id,
      unit: {
        normalized_primitives: [{ kind: "replace_selection", from: 6, to: 6, text: "+" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1",
          from: 6,
          to: 6,
        },
      },
      undo: "018f0000-0000-7001-8000-000000000049",
      createdAt: "2026-08-15T08:00:00.500Z",
    }]);
    expect(finalJournal.payloadChains).toHaveLength(2);
    expect(finalJournal.payloadChains.flatMap((chain) => chain.ordered_patch_refs)
      .map((patch) => patch.local_intent_sequence)).toEqual([1, 2, 3]);
    expect(finalJournal.groups).toHaveLength(2);
    expect(finalJournal.groups.map((group) => group.settlement)).toMatchObject([{
      kind: "applied_receipt_settled",
      command_id: "018f0000-0000-7001-8000-000000000031",
      authoritative_revision: { revision_id: FIRST_REVISION, body: "Base!?" },
      project_activity_position: "1",
      installed_base_snapshot: freshSession.base_snapshot,
    }, {
      kind: "applied_receipt_settled",
      command_id: "018f0000-0000-7001-8000-000000000041",
      authoritative_revision: { revision_id: SECOND_REVISION, body: "Base!?+" },
      project_activity_position: "2",
      installed_base_snapshot: secondFreshSession.base_snapshot,
    }]);
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
    const limitGroup = await freezeOneIntentSubmission(workspace, crypto);
    expect(limitGroup.covered_sequence_range).toEqual({ first: 1, last: 240 });
    expect(limitGroup.ordered_coverage).toHaveLength(240);
    expect(limitGroup.frozen_request_body.author_edit_units).toHaveLength(240);
    expect(challengeRequests).toHaveLength(challengeCountBeforeLimitFreeze);
    await deleteWorkspace(workspace);

    const zeroCases = [{
      result: "no_effect",
      text: "",
      suffix: "",
      reason: "content_unchanged",
    }, {
      result: "conflicted",
      text: "C",
      suffix: "C",
      reason: "stale_authoritative_head",
    }, {
      result: "refused",
      text: "R",
      suffix: "R",
      reason: "invalid_selection",
    }] satisfies ReadonlyArray<{
      reason: string;
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
      const zeroJournal = await validateJournalSnapshot(
        workspace,
        await readJournalSnapshot(workspace),
      );
      expect({
        records: zeroJournal.records.length,
        chains: zeroJournal.payloadChains.length,
        groups: zeroJournal.groups.length,
      }).toEqual({ records: 1, chains: 1, groups: 1 });
      expect(zeroJournal.activeBase).toEqual(baseBeforeZero.base_snapshot);
      expect([...zeroJournal.bodyBySequence.entries()]).toEqual([[1, localBody]]);
      expect([...zeroJournal.covered]).toEqual([1]);
      const zeroGroup = groupAt(zeroJournal.groups, 0);
      expect(zeroGroup.settlement).toMatchObject({
        kind: "zero_authority_receipt_settled",
        receipt: { result: zeroCase.result },
        effect: { kind: zeroCase.result, reason: zeroCase.reason },
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

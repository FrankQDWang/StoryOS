import { expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  openEditorWorkspace,
  persistReplaceSelection,
  submitOnePendingAuthorEdit,
} from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import {
  readJournalSnapshot,
  rebuildPendingProjection,
} from "../../src/local-edit-journal.ts";
import {
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  closeTrackedDatabases,
  chapterRevision,
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requestHeaders,
  requireDigestValue,
  requireEditorReady,
  requireRequestBody,
  trackDatabase,
} from "./scenario.ts";

const NEXT_REVISION = "018f0000-0000-7001-8000-000000000034";
const NEXT_SNAPSHOT = "018f0000-0000-7001-8000-000000000038";
const SUCCESSOR_DIGEST: DigestValue = {
  algorithm: "sha256",
  profile: "storyos.canonical-payload.sha256.v1",
  value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
};

it("collects Journal payload only after settlement and a durable successor", async () => {
  const scenario = createBrowserScenario();
  const freshSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000091",
    writer: { kind: "current_writer", writer_generation: "1" },
    base_snapshot: {
      ...scenario.session.base_snapshot,
      snapshot_id: NEXT_SNAPSHOT,
      project_activity_position: "1",
      authoritative_head_revision_id: NEXT_REVISION,
      materialized_revision: chapterRevision(NEXT_REVISION, "Base!?"),
      materialized_payload_digest: SUCCESSOR_DIGEST,
      created_at: "2026-08-15T08:00:00.000Z",
    },
  };
  let commandDigest: DigestValue | undefined;
  let lastEdit: Record<string, unknown> | undefined;
  let lastIdempotencyKey: string | undefined;
  let canonicalSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    expect(path).not.toMatch(/draft/i);
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
        commandDigest = requireDigestValue(request.canonical_command_digest, "command digest");
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
      lastEdit = requireRequestBody(init);
      lastIdempotencyKey = requestHeaders(init).get("idempotency-key") ?? undefined;
      if (!commandDigest || !lastIdempotencyKey) {
        throw new Error("the admitted command identity is unavailable");
      }
      canonicalSession = freshSession;
      return jsonResponse(createAppliedAuthorEditResponse({
        request: lastEdit,
        commandDigest,
        idempotencyKey: lastIdempotencyKey,
      }));
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();

  await deleteJournal(scenario.journalName);
  try {
    const workspace = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(workspace);
    trackDatabase(workspace.database, openDatabases);
    await persistReplaceSelection(workspace, {
      from: 4,
      to: 4,
      text: "!?",
      resultingBody: "Base!?",
      inputOrigin: "paste",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
    const unsettled = await readJournalSnapshot(workspace);
    expect(await collectEligibleJournalPayload(workspace)).toEqual({ fences: [] });
    expect(await readJournalSnapshot(workspace)).toEqual(unsettled);
    expect(await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toEqual({
      body: "Base!?",
      save_state: "saved",
      unsettled_intent_count: 0,
      authoritative_revision_id: NEXT_REVISION,
    });
    const settled = await readJournalSnapshot(workspace);
    expect(settled.records).toHaveLength(1);
    expect(settled.payloadChains).toHaveLength(1);
    expect(settled.groups).toHaveLength(1);
    const record = settled.records[0];
    const chain = settled.payloadChains[0];
    const group = settled.groups[0];
    if (!record || !chain || !group) throw new Error("settled Journal rows are unavailable");
    const collection = await collectEligibleJournalPayload(workspace);
    expect(collection.fences).toHaveLength(1);
    const fence = collection.fences[0];
    if (fence === null || typeof fence !== "object") {
      throw new Error("the collection fence is unavailable");
    }
    const fenceId = Reflect.get(fence, "collection_fence_id");
    if (typeof fenceId !== "string") throw new Error("the collection fence ID is unavailable");
    const expectedFence = {
      collection_fence_id: fenceId,
      journal_partition_id: workspace.partition.journal_partition_id,
      project_scope: scenario.project.project_scope,
      writer_generation: "1",
      partition_disposition: "current_writer_open",
      collected_groups: [{
        journal_submission_group_id: group.journal_submission_group_id,
        covered_sequence_range: { first: 1, last: 1 },
        payload_digests: [record.payload_digest],
        settlement_kind: "applied_receipt_settled",
        command_id: "018f0000-0000-7001-8000-000000000031",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        receipt_id: "018f0000-0000-7001-8000-000000000033",
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
    expect(collection).toEqual({ fences: [expectedFence] });
    const collected = await readJournalSnapshot(workspace);
    expect(collected.fences).toEqual([expectedFence]);
    expect(collected.records[0]?.author_edit_unit).toBeUndefined();
    expect(collected.payloadChains[0]?.checkpoint_ref.materialized_payload).toBeUndefined();
    expect(collected.payloadChains[0]?.ordered_patch_refs[0]?.normalized_primitives).toBeUndefined();
    expect(collected.groups[0]?.frozen_request_body.author_edit_units).toEqual([]);
    expect(collected.groups[0]?.payload_collection).toEqual({
      kind: "collected",
      collection_fence_id: fenceId,
    });
    expect(collected.groups[0]?.journal_submission_group_id)
      .toBe(group.journal_submission_group_id);
    expect(collected.groups[0]?.settlement.kind).toBe("applied_receipt_settled");
    expect(await rebuildPendingProjection(workspace)).toEqual({
      body: "Base!?",
      save_state: "saved",
      unsettled_intent_count: 0,
      authoritative_revision_id: NEXT_REVISION,
    });
    await persistReplaceSelection(workspace, {
      from: 6,
      to: 6,
      text: "+",
      resultingBody: "Base!?+",
      inputOrigin: "paste",
      undoGroupId: "018f0000-0000-7001-8000-000000000049",
      createdAt: "2026-08-15T08:00:00.500Z",
    });
    const pending = await readJournalSnapshot(workspace);
    expect(await collectEligibleJournalPayload(workspace)).toEqual({ fences: [expectedFence] });
    const retained = await readJournalSnapshot(workspace);
    expect(retained.records).toHaveLength(2);
    expect(retained.records[1]?.author_edit_unit?.normalized_primitives[0]?.text).toBe("+");
    expect(retained.fences).toEqual(pending.fences);
    expect(lastEdit).toBeDefined();
    workspace.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

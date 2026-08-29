import { expect, it } from "vitest";

import type {
  DigestValue,
  GetApplyAuthorEditOutcomeResponse,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  openEditorWorkspace,
  persistReplaceSelection,
  submitOnePendingAuthorEdit,
} from "../../src/editor-session.ts";
import {
  readJournalSnapshot,
  validateJournalSnapshot,
} from "../../src/local-edit-journal.ts";
import {
  OWNER,
  PROJECT,
  SESSION,
  closeTrackedDatabases,
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
  trackDatabase,
} from "./scenario.ts";

const NEXT_REVISION = "018f0000-0000-7001-8000-000000000034";

it("fences the old Journal partition before a late applied result settles", async () => {
  const scenario = createBrowserScenario();
  const fencedSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000091",
    writer: {
      kind: "read_only",
      observed_writer_generation: "2",
      reason: "superseded_by_takeover",
    },
    base_snapshot: {
      ...scenario.session.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000038",
      project_activity_position: "1",
      authoritative_head_revision_id: NEXT_REVISION,
      materialized_revision: chapterRevision(NEXT_REVISION, "Base!?"),
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
      },
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
  const counts = { authorEdits: 0, outcomes: 0 };
  const fetchImpl: typeof fetch = async (input, init) => {
    const parsed = new URL(input instanceof Request ? input.url : input);
    expect(parsed.href).not.toContain("a".repeat(64));
    const path = parsed.pathname;
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
      counts.authorEdits += 1;
      canonicalSession = fencedSession;
      throw new Error("simulated acknowledgement loss");
    }
    if (path.includes("/manuscript/author-edit-outcomes/")) {
      counts.outcomes += 1;
      expect(init?.method).toBe("GET");
      expect(init?.body).toBeUndefined();
      expect(requestHeaders(init).get("x-storyos-anti-forgery")).toBe("a".repeat(64));
      if (!lastEdit || !commandDigest || !lastIdempotencyKey) {
        throw new Error("the admitted command identity is unavailable");
      }
      const response: GetApplyAuthorEditOutcomeResponse = {
        schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000081",
        project_scope: scenario.project.project_scope,
        outcome: {
          outcome_kind: "committed",
          response: createAppliedAuthorEditResponse({
            request: lastEdit,
            commandDigest,
            idempotencyKey: lastIdempotencyKey,
          }),
        },
      };
      return jsonResponse(response);
    }
    throw new Error(`unexpected request: ${path}`);
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
    const openPartition = { ...workspace.partition };
    expect(openPartition).toMatchObject({
      writer_generation: "1",
      disposition: "current_writer_open",
    });
    await persistReplaceSelection(workspace, {
      from: 4,
      to: 4,
      text: "!?",
      resultingBody: "Base!?",
      inputOrigin: "paste",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
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
    expect(counts).toEqual({ authorEdits: 1, outcomes: 1 });
    const expectedPartition = { ...openPartition, disposition: "read_only_observer" };
    expect(workspace.partition).toEqual(expectedPartition);
    expect(workspace.session.writer).toEqual(fencedSession.writer);
    expect(await requestResult(
      workspace.database.transaction("partitions").objectStore("partitions")
        .get(openPartition.journal_partition_id),
    )).toEqual(expectedPartition);
    const snapshot = await validateJournalSnapshot(
      workspace,
      await readJournalSnapshot(workspace),
    );
    expect(snapshot.groups).toHaveLength(1);
    const group = snapshot.groups[0];
    if (!group) throw new Error("the settled Journal group is unavailable");
    expect(group).toMatchObject({
      journal_partition_id: openPartition.journal_partition_id,
      project_scope: scenario.project.project_scope,
      editor_session_id: SESSION,
      writer_generation: "1",
      batch_policy_revision: "storyos.author-edit-batch.release-1.preview.v1",
      action_class: "direct_editor_action",
      method: "POST",
      command_kind: "applyAuthorEdit",
      settlement: {
        kind: "applied_receipt_settled",
        command_id: "018f0000-0000-7001-8000-000000000031",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        authoritative_revision: chapterRevision(NEXT_REVISION, "Base!?"),
        project_activity_position: "1",
        installed_base_snapshot: fencedSession.base_snapshot,
      },
    });
    expect(group.ordered_coverage).toHaveLength(snapshot.records.length);
    await expect(persistReplaceSelection(workspace, {
      from: 6,
      to: 6,
      text: "+",
      resultingBody: "Base!?+",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000041",
      createdAt: "2026-08-15T08:00:01.000Z",
    })).rejects.toThrow(/read only/);
    expect(counts.authorEdits).toBe(1);
    workspace.database.close();
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

import { restoreVersionThreeJournal } from "./journal-version-three.ts";
import { expect, it, vi } from "vitest";
import type { DigestValue, GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openEditorWorkspace, persistReplaceSelection, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import { createJournalUuid, readJournalSnapshot } from "../../src/local-edit-journal.ts";
import {
  OWNER, PROJECT, SESSION, chapterRevision, createAppliedAuthorEditResponse,
  createBrowserScenario, deleteJournal, jsonResponse, requestHeaders,
  requireDigestValue, requireEditorReady, requireRequestBody, requestResult,
} from "./scenario.ts";

it("continues writing after 2400 saved and collected input intents", async () => {
  const scenario = createBrowserScenario();
  let body = "Base";
  let position = 0;
  let commandDigest: DigestValue | undefined;
  let session: GetEditorSessionResponse = {
    ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      commandDigest = requireDigestValue(
        requireRequestBody(init).canonical_command_digest, "command digest",
      );
      return jsonResponse({ nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(session);
    if (path.endsWith("/manuscript/author-edits")) {
      const request = requireRequestBody(init);
      const idempotencyKey = requestHeaders(init).get("idempotency-key");
      if (!commandDigest || !idempotencyKey) throw new Error("missing admitted identity");
      const revisionId = createJournalUuid();
      const response = createAppliedAuthorEditResponse({ request, commandDigest,
        idempotencyKey, body, projectActivityPosition: String(++position),
        priorRevisionId: session.base_snapshot.authoritative_head_revision_id,
        authoritativeRevisionId: revisionId, commandId: createJournalUuid(),
        authorCommandAdmissionId: createJournalUuid(), receiptId: createJournalUuid(),
        authoritativeCommitId: createJournalUuid(),
      });
      const digest = new Uint8Array(await crypto.subtle.digest(
        "SHA-256", new TextEncoder().encode(body),
      ));
      session = { ...session, base_snapshot: { ...session.base_snapshot,
        snapshot_id: createJournalUuid(), project_activity_position: String(position),
        authoritative_head_revision_id: revisionId,
        materialized_revision: chapterRevision(revisionId, body),
        materialized_payload_digest: { algorithm: "sha256",
          profile: "storyos.canonical-payload.sha256.v1",
          value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
        },
      } };
      return jsonResponse(response);
    }
    throw new Error(`unexpected request ${path}`);
  };
  await deleteJournal(scenario.journalName);
  let workspace = await openEditorWorkspace({ baseUrl: location.origin,
    project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  try {
    for (let batch = 0; batch < 10; batch++) {
      const undoGroupId = createJournalUuid();
      for (let offset = 0; offset < 240; offset++) {
        const from = body.length;
        body += "a";
        await persistReplaceSelection(workspace, { from, to: from, text: "a",
          resultingBody: body, inputOrigin: "typing", undoGroupId,
          createdAt: new Date(Date.UTC(2026, 7, 15, 8) + batch * 10000 + offset).toISOString(),
        });
      }
      expect(await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
        fetchImpl, cryptoImpl: crypto })).toMatchObject({ body, save_state: "saved",
        unsettled_intent_count: 0 });
      await collectEligibleJournalPayload(workspace);
    }
    const saved = await readJournalSnapshot(workspace);
    expect(saved.records).toHaveLength(2400);
    expect(saved.records.every((record) => record.author_edit_unit === undefined)).toBe(true);
    expect(saved.groups.every((group) => group.payload_collection?.kind === "collected")).toBe(true);
    await restoreVersionThreeJournal(workspace.database);
    workspace = await openEditorWorkspace({ baseUrl: location.origin,
      project: scenario.project, chapter: { ...scenario.chapter,
        project_activity_position: session.base_snapshot.project_activity_position,
        chapter: { ...scenario.chapter.chapter,
          current_revision: session.base_snapshot.materialized_revision },
      }, profile: scenario.profile,
      fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    requireEditorReady(workspace);
    expect(await readJournalSnapshot(workspace)).toEqual(saved);
    const partitionId = workspace.partition.journal_partition_id;
    const put = IDBObjectStore.prototype.put;
    const interrupted = vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
      this: IDBObjectStore, value: unknown, key?: IDBValidKey,
    ) {
      if (this.name === "metadata" && value !== null && typeof value === "object"
        && Reflect.get(value, "key") === `working_boundary:${partitionId}`) {
        this.transaction.abort();
      }
      return key === undefined ? put.call(this, value) : put.call(this, value, key);
    });
    try {
      await expect(persistReplaceSelection(workspace, { from: body.length, to: body.length,
        text: "+", resultingBody: `${body}+`, inputOrigin: "typing" })).rejects.toThrow();
    } finally { interrupted.mockRestore(); }
    expect(await readJournalSnapshot(workspace)).toEqual(saved);
    await persistReplaceSelection(workspace, { from: body.length, to: body.length,
      text: "+", resultingBody: `${body}+`, inputOrigin: "typing" });
    const continued = await readJournalSnapshot(workspace);
    expect(continued.records).toHaveLength(1);
    expect(continued.workingBoundary?.last_sequence).toBe(2400);
    const retained = workspace.database.transaction(["intents", "submission_groups", "metadata"]);
    const [history, groups, retainedFences] = await Promise.all([
      requestResult(retained.objectStore("intents").index("partition").getAll(workspace.partition.journal_partition_id)),
      requestResult(retained.objectStore("submission_groups").index("partition").getAll(workspace.partition.journal_partition_id)),
      Promise.all(saved.fences.map((fence) => requestResult(retained.objectStore("metadata")
        .get(`retained_collection_fence:${Reflect.get(fence as object, "collection_fence_id")}`)))),
    ]);
    const withoutIndex = ({ working_set_partition_id: _index, ...value }: Record<string, unknown>) => value;
    expect(history.slice(0, 2400)).toEqual(saved.records.map(withoutIndex));
    expect(groups).toEqual(saved.groups.map(withoutIndex));
    expect(retainedFences.map((entry) => entry.value)).toEqual(saved.fences);
  } finally {
    if (workspace.kind === "editor-ready") workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 180000);

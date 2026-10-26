import { expect, it } from "vitest";

import type { DigestValue, GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { appendAuthorEdit, openEditorWorkspace, submitOnePendingAuthorEdit } from "../../src/editor-session.ts";
import type { JournalIntentRecord } from "../../src/editor-types.ts";
import { installAuthoritativeBaseSnapshot } from "../../src/undo-latest-author-action.ts";
import {
  BLOCK, OWNER, PROJECT, SESSION, chapterRevision, closeTrackedDatabases, createAppliedAuthorEditResponse,
  createBrowserScenario, deleteJournal, jsonResponse, requestHeaders, requireDigestValue, requireEditorReady,
  requireRequestBody, trackDatabase,
} from "./scenario.ts";

const PROPOSAL_HEAD = "018f0000-0000-7001-8000-0000000000e1";
const NEXT_REVISION = "018f0000-0000-7001-8000-000000000034";

it("settles a stored edge insertion of an earlier client next to an inline Proposal after response loss and reload",
  async () => {
    const scenario = createBrowserScenario();
    const before = scenario.session;
    // After the insertion, the Server shows the Chapter base with no Proposal head.
    const after = { ...scenario.session, base_snapshot: { ...scenario.session.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000038", project_activity_position: "1",
      authoritative_head_revision_id: NEXT_REVISION, materialized_revision: chapterRevision(NEXT_REVISION, "Base!"),
      materialized_payload_digest: { algorithm: "sha256" as const,
        profile: "storyos.canonical-payload.sha256.v1" as const,
        value_hex_lowercase: "f284a09f33779160d1efe782e2ad3f630b0ece4127b45c9f4caf3715c980fae0" },
      proposal_head_revision_ids: [], observed_ownership_partition: "authoritative" as const,
      created_at: "2026-08-15T08:00:00.000Z" } };
    let current = before;
    let lost: { request: Record<string, unknown>; digest: DigestValue; key: string } | undefined;
    let digest: DigestValue | undefined;
    const sent = { authorEdits: 0, outcomes: 0 };
    const fetchImpl: typeof fetch = async (input, init) => {
      const path = new URL(input instanceof Request ? input.url : input).pathname;
      if (path.endsWith("/anti-forgery-challenges")) {
        const request = requireRequestBody(init);
        if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
          digest = requireDigestValue(request.canonical_command_digest, "command digest");
        }
        return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-13T08:05:00.000Z",
          limit_profile_revision: "storyos.foundation.absolute.v1" });
      }
      if (path.endsWith("/editor-sessions")) return jsonResponse(current);
      if (path.endsWith(`/editor-sessions/${SESSION}`)) {
        return jsonResponse({ ...current,
          schema_id: "storyos.query.editor-session.response.v1" } as GetEditorSessionResponse);
      }
      if (path.endsWith("/manuscript/author-edits")) {
        sent.authorEdits += 1;
        lost = { request: requireRequestBody(init), digest: digest!,
          key: requestHeaders(init).get("idempotency-key")! };
        throw new Error("simulated acknowledgement loss");
      }
      if (path.includes("/manuscript/author-edit-outcomes/")) {
        sent.outcomes += 1;
        if (current === before) throw new Error("simulated query unavailable");
        return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000081", project_scope: scenario.project.project_scope,
          outcome: { outcome_kind: "committed", response: createAppliedAuthorEditResponse({ request: lost!.request,
            commandDigest: lost!.digest, idempotencyKey: lost!.key, body: "Base!" }) } });
      }
      throw new Error(`unexpected request: ${path}`);
    };
    const databases = new Set<IDBDatabase>();
    const open = async (chapter = scenario.chapter) => {
      const workspace = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project, chapter,
        profile: scenario.profile, fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
      requireEditorReady(workspace);
      trackDatabase(workspace.database, databases);
      return workspace;
    };
    await deleteJournal(scenario.journalName);
    try {
      const workspace = await open();
      // The earlier client installed a base with one pending inline Proposal in the block.
      workspace.session.base_snapshot = { ...workspace.session.base_snapshot,
        snapshot_id: "018f0000-0000-7001-8000-000000000037",
        proposal_head_revision_ids: [PROPOSAL_HEAD], observed_ownership_partition: "mixed" };
      await installAuthoritativeBaseSnapshot(workspace, workspace.session.base_snapshot);
      await appendAuthorEdit(workspace, { from: 4, to: 4, text: "!", resultingBody: "Base!", inputOrigin: "typing",
        undoGroupId: "018f0000-0000-7001-8000-000000000040", createdAt: "2026-08-15T08:00:00.000Z" });
      // The earlier client stored the Anchor of the inline Proposal that ends at the insertion point.
      const transaction = workspace.database.transaction("intents", "readwrite");
      const store = transaction.objectStore("intents");
      const records = await new Promise<JournalIntentRecord[]>((resolve, reject) => {
        const read = store.getAll();
        read.onsuccess = () => resolve(read.result as JournalIntentRecord[]);
        read.onerror = () => reject(read.error);
      });
      store.put({ ...records[0]!, proposal_anchors: [{ manuscript_block_id: BLOCK,
        coordinate_profile: "prosemirror-token-utf16.v1", from: 0, to: 4,
        base_slice_digest: `sha256:${"0".repeat(64)}` }] });
      await new Promise<void>((resolve, reject) => {
        transaction.oncomplete = () => resolve();
        transaction.onabort = () => reject(transaction.error);
      });
      expect(await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto }))
        .toMatchObject({ save_state: "saving", unsettled_intent_count: 1 });
      workspace.database.close();

      current = after;
      const reopened = await open({ ...scenario.chapter, project_activity_position: "1",
        chapter: { ...scenario.chapter.chapter, current_revision: chapterRevision(NEXT_REVISION, "Base!") } });
      expect(await submitOnePendingAuthorEdit({ workspace: reopened, baseUrl: location.origin, fetchImpl,
        cryptoImpl: crypto })).toMatchObject({ body: "Base!", save_state: "saved", unsettled_intent_count: 0 });
      expect(sent).toEqual({ authorEdits: 1, outcomes: 2 });
    } finally {
      closeTrackedDatabases(databases);
      sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
      await deleteJournal(scenario.journalName);
    }
  });

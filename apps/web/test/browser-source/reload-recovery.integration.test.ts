import { expect, it } from "vitest";

import type {
  DigestValue,
  GetApplyAuthorEditOutcomeResponse,
  GetChapterResponse,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { commitStrongerGroup } from "../../src/author-edit-outcome-reconciliation.ts";
import {
  openEditorWorkspace,
  persistReplaceSelection,
  submitOnePendingAuthorEdit,
} from "../../src/editor-session.ts";
import type {
  EditorReadyState,
  JournalSubmissionGroup,
  ValidatedJournalSnapshot,
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
  requireRecord,
  requireRequestBody,
} from "./scenario.ts";

type OutcomeMode = "challenge" | "committed" | "rejected" | "unavailable";

const NEXT_REVISION = "018f0000-0000-7001-8000-000000000034";
const EXPIRES = "2026-08-13T08:05:00.000Z";
const NONCE = "a".repeat(64);

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

it("recovers ApplyAuthorEdit from a persisted capsule after reload without a second POST", async () => {
  const scenario = createBrowserScenario();
  const freshSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000037",
    base_snapshot: {
      ...scenario.session.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000038",
      project_activity_position: "1",
      authoritative_head_revision_id: NEXT_REVISION,
      materialized_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
      },
      created_at: "2026-08-15T08:00:00.000Z",
    },
  };
  let outcomeMode: OutcomeMode = "unavailable";
  let commandDigest: DigestValue | undefined;
  let lastEdit: Record<string, unknown> | undefined;
  let lastIdempotencyKey: string | undefined;
  let canonicalSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
  };
  const counts = { authorEdits: 0, outcomes: 0, paths: [] as string[] };
  const fetchImpl: typeof fetch = async (input, init) => {
    const parsed = new URL(input instanceof Request ? input.url : input);
    expect(parsed.href).not.toContain(NONCE);
    const path = parsed.pathname;
    counts.paths.push(path);
    expect(path).not.toMatch(/draft/i);
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
        commandDigest = requireDigestValue(request.canonical_command_digest, "command digest");
      }
      return jsonResponse({
        nonce: NONCE,
        expires_at: EXPIRES,
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(canonicalSession);
    if (path.endsWith("/manuscript/author-edits")) {
      lastEdit = requireRequestBody(init);
      lastIdempotencyKey = requestHeaders(init).get("idempotency-key") ?? undefined;
      counts.authorEdits += 1;
      throw new Error("simulated acknowledgement loss");
    }
    if (path.includes("/manuscript/author-edit-outcomes/")) {
      counts.outcomes += 1;
      expect(init?.method).toBe("GET");
      expect(init?.body).toBeUndefined();
      expect(requestHeaders(init).get("x-storyos-anti-forgery")).toBe(NONCE);
      if (outcomeMode === "unavailable") throw new Error("simulated query unavailable");
      let response: GetApplyAuthorEditOutcomeResponse;
      if (outcomeMode === "rejected") {
        response = {
          schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000082",
          project_scope: scenario.project.project_scope,
          outcome: { outcome_kind: "rejected", reason: "challenge_expired_unconsumed" },
        };
      } else if (outcomeMode === "challenge") {
        response = {
          schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000083",
          project_scope: scenario.project.project_scope,
          outcome: {
            outcome_kind: "still_unknown",
            observation: { observation_kind: "challenge_issued", expires_at: EXPIRES },
          },
        };
      } else {
        if (!lastEdit || !commandDigest || !lastIdempotencyKey) {
          throw new Error("the admitted command identity is unavailable");
        }
        canonicalSession = freshSession;
        response = {
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
      }
      return jsonResponse(response);
    }
    throw new Error(`unexpected request: ${path}`);
  };

  async function openReady(
    chapter: GetChapterResponse = scenario.chapter,
  ): Promise<EditorReadyState> {
    const workspace = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(workspace);
    expect(workspace.database.version).toBe(3);
    expect([...workspace.database.objectStoreNames].sort()).toEqual([
      "intents",
      "metadata",
      "outcome_query_attempts",
      "outcome_query_observations",
      "partitions",
      "payload_chains",
      "protocol_observations",
      "submission_groups",
      "transport_attempts",
      "transport_capsules",
    ]);
    return workspace;
  }

  async function journalSnapshot(
    workspace: EditorReadyState,
  ): Promise<ValidatedJournalSnapshot> {
    return validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  }

  function requireGroup(snapshot: ValidatedJournalSnapshot): JournalSubmissionGroup {
    const group = snapshot.groups[0];
    if (!group) throw new Error("the Journal Submission Group is unavailable");
    return group;
  }

  async function countGroupRows(
    database: IDBDatabase,
    store: string,
    groupId: string,
  ): Promise<number> {
    const value: unknown = await requestResult(
      database.transaction(store, "readonly").objectStore(store).index("group").getAll(groupId),
    );
    if (!Array.isArray(value)) throw new Error(`${store} did not return a row array`);
    return value.length;
  }

  async function groupObservations(
    database: IDBDatabase,
    groupId: string,
  ): Promise<Record<string, unknown>[]> {
    const value: unknown = await requestResult(
      database.transaction("outcome_query_observations", "readonly")
        .objectStore("outcome_query_observations").index("group").getAll(groupId),
    );
    if (!Array.isArray(value)) throw new Error("outcome observations are unavailable");
    return value.map((item, index) => requireRecord(item, `outcome observation ${index}`));
  }

  async function loseAcknowledgement(): Promise<{
    groupId: string;
    workspace: EditorReadyState;
  }> {
    canonicalSession = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
    outcomeMode = "unavailable";
    const workspace = await openReady();
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
      save_state: "saving",
      unsettled_intent_count: 1,
      authoritative_revision_id: REVISION,
    });
    const group = requireGroup(await journalSnapshot(workspace));
    expect(group.reconciliation?.kind).toBe("outcome_query_unresolved");
    return { workspace, groupId: group.journal_submission_group_id };
  }

  async function closeScenario(workspace: EditorReadyState): Promise<void> {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }

  await deleteJournal(scenario.journalName);
  try {
    const staleRequest = indexedDB.open(scenario.journalName, 2);
    staleRequest.onupgradeneeded = () => {
      const database = staleRequest.result;
      database.createObjectStore("metadata", { keyPath: "key" });
      database.createObjectStore("partitions", { keyPath: "journal_partition_id" });
      database.createObjectStore("payload_chains", { keyPath: "payload_chain_id" })
        .createIndex("partition", "journal_partition_id", { unique: false });
      database.createObjectStore("intents", {
        keyPath: ["journal_partition_id", "local_intent_sequence"],
      }).createIndex("partition", "journal_partition_id", { unique: false });
      database.createObjectStore("submission_groups", {
        keyPath: "journal_submission_group_id",
      }).createIndex("partition", "journal_partition_id", { unique: false });
      staleRequest.transaction?.objectStore("metadata").put({ key: "schema", version: 2 });
    };
    const stale = await requestResult(staleRequest);
    stale.close();
    const refused = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    expect(refused).toMatchObject({
      kind: "editor-read-only-recovery",
      code: "local_journal_unavailable",
    });
    await deleteJournal(scenario.journalName);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);

    counts.authorEdits = 0;
    counts.outcomes = 0;
    counts.paths = [];
    let unresolved = await loseAcknowledgement();
    expect(JSON.stringify(requireGroup(await journalSnapshot(unresolved.workspace))))
      .not.toContain(NONCE);
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 1 });
    expect(await countGroupRows(
      unresolved.workspace.database,
      "outcome_query_attempts",
      unresolved.groupId,
    )).toBe(1);
    unresolved.workspace.database.close();

    canonicalSession = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
    outcomeMode = "committed";
    let reopened = await openReady();
    expect(await submitOnePendingAuthorEdit({
      workspace: reopened,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toEqual({
      body: "Base!?",
      save_state: "saved",
      unsettled_intent_count: 0,
      authoritative_revision_id: NEXT_REVISION,
    });
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 2 });
    expect(await countGroupRows(
      reopened.database,
      "outcome_query_attempts",
      unresolved.groupId,
    )).toBe(2);
    expect(await countGroupRows(
      reopened.database,
      "outcome_query_observations",
      unresolved.groupId,
    )).toBe(1);
    const settled = requireGroup(await journalSnapshot(reopened));
    const committedObservation = (await groupObservations(reopened.database, unresolved.groupId))[0];
    if (!committedObservation) throw new Error("the committed observation is unavailable");
    expect(requireRecord(committedObservation.outcome, "observation outcome").kind).toBe("committed");
    expect(settled.settlement.kind).toBe("applied_receipt_settled");
    expect(committedObservation.local_response_payload_ref)
      .toBe(committedObservation.outcome_query_observation_id);
    expect(committedObservation.local_response_payload).toContain('"outcome_kind":"committed"');
    const weaker: JournalSubmissionGroup = {
      ...settled,
      settlement: { kind: "unsettled" },
      reconciliation: {
        kind: "outcome_query_unresolved",
        strongest: { kind: "no_outcome_observed" },
      },
    };
    await expect(commitStrongerGroup(reopened, weaker))
      .rejects.toThrow(/acknowledgement does not converge/);
    reopened.database.close();
    canonicalSession = freshSession;
    const afterSettle = await openReady({
      ...scenario.chapter,
      project_activity_position: "1",
      chapter: {
        ...scenario.chapter.chapter,
        current_revision: { revision_id: NEXT_REVISION, body: "Base!?" },
      },
    });
    await expect(submitOnePendingAuthorEdit({
      workspace: afterSettle,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).rejects.toThrow(/One pending Author Edit is required/);
    expect(counts.authorEdits).toBe(1);
    await closeScenario(afterSettle);

    for (const mode of ["rejected", "challenge"] as const) {
      counts.authorEdits = 0;
      counts.outcomes = 0;
      counts.paths = [];
      unresolved = await loseAcknowledgement();
      unresolved.workspace.database.close();
      canonicalSession = {
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v1",
      };
      outcomeMode = mode;
      reopened = await openReady();
      const projection = await submitOnePendingAuthorEdit({
        workspace: reopened,
        baseUrl: location.origin,
        fetchImpl,
        cryptoImpl: crypto,
      });
      expect(projection).toEqual(mode === "rejected" ? {
        body: "Base!?",
        save_state: "needs_attention",
        unsettled_intent_count: 1,
        authoritative_revision_id: REVISION,
      } : {
        body: "Base!?",
        save_state: "saving",
        unsettled_intent_count: 1,
        authoritative_revision_id: REVISION,
      });
      expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
        .toEqual({ authorEdits: 1, outcomes: 2 });
      const group = requireGroup(await journalSnapshot(reopened));
      if (mode === "rejected") {
        expect(group.settlement.kind).toBe("outcome_query_rejected_no_admission");
      } else {
        expect(group.reconciliation).toEqual({
          kind: "outcome_query_unresolved",
          strongest: { kind: "challenge_issued", expires_at: EXPIRES },
        });
      }
      const observation = (await groupObservations(reopened.database, unresolved.groupId))[0];
      if (!observation) throw new Error("the reload observation is unavailable");
      expect(requireRecord(observation.outcome, "observation outcome").kind)
        .toBe(mode === "rejected" ? "rejected_no_admission" : "still_unknown");
      await closeScenario(reopened);
    }

    counts.authorEdits = 0;
    counts.outcomes = 0;
    unresolved = await loseAcknowledgement();
    unresolved.workspace.database.close();
    const opened = await requestResult(indexedDB.open(scenario.journalName));
    const clear = opened.transaction("transport_capsules", "readwrite");
    clear.objectStore("transport_capsules").clear();
    await transactionResult(clear);
    opened.close();
    canonicalSession = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
    outcomeMode = "committed";
    reopened = await openReady();
    expect(await submitOnePendingAuthorEdit({
      workspace: reopened,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toMatchObject({ save_state: "saving", unsettled_intent_count: 1 });
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 1 });
    expect(requireGroup(await journalSnapshot(reopened)).reconciliation?.strongest.kind)
      .toBe("no_outcome_observed");
    await closeScenario(reopened);

    counts.authorEdits = 0;
    counts.outcomes = 0;
    unresolved = await loseAcknowledgement();
    unresolved.workspace.database.close();
    canonicalSession = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
    outcomeMode = "unavailable";
    reopened = await openReady();
    expect(await submitOnePendingAuthorEdit({
      workspace: reopened,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toMatchObject({ save_state: "saving", unsettled_intent_count: 1 });
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 2 });
    await closeScenario(reopened);
  } finally {
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

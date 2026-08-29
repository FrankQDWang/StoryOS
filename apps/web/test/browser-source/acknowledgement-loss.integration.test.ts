import { expect, it } from "vitest";

import type {
  ApplyAuthorEditResponse,
  DigestValue,
  GetApplyAuthorEditOutcomeResponse,
  GetEditorSessionResponse,
  ProjectScope,
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
  OutcomeQueryReconciliation,
  SubmissionSettlement,
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
  chapterRevision,
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

type OutcomeMode =
  | "admission"
  | "challenge"
  | "committed"
  | "malformed"
  | "mismatch"
  | "rejected"
  | "unavailable";

const NEXT_REVISION = "018f0000-0000-7001-8000-000000000034";
const NEXT_COMMIT = "018f0000-0000-7001-8000-000000000035";
const EXPIRES = "2026-08-13T08:05:00.000Z";
const NONCE = "a".repeat(64);

function requireGroup(snapshot: ValidatedJournalSnapshot): JournalSubmissionGroup {
  const group = snapshot.groups[0];
  if (!group) throw new Error("the Journal Submission Group is unavailable");
  return group;
}

function assertGroup(
  snapshot: ValidatedJournalSnapshot,
  settlement: SubmissionSettlement,
  reconciliation?: OutcomeQueryReconciliation,
): JournalSubmissionGroup {
  const group = requireGroup(snapshot);
  expect(group.settlement).toEqual(settlement);
  expect(group.reconciliation).toEqual(reconciliation);
  expect(group.batch_policy_revision).toBe("storyos.author-edit-batch.release-1.preview.v1");
  expect(group.editor_session_id).toBe(SESSION);
  expect(group.writer_generation).toBe("1");
  expect(group.frozen_request_body).toMatchObject({
    command_schema: "storyos.command.apply-author-edit.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    editor_session_id: SESSION,
    writer_generation: "1",
    chapter_id: CHAPTER,
    expected_authoritative_revision_id: REVISION,
    expected_proposal_head_revision_ids: [],
    observed_ownership_partition: "authoritative",
    editor_contract_revision: "storyos.editor-contract.release-1.v2",
  });
  expect(group.ordered_coverage).toHaveLength(snapshot.records.length);
  return group;
}

it("converges lost ApplyAuthorEdit acknowledgement from persistent outcome evidence", async () => {
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
      materialized_revision: chapterRevision(NEXT_REVISION, "Base!?"),
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
      },
      created_at: "2026-08-15T08:00:00.000Z",
    },
  };
  let outcomeMode: OutcomeMode = "committed";
  let commandDigest: DigestValue | undefined;
  let lastEdit: Record<string, unknown> | undefined;
  let lastIdempotencyKey: string | undefined;
  let canonicalSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
  };
  const counts = { authorEdits: 0, methods: [] as string[], outcomes: 0 };
  const openDatabases = new Set<IDBDatabase>();
  const fetchImpl: typeof fetch = async (input, init) => {
    const parsed = new URL(input instanceof Request ? input.url : input);
    expect(parsed.href).not.toContain(NONCE);
    const path = parsed.pathname;
    counts.methods.push(`${init?.method ?? "GET"} ${path}`);
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
      if (outcomeMode === "malformed") {
        return jsonResponse({ schema_id: "storyos.query.not-an-outcome.response.v1" });
      }
      if (!lastEdit || !commandDigest || !lastIdempotencyKey) {
        throw new Error("the admitted command identity is unavailable");
      }
      const scope: ProjectScope = outcomeMode === "mismatch"
        ? { owner_user_id: "018f0000-0000-7001-8000-000000000099", project_id: PROJECT }
        : scenario.project.project_scope;
      const baseResponse = createAppliedAuthorEditResponse({
        request: lastEdit,
        commandDigest,
        idempotencyKey: lastIdempotencyKey,
      });
      const committedResponse: ApplyAuthorEditResponse = {
        ...baseResponse,
        project_scope: scope,
        receipt: { ...baseResponse.receipt, project_scope: scope },
      };
      let response: GetApplyAuthorEditOutcomeResponse;
      if (outcomeMode === "rejected") {
        response = {
          schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000082",
          project_scope: scope,
          outcome: { outcome_kind: "rejected", reason: "challenge_expired_unconsumed" },
        };
      } else if (outcomeMode === "challenge") {
        response = {
          schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000083",
          project_scope: scope,
          outcome: {
            outcome_kind: "still_unknown",
            observation: { observation_kind: "challenge_issued", expires_at: EXPIRES },
          },
        };
      } else if (outcomeMode === "admission") {
        response = {
          schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000084",
          project_scope: scope,
          outcome: {
            outcome_kind: "still_unknown",
            observation: {
              observation_kind: "admission_committed",
              command_id: "018f0000-0000-7001-8000-000000000031",
              author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
              reconciliation_required: true,
            },
          },
        };
      } else {
        response = {
          schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000081",
          project_scope: scope,
          outcome: { outcome_kind: "committed", response: committedResponse },
        };
      }
      if (outcomeMode === "committed" || outcomeMode === "mismatch") {
        canonicalSession = freshSession;
      }
      return jsonResponse(response);
    }
    throw new Error(`unexpected request: ${path}`);
  };

  async function openReady(): Promise<EditorReadyState> {
    canonicalSession = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
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

  async function persistPending(workspace: EditorReadyState): Promise<void> {
    await persistReplaceSelection(workspace, {
      from: 4,
      to: 4,
      text: "!?",
      resultingBody: "Base!?",
      inputOrigin: "paste",
      undoGroupId: "018f0000-0000-7001-8000-000000000040",
      createdAt: "2026-08-15T08:00:00.000Z",
    });
  }

  async function snapshot(workspace: EditorReadyState): Promise<ValidatedJournalSnapshot> {
    return validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  }

  function appliedSettlement(group: JournalSubmissionGroup): SubmissionSettlement {
    return {
      kind: "applied_receipt_settled",
      command_id: "018f0000-0000-7001-8000-000000000031",
      author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
      receipt: {
        receipt_id: "018f0000-0000-7001-8000-000000000033",
        project_scope: scenario.project.project_scope,
        command_kind: "applyAuthorEdit",
        command_digest: group.frozen_request_digest,
        idempotency_key: group.idempotency_key,
        producer_cause: "author_command_admission",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        expected_heads: [REVISION],
        prior_heads: [REVISION],
        resulting_heads: [NEXT_REVISION],
        authoritative_revision_ids: [NEXT_REVISION],
        proposal_revision_ids: [],
        authoritative_commit_ids: [NEXT_COMMIT],
        author_action_sequence: "1",
        draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [],
        condition_refs: [],
        result: "authoritative_applied",
        created_at: "2026-08-15T08:00:00.000Z",
      },
      authoritative_revision: chapterRevision(NEXT_REVISION, "Base!?"),
      authoritative_commit_id: NEXT_COMMIT,
      author_action_sequence: "1",
      project_activity_position: "1",
      installed_base_snapshot: freshSession.base_snapshot,
    };
  }

  async function closeScenario(workspace: EditorReadyState): Promise<void> {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }

  async function persistBlocked(workspace: EditorReadyState): Promise<void> {
    await expect(persistReplaceSelection(workspace, {
      from: 6,
      to: 6,
      text: "+",
      resultingBody: "Base!?+",
      inputOrigin: "typing",
      undoGroupId: "018f0000-0000-7001-8000-000000000041",
      createdAt: "2026-08-15T08:00:01.000Z",
    })).rejects.toThrow(/limit failed|incompatible|settlement/);
  }

  await deleteJournal(scenario.journalName);
  try {
    outcomeMode = "committed";
    counts.authorEdits = 0;
    counts.outcomes = 0;
    let workspace = await openReady();
    await persistPending(workspace);
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
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 1 });
    const committedSnapshot = await snapshot(workspace);
    const committedGroup = assertGroup(
      committedSnapshot,
      appliedSettlement(requireGroup(committedSnapshot)),
    );
    expect(JSON.stringify(committedGroup)).not.toContain(NONCE);
    await expect(submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).rejects.toThrow(/One pending Author Edit is required/);
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 1 });
    const weaker: JournalSubmissionGroup = {
      ...committedGroup,
      settlement: { kind: "unsettled" },
      reconciliation: {
        kind: "outcome_query_unresolved",
        strongest: { kind: "no_outcome_observed" },
      },
    };
    await expect(commitStrongerGroup(workspace, weaker))
      .rejects.toThrow(/acknowledgement does not converge/);
    expect(requireGroup(await snapshot(workspace))).toEqual(committedGroup);
    await closeScenario(workspace);

    outcomeMode = "rejected";
    counts.authorEdits = 0;
    counts.outcomes = 0;
    workspace = await openReady();
    await persistPending(workspace);
    expect(await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toEqual({
      body: "Base!?",
      save_state: "needs_attention",
      unsettled_intent_count: 1,
      authoritative_revision_id: REVISION,
    });
    assertGroup(await snapshot(workspace), {
      kind: "outcome_query_rejected_no_admission",
      reason: "challenge_expired_unconsumed",
    });
    await persistBlocked(workspace);
    await closeScenario(workspace);

    outcomeMode = "challenge";
    counts.authorEdits = 0;
    counts.outcomes = 0;
    workspace = await openReady();
    await persistPending(workspace);
    expect(await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).toMatchObject({ save_state: "saving", unsettled_intent_count: 1 });
    assertGroup(await snapshot(workspace), { kind: "unsettled" }, {
      kind: "outcome_query_unresolved",
      strongest: { kind: "challenge_issued", expires_at: EXPIRES },
    });
    await persistBlocked(workspace);
    await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    });
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 2 });
    await closeScenario(workspace);

    outcomeMode = "admission";
    counts.authorEdits = 0;
    counts.outcomes = 0;
    workspace = await openReady();
    await persistPending(workspace);
    await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    });
    assertGroup(await snapshot(workspace), { kind: "unsettled" }, {
      kind: "outcome_query_unresolved",
      strongest: {
        kind: "admission_committed",
        command_id: "018f0000-0000-7001-8000-000000000031",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        reconciliation_required: true,
      },
    });
    await submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    });
    expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
      .toEqual({ authorEdits: 1, outcomes: 2 });
    await closeScenario(workspace);

    for (const mode of ["unavailable", "malformed"] as const) {
      outcomeMode = mode;
      counts.authorEdits = 0;
      counts.outcomes = 0;
      workspace = await openReady();
      await persistPending(workspace);
      expect(await submitOnePendingAuthorEdit({
        workspace,
        baseUrl: location.origin,
        fetchImpl,
        cryptoImpl: crypto,
      })).toMatchObject({ save_state: "saving", unsettled_intent_count: 1 });
      assertGroup(await snapshot(workspace), { kind: "unsettled" }, {
        kind: "outcome_query_unresolved",
        strongest: { kind: "no_outcome_observed" },
      });
      if (mode === "unavailable") {
        await submitOnePendingAuthorEdit({
          workspace,
          baseUrl: location.origin,
          fetchImpl,
          cryptoImpl: crypto,
        });
        expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
          .toEqual({ authorEdits: 1, outcomes: 2 });
      } else {
        expect({ authorEdits: counts.authorEdits, outcomes: counts.outcomes })
          .toEqual({ authorEdits: 1, outcomes: 1 });
      }
      await closeScenario(workspace);
    }

    outcomeMode = "mismatch";
    workspace = await openReady();
    await persistPending(workspace);
    await expect(submitOnePendingAuthorEdit({
      workspace,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
    })).rejects.toThrow(/acknowledgement does not converge/);
    assertGroup(await snapshot(workspace), { kind: "unsettled" }, {
      kind: "outcome_query_unresolved",
      strongest: { kind: "no_outcome_observed" },
    });
    await closeScenario(workspace);
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

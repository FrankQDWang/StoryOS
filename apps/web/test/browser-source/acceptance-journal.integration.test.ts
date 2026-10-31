import { expect, it } from "vitest";

import { digestAcceptProposal, digestRejectProposalOperations }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, RejectProposalOperationsRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  acceptanceJournalProposals, createFlight, parsePreAdmissionAcceptanceProblem,
  readAcceptanceJournal, writeFlight, type AcceptanceSettlement,
} from "../../src/acceptance-journal.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { requestResult } from "./scenario.ts";

const historyId = (kind: number, index: number) =>
  `018f0000-0000-7001-8${kind}00-${String(index).padStart(12, "0")}`;

async function decide(workspace: EditorReadyState, index: number, proposalId: string,
  outcome: "settled" | "refused" | "problem" | "unsettled", rejection = false) {
  const partition = workspace.partition;
  const revisionId = historyId(1, index);
  const operationId = historyId(2, index);
  const correlationId = historyId(3, index);
  const shared = {
    kind: "explicit_editor_command" as const,
    explicit_command_record_id: historyId(4, index),
    // Group keys sort against the record order, so the reader joins two different orders.
    journal_submission_group_id: historyId(5, 999_999 - index),
    journal_partition_id: partition.journal_partition_id,
    project_scope: partition.project_scope,
    editor_session_id: partition.editor_session_id,
    writer_generation: partition.writer_generation,
    author_visible_decision_ref: { proposal_id: proposalId, operation_id: operationId,
      revision_id: revisionId },
    settlement: "frozen" as const,
    proposalId,
    idempotencyKey: historyId(6, index),
  };
  const problemBody = JSON.stringify({ schema_id: "storyos.problem.v1",
    code: "challenge_invalid", message: "The decision challenge is invalid." });
  const problem = { kind: "pre_admission_problem" as const,
    problem: parsePreAdmissionAcceptanceProblem(422, problemBody)! };
  if (rejection) {
    const request: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: { proposal_revision_id: revisionId,
        selected_pending_operation_ids: [operationId],
        expected_target_revisions: [workspace.session.base_snapshot.authoritative_head_revision_id],
        rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
        editor_session_id: partition.editor_session_id,
        client_contract_revision: partition.client_contract_revision,
        security_policy_revision: partition.security_policy_revision,
        correlation_id: correlationId },
    };
    const flight = await createFlight(workspace, { ...shared, request,
      key: `rejection:${partition.journal_partition_id}:${proposalId}:${revisionId}:${operationId}`,
      command_kind: "rejectProposalOperations",
      frozen_request_digest: await digestRejectProposalOperations(request, crypto) });
    await writeFlight(workspace.database, flight, problem);
    return;
  }
  const request: AcceptProposalRequest = {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: { proposal_revision_id: revisionId,
      validation_receipt_id: historyId(7, index), selected_operation_ids: [operationId],
      expected_authoritative_revision_id:
        workspace.session.base_snapshot.authoritative_head_revision_id,
      editor_session_id: partition.editor_session_id,
      client_contract_revision: partition.client_contract_revision,
      security_policy_revision: partition.security_policy_revision,
      correlation_id: correlationId },
  };
  const digest = await digestAcceptProposal(request, crypto);
  const flight = await createFlight(workspace, { ...shared, request,
    key: `acceptance:${partition.journal_partition_id}:${proposalId}:${revisionId}:${operationId}`,
    command_kind: "acceptProposal", frozen_request_digest: digest });
  const head = workspace.session.base_snapshot.authoritative_head_revision_id;
  const settlement: AcceptanceSettlement | undefined = outcome === "unsettled" ? undefined
    : outcome === "problem" ? problem
      : outcome === "refused" ? { kind: "refused", refusal: { kind: "present",
        refusal_id: historyId(8, index), correlation_id: correlationId,
        reason: "invalid_challenge", boundary: "challenge",
        command_schema: request.command_schema,
        refusal_profile_revision: "storyos.acceptance-refusal.release-1.v1",
        client_contract_revision: partition.client_contract_revision,
        security_policy_revision: partition.security_policy_revision,
        limit_profile_revision: partition.limit_profile_revision,
        challenge_rate_policy_revision: "storyos.challenge-rate.release-1.v1",
        recorded_at: "2026-09-25T08:00:00.000Z" } }
        : { kind: "settled", response: {
          schema_id: "storyos.command.accept-proposal.response.v1",
          correlation_id: correlationId, project_scope: partition.project_scope,
          command_id: historyId(9, index), author_command_admission_id: historyId(9, index),
          receipt: { receipt_id: historyId(8, index), project_scope: partition.project_scope,
            command_digest: digest, idempotency_key: shared.idempotencyKey,
            author_command_admission_id: historyId(9, index), proposal_id: proposalId,
            proposal_revision_id: revisionId, validation_receipt_id: historyId(7, index),
            selected_operation_ids: [operationId], prior_authoritative_revision_ids: [head],
            resulting_authoritative_revision_ids: [head], authoritative_commit_ids: [],
            condition_refs: [], result: "conflicted", created_at: "2026-09-25T08:00:00.000Z" },
          project: { project_id: partition.project_scope.project_id, title: "Project A",
            open: { kind: "current_chapter",
              current_chapter_id: workspace.session.base_snapshot.chapter_id } },
          effect: { kind: "conflicted", reason: "changed_head" },
        } };
  if (settlement !== undefined) await writeFlight(workspace.database, flight, settlement);
}

it("reads the same decisions and recovery lists from a large retained history", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    const { workspace } = test;
    const proposalIds = new Set<string>();
    const unresolvedIds = new Set<string>();
    const outcomes = ["settled", "refused", "problem", "unsettled"] as const;
    for (let index = 0; index < 240; index++) {
      const proposalId = historyId(0, index - (index % 6 === 5 ? 5 : 0));
      if (index % 6 === 4) {
        await decide(workspace, index, proposalId, "problem", true);
        continue;
      }
      const outcome = index % 6 === 5 ? (index % 12 === 5 ? "unsettled" : "refused")
        : outcomes[index % 6]!;
      await decide(workspace, index, proposalId, outcome);
      proposalIds.add(proposalId);
      if (outcome === "unsettled") unresolvedIds.add(proposalId);
    }
    const expected = { proposalIds: [...proposalIds].sort(), unresolvedIds: [...unresolvedIds].sort() };
    const partitionId = workspace.partition.journal_partition_id;
    const stored = async () => {
      const transaction = workspace.database.transaction(["intents", "submission_groups"]);
      return { records: await requestResult(transaction.objectStore("intents").index("partition")
        .getAll(partitionId)), groups: await requestResult(transaction
        .objectStore("submission_groups").index("partition").getAll(partitionId)) };
    };
    const history = await stored();
    expect(history.records.map((record) => record.explicit_command_record_id))
      .toEqual(Array.from({ length: 240 }, (_, index) => historyId(4, index)));
    expect(history.groups.map((group) => group.ordered_coverage[0].intent_record_ref))
      .toEqual(Array.from({ length: 240 }, (_, index) => historyId(4, 239 - index)));
    expect(await readAcceptanceJournal(workspace)).toEqual(history);
    expect(await acceptanceJournalProposals(workspace)).toEqual(expected);

    const group = history.groups[17] as Record<string, unknown>;
    const corrupt = workspace.database.transaction("submission_groups", "readwrite");
    await requestResult(corrupt.objectStore("submission_groups").put({ ...group,
      ordered_coverage: [{ ...(group.ordered_coverage as object[])[0],
        intent_record_ref: historyId(4, 240) }] }));
    await expect(readAcceptanceJournal(workspace)).rejects.toThrow(/Acceptance Journal is corrupt/);
    await expect(acceptanceJournalProposals(workspace))
      .rejects.toThrow(/Acceptance Journal is corrupt/);
    const restore = workspace.database.transaction("submission_groups", "readwrite");
    await requestResult(restore.objectStore("submission_groups").put(group));
    expect(await acceptanceJournalProposals(workspace)).toEqual(expected);
  } finally {
    await test.close();
  }
}, 120000);

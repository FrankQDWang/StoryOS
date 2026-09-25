import { expect, it } from "vitest";

import { digestAcceptProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createFlight, type AcceptanceFlight } from "../../src/acceptance-journal.ts";
import { readJournalSnapshot, rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";

it("rejects a foreign Acceptance flight before it can disappear from Journal recovery", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    const { workspace } = test;
    const proposalId = "018f0000-0000-7001-8000-000000000101";
    const operationId = "018f0000-0000-7001-8000-000000000102";
    const revisionId = "018f0000-0000-7001-8000-000000000103";
    const receiptId = "018f0000-0000-7001-8000-000000000104";
    const request: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: revisionId,
        validation_receipt_id: receiptId,
        selected_operation_ids: [operationId],
        expected_authoritative_revision_id:
          workspace.session.base_snapshot.authoritative_head_revision_id,
        editor_session_id: workspace.partition.editor_session_id,
        client_contract_revision: workspace.partition.client_contract_revision,
        security_policy_revision: workspace.partition.security_policy_revision,
        correlation_id: "018f0000-0000-7001-8000-000000000105",
      },
    };
    const flight: Omit<AcceptanceFlight, "local_intent_sequence"> = {
      key: `acceptance:${workspace.partition.journal_partition_id}:${proposalId}:${revisionId}:${operationId}`,
      kind: "explicit_editor_command",
      explicit_command_record_id: "018f0000-0000-7001-8000-000000000106",
      journal_submission_group_id: "018f0000-0000-7001-8000-000000000107",
      journal_partition_id: workspace.partition.journal_partition_id,
      project_scope: workspace.partition.project_scope,
      editor_session_id: workspace.partition.editor_session_id,
      writer_generation: workspace.partition.writer_generation,
      command_kind: "acceptProposal",
      author_visible_decision_ref: { proposal_id: proposalId,
        operation_id: operationId, revision_id: revisionId },
      frozen_request_digest: await digestAcceptProposal(request, crypto),
      settlement: "frozen",
      proposalId,
      idempotencyKey: "018f0000-0000-7001-8000-000000000108",
      request,
    };
    await expect(createFlight(workspace, { ...flight, key: "hidden-flight" }))
      .rejects.toThrow(/frozen command/);
    await expect(createFlight(workspace, { ...flight,
      journal_partition_id: "018f0000-0000-7001-8000-000000000109",
      key: `acceptance:018f0000-0000-7001-8000-000000000109:${proposalId}:${revisionId}:${operationId}`,
    })).rejects.toThrow(/partition changed/);
    expect((await readJournalSnapshot(workspace)).explicitAcceptance)
      .toEqual({ records: [], groups: [] });

    const created = await createFlight(workspace, flight);
    expect(created.local_intent_sequence).toBe(1);
    const snapshot = await readJournalSnapshot(workspace);
    expect(snapshot.explicitAcceptance?.records).toHaveLength(1);
    expect(snapshot.explicitAcceptance?.groups).toHaveLength(1);
    const pending = await rebuildPendingProjection(workspace);
    expect(pending.save_state).toBe("needs_attention");
    expect(pending.unsettled_intent_count).toBe(1);
  } finally {
    await test.close();
  }
});

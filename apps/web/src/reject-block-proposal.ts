import { createProjectCommandChallenge, digestRejectProposalOperations, rejectProposalOperations,
  StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { RejectProposalOperationsRequest, RejectProposalOperationsResponse }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE }
  from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { EditorReadyState } from "./editor-types.ts";
import type { EditorWorkspace } from "./editor-types.ts";
import { beginAcceptanceAttempt, finishAcceptanceAttempt } from "./acceptance-transport.ts";
import { createFlight, decisionInput, parsePreAdmissionAcceptanceProblem,
  readAcceptanceJournal, readFlight, uuidV7, writeFlight, type RejectionFlight,
  type RejectionSettlement } from "./acceptance-journal.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type RejectionOptions = {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  workspace: EditorReadyState;
  proposalId: string;
  operationId: string;
  proposalRevisionId: string;
  targetRevisionId: string;
};

function rejectionKey(options: RejectionOptions): string {
  return `rejection:${options.workspace.partition.journal_partition_id}:${options.proposalId}:${options.proposalRevisionId}:${options.operationId}`;
}

export async function rejectionJournalState(workspace: EditorWorkspace): Promise<{
  proposalIds: string[];
  pendingIds: string[];
  settledIds: string[];
}> {
  const journal = await readAcceptanceJournal(workspace);
  const proposalIds = new Set<string>();
  const pendingIds = new Set<string>();
  const settledIds = new Set<string>();
  for (const record of journal.records) {
    if (record.command_kind !== "rejectProposalOperations") continue;
    const id = (record.author_visible_decision_ref as { proposal_id: string }).proposal_id;
    const group = journal.groups.find((item) =>
      (item.ordered_coverage as { intent_record_ref: string }[])?.[0]?.intent_record_ref
        === record.explicit_command_record_id);
    proposalIds.add(id);
    if ((group?.settlement as { kind?: string })?.kind === "unsettled") pendingIds.add(id);
    if ((group?.settlement as { kind?: string })?.kind === "settled") settledIds.add(id);
  }
  return { proposalIds: [...proposalIds], pendingIds: [...pendingIds],
    settledIds: [...settledIds] };
}

export async function rejectDisplayedBlockProposal(options: RejectionOptions):
  Promise<RejectProposalOperationsResponse> {
  if (globalThis.navigator?.locks === undefined) {
    throw new Error("Protected Rejection transport lock is unavailable");
  }
  return navigator.locks.request(`storyos-decision:${options.workspace.partition.journal_partition_id}:${options.proposalId}`,
    () => rejectLocked(options));
}

export async function retryPendingDisplayedRejection(options: Pick<RejectionOptions,
  "baseUrl" | "fetchImpl" | "cryptoImpl" | "workspace" | "proposalId">):
  Promise<RejectProposalOperationsResponse> {
  const prefix = `rejection:${options.workspace.partition.journal_partition_id}:${options.proposalId}:`;
  const request = options.workspace.database.transaction("metadata", "readonly")
    .objectStore("metadata").getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`));
  const flights = await new Promise<RejectionFlight[]>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result as RejectionFlight[]);
    request.onerror = () => reject(request.error ?? new Error("Rejection record read failed"));
  });
  if (flights.length !== 1 || flights[0]?.command_kind !== "rejectProposalOperations") {
    throw new Error("Rejection recovery is not available");
  }
  const flight = flights[0];
  return rejectDisplayedBlockProposal({ ...options,
    operationId: flight.author_visible_decision_ref.operation_id,
    proposalRevisionId: flight.request.reject_proposal_operations_input.proposal_revision_id,
    targetRevisionId: flight.request.reject_proposal_operations_input.expected_target_revisions[0] ?? "",
  });
}

async function rejectLocked(options: RejectionOptions): Promise<RejectProposalOperationsResponse> {
  const workspace = options.workspace;
  const key = rejectionKey(options);
  let flight = await readFlight(workspace.database, key);
  if (flight !== undefined && flight.command_kind !== "rejectProposalOperations") {
    throw new Error("A prior Rejection decision is unresolved");
  }
  const journal = await readAcceptanceJournal(workspace);
  if (flight === undefined) {
    const prior = journal.records.find((record) => record.command_kind === "rejectProposalOperations"
      && JSON.stringify(record.author_visible_decision_ref) === JSON.stringify({
        proposal_id: options.proposalId, operation_id: options.operationId,
        revision_id: options.proposalRevisionId,
      }));
    if (prior !== undefined) {
      const group = journal.groups.find((item) =>
        (item.ordered_coverage as { intent_record_ref: string }[])?.[0]?.intent_record_ref
          === prior.explicit_command_record_id);
      const settlement = group?.settlement as RejectionSettlement | undefined;
      const input = (group?.frozen_request_body as RejectProposalOperationsRequest | undefined)
        ?.reject_proposal_operations_input;
      if (settlement?.kind === "settled"
        && JSON.stringify(input?.expected_target_revisions) === JSON.stringify(
          [options.targetRevisionId])) return settlement.response;
      throw new Error("A prior Rejection decision is unresolved");
    }
    const request: RejectProposalOperationsRequest = {
      command_schema: "storyos.command.reject-proposal-operations.request.v1",
      reject_proposal_operations_input: {
        proposal_revision_id: options.proposalRevisionId,
        selected_pending_operation_ids: [options.operationId],
        expected_target_revisions: [options.targetRevisionId],
        rejection_reason: { kind: "author_declined", note: { kind: "omitted" } },
        editor_session_id: workspace.partition.editor_session_id,
        client_contract_revision:
          RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: SECURITY_POLICY_REVISION,
        correlation_id: uuidV7(options.cryptoImpl),
      },
    };
    flight = await createFlight(workspace, {
      key, kind: "explicit_editor_command",
      explicit_command_record_id: uuidV7(options.cryptoImpl),
      journal_submission_group_id: uuidV7(options.cryptoImpl),
      journal_partition_id: workspace.partition.journal_partition_id,
      project_scope: workspace.partition.project_scope,
      editor_session_id: workspace.partition.editor_session_id,
      writer_generation: workspace.partition.writer_generation,
      command_kind: "rejectProposalOperations",
      author_visible_decision_ref: { proposal_id: options.proposalId,
        operation_id: options.operationId, revision_id: options.proposalRevisionId },
      frozen_request_digest: await digestRejectProposalOperations(request, options.cryptoImpl),
      settlement: "frozen", proposalId: options.proposalId,
      idempotencyKey: uuidV7(options.cryptoImpl), request,
    });
  }
  const input = decisionInput(flight);
  const group = journal.groups.find((item) => item.journal_submission_group_id
    === flight.journal_submission_group_id);
  const record = journal.records.find((item) => item.explicit_command_record_id
    === flight.explicit_command_record_id);
  if (flight.key !== key || flight.proposalId !== options.proposalId
    || group === undefined || record === undefined
    || (group.settlement as { kind?: string }).kind !== "unsettled"
    || group.idempotency_key !== flight.idempotencyKey
    || JSON.stringify(group.frozen_request_body) !== JSON.stringify(flight.request)
    || record.local_intent_sequence !== flight.local_intent_sequence
    || flight.journal_partition_id !== workspace.partition.journal_partition_id
    || JSON.stringify(flight.project_scope) !== JSON.stringify(workspace.partition.project_scope)
    || flight.editor_session_id !== workspace.partition.editor_session_id
    || flight.writer_generation !== workspace.partition.writer_generation
    || flight.request.command_schema !== "storyos.command.reject-proposal-operations.request.v1"
    || input.proposal_revision_id !== options.proposalRevisionId
    || JSON.stringify(input.selected_operation_ids) !== JSON.stringify([options.operationId])
    || JSON.stringify(input.target_revisions) !== JSON.stringify([options.targetRevisionId])
    || flight.author_visible_decision_ref.proposal_id !== options.proposalId
    || flight.author_visible_decision_ref.operation_id !== options.operationId
    || flight.author_visible_decision_ref.revision_id !== options.proposalRevisionId
    || JSON.stringify(flight.frozen_request_digest)
      !== JSON.stringify(await digestRejectProposalOperations(flight.request, options.cryptoImpl))) {
    throw new Error("A prior Rejection decision is unresolved");
  }
  if (flight.problem !== undefined) throw new Error(`Rejection response requires review (HTTP ${flight.problem.status})`);
  if (flight.nonce === undefined) {
    const challenge = await createProjectCommandChallenge({
      baseUrl: options.baseUrl,
      projectId: workspace.partition.project_scope.project_id,
      fetchImpl: options.fetchImpl,
      request: { method: "POST",
        route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
        command_schema: flight.request.command_schema,
        canonical_command_digest: flight.frozen_request_digest,
        idempotency_key: flight.idempotencyKey },
    });
    flight.nonce = challenge.nonce;
    flight.challengeExpiresAt = challenge.expires_at;
    await writeFlight(workspace.database, flight);
  }
  const frozen = flight;
  const submit = async () => {
    const attempt = await beginAcceptanceAttempt(workspace, frozen);
    let response: RejectProposalOperationsResponse;
    try {
      response = await rejectProposalOperations({ baseUrl: options.baseUrl,
        projectId: workspace.partition.project_scope.project_id,
        proposalId: frozen.proposalId, fetchImpl: options.fetchImpl,
        idempotencyKey: frozen.idempotencyKey, antiForgery: frozen.nonce ?? "",
        request: frozen.request });
    } catch (error) {
      if (error instanceof StoryOSProtocolError && error.code === "command_http_error") {
        const terminal = attempt.ordinal === 1
          ? parsePreAdmissionAcceptanceProblem(error.status ?? 0, error.responseBody ?? "")
          : undefined;
        if (terminal !== undefined) await writeFlight(workspace.database, frozen,
          { kind: "pre_admission_problem", problem: terminal });
        else await writeFlight(workspace.database, { ...frozen, settlement: "known_problem",
          problem: { status: error.status ?? 0, code: error.code,
            responseBody: error.responseBody ?? "" } });
        await finishAcceptanceAttempt(workspace.database, attempt.id,
          { kind: "response_observed" });
        throw error;
      }
      await finishAcceptanceAttempt(workspace.database, attempt.id,
        { kind: "delivery_unknown", evidence: error instanceof StoryOSProtocolError
          && error.code === "command_invalid_json" ? "response_unreadable" : "connection_lost" });
      throw error;
    }
    if (response.schema_id !== "storyos.command.reject-proposal-operations.response.v1"
      || typeof response.command_id !== "string" || response.command_id.length === 0
      || typeof response.author_command_admission_id !== "string"
      || response.author_command_admission_id.length === 0
      || typeof response.receipt.receipt_id !== "string"
      || response.receipt.receipt_id.length === 0
      || response.correlation_id !== input.correlation_id
      || response.project_scope.owner_user_id !== frozen.project_scope.owner_user_id
      || response.project_scope.project_id !== frozen.project_scope.project_id
      || response.receipt.project_scope.owner_user_id !== frozen.project_scope.owner_user_id
      || response.receipt.project_scope.project_id !== frozen.project_scope.project_id
      || response.author_command_admission_id !== response.receipt.author_command_admission_id
      || response.receipt.result !== response.effect.kind
      || response.receipt.proposal_id !== frozen.proposalId
      || response.receipt.proposal_revision_id !== input.proposal_revision_id
      || JSON.stringify(response.receipt.selected_pending_operation_ids)
        !== JSON.stringify(input.selected_operation_ids)
      || JSON.stringify(response.receipt.expected_target_revisions)
        !== JSON.stringify(input.target_revisions)
      || response.receipt.idempotency_key !== frozen.idempotencyKey
      || JSON.stringify(response.receipt.command_digest)
        !== JSON.stringify(frozen.frozen_request_digest)) {
      await finishAcceptanceAttempt(workspace.database, attempt.id,
        { kind: "delivery_unknown", evidence: "response_unreadable" });
      throw new Error("Rejection acknowledgement does not match the frozen command");
    }
    await writeFlight(workspace.database, frozen, { kind: "settled", response });
    await finishAcceptanceAttempt(workspace.database, attempt.id,
      { kind: "response_observed" });
    return response;
  };
  try { return await submit(); }
  catch (error) {
    if (!(error instanceof TypeError)
      && !(error instanceof StoryOSProtocolError && error.code === "command_invalid_json")) {
      throw error;
    }
    try { return await submit(); }
    catch (retryError) {
      if (retryError instanceof TypeError
        || (retryError instanceof StoryOSProtocolError
          && retryError.code === "command_invalid_json")) {
        await writeFlight(workspace.database, { ...frozen, settlement: "delivery_unknown" });
      }
      throw retryError;
    }
  }
}

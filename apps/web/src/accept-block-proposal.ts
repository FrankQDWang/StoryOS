import { acceptProposal, createProjectCommandChallenge, digestAcceptProposal, getProposal, StoryOSProtocolError } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, AcceptProposalResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { historicalAcknowledgementUnavailable } from "./historical-acknowledgement.ts";
import type { EditorReadyState } from "./editor-types.ts";
import { beginAcceptanceAttempt, finishAcceptanceAttempt } from "./acceptance-transport.ts";
import { createFlight, readAcceptanceJournal, readFlight, uuidV7, writeFlight,
  type AcceptanceFlight, type AcceptanceRefusal } from "./acceptance-journal.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

class AcceptanceDeliveryUnknown extends Error {
  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : "Acceptance delivery is unknown");
  }
}

export async function retryPendingDisplayedAcceptance(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  workspace: EditorReadyState;
  proposalId: string;
}): Promise<AcceptProposalResponse> {
  const prefix = `acceptance:${options.workspace.partition.journal_partition_id}:${options.proposalId}:`;
  const request = options.workspace.database.transaction("metadata", "readonly")
    .objectStore("metadata").getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`));
  const flights = await new Promise<AcceptanceFlight[]>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result as AcceptanceFlight[]);
    request.onerror = () => reject(request.error ?? new Error("Acceptance record read failed"));
  });
  if (flights.length !== 1) throw new Error("Acceptance recovery is not available");
  const flight = flights[0]!;
  return acceptDisplayedBlockProposal({ ...options,
    operationId: flight.author_visible_decision_ref.operation_id,
    proposalRevisionId: flight.request.accept_proposal_input.proposal_revision_id,
    validationReceiptId: flight.request.accept_proposal_input.validation_receipt_id,
    authoritativeRevisionId: flight.request.accept_proposal_input.expected_authoritative_revision_id,
  });
}

export async function acceptDisplayedBlockProposal(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  workspace: EditorReadyState;
  proposalId: string;
  operationId: string;
  proposalRevisionId: string;
  validationReceiptId: string;
  authoritativeRevisionId: string;
}): Promise<AcceptProposalResponse> {
  const workspace = options.workspace;
  const projectId = workspace.partition.project_scope.project_id;
  const key = `acceptance:${workspace.partition.journal_partition_id}:${options.proposalId}:${options.proposalRevisionId}:${options.operationId}`;
  let flight = await readFlight(workspace.database, key);
  if (flight === undefined) {
    const request: AcceptProposalRequest = {
      command_schema: "storyos.command.accept-proposal.request.v1",
      accept_proposal_input: {
        proposal_revision_id: options.proposalRevisionId,
        validation_receipt_id: options.validationReceiptId,
        selected_operation_ids: [options.operationId],
        expected_authoritative_revision_id: options.authoritativeRevisionId,
        editor_session_id: workspace.partition.editor_session_id,
        client_contract_revision:
          RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: SECURITY_POLICY_REVISION,
        correlation_id: uuidV7(options.cryptoImpl),
      },
    };
    flight = await createFlight(workspace, {
      key,
      kind: "explicit_editor_command",
      explicit_command_record_id: uuidV7(options.cryptoImpl),
      journal_submission_group_id: uuidV7(options.cryptoImpl),
      journal_partition_id: workspace.partition.journal_partition_id,
      project_scope: workspace.partition.project_scope,
      editor_session_id: workspace.partition.editor_session_id,
      writer_generation: workspace.partition.writer_generation,
      command_kind: "acceptProposal",
      author_visible_decision_ref: {
        proposal_id: options.proposalId,
        operation_id: options.operationId,
        revision_id: options.proposalRevisionId,
      },
      frozen_request_digest: await digestAcceptProposal(request, options.cryptoImpl),
      settlement: "frozen",
      proposalId: options.proposalId,
      idempotencyKey: uuidV7(options.cryptoImpl),
      request,
    });
  }
  const input = flight.request?.accept_proposal_input;
  const journal = await readAcceptanceJournal(workspace);
  const group = journal.groups.find((item) => item.journal_submission_group_id
    === flight.journal_submission_group_id);
  const record = journal.records.find((item) => item.explicit_command_record_id
    === flight.explicit_command_record_id);
  if (flight.key !== key || flight.kind !== "explicit_editor_command"
    || group === undefined || record === undefined
    || (group.settlement as { kind?: string }).kind !== "unsettled"
    || group.idempotency_key !== flight.idempotencyKey
    || JSON.stringify(group.frozen_request_body) !== JSON.stringify(flight.request)
    || record.local_intent_sequence !== flight.local_intent_sequence
    || flight.journal_partition_id !== workspace.partition.journal_partition_id
    || flight.project_scope?.owner_user_id !== workspace.partition.project_scope.owner_user_id
    || flight.project_scope.project_id !== projectId
    || flight.editor_session_id !== workspace.partition.editor_session_id
    || flight.writer_generation !== workspace.partition.writer_generation
    || flight.command_kind !== "acceptProposal"
    || flight.proposalId !== options.proposalId
    || flight.request.command_schema !== "storyos.command.accept-proposal.request.v1"
    || input?.proposal_revision_id !== options.proposalRevisionId
    || input.validation_receipt_id !== options.validationReceiptId
    || input.selected_operation_ids.length !== 1
    || input.selected_operation_ids[0] !== options.operationId
    || input.expected_authoritative_revision_id !== options.authoritativeRevisionId
    || input.editor_session_id !== workspace.partition.editor_session_id
    || flight.author_visible_decision_ref.proposal_id !== options.proposalId
    || flight.author_visible_decision_ref.operation_id !== options.operationId
    || flight.author_visible_decision_ref.revision_id !== options.proposalRevisionId
    || JSON.stringify(flight.frozen_request_digest)
      !== JSON.stringify(await digestAcceptProposal(flight.request, options.cryptoImpl))) {
    throw new Error("A prior Acceptance decision is unresolved. Reload and inspect its result.");
  }
  if (flight.nonce === undefined) {
    const challenge = await createProjectCommandChallenge({
      baseUrl: options.baseUrl,
      projectId,
      fetchImpl: options.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
        command_schema: flight.request.command_schema,
        canonical_command_digest: flight.frozen_request_digest,
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
    flight.challengeExpiresAt = challenge.expires_at;
    await writeFlight(workspace.database, flight);
  }
  const frozen = flight;
  const submit = async () => {
    const attemptId = await beginAcceptanceAttempt(workspace, frozen);
    let accepted: AcceptProposalResponse;
    try {
      accepted = await acceptProposal({
        baseUrl: options.baseUrl,
        projectId,
        proposalId: frozen.proposalId,
        fetchImpl: options.fetchImpl,
        idempotencyKey: frozen.idempotencyKey,
        antiForgery: frozen.nonce ?? "",
        request: frozen.request,
      });
    } catch (error) {
      const observed = error instanceof StoryOSProtocolError;
      await finishAcceptanceAttempt(workspace.database, attemptId,
        observed ? "response_observed" : "delivery_unknown");
      throw observed ? error : new AcceptanceDeliveryUnknown(error);
    }
    await finishAcceptanceAttempt(workspace.database, attemptId, "response_observed");
    return accepted;
  };
  let response: AcceptProposalResponse;
  try {
    response = await submit();
  } catch (error) {
    if (historicalAcknowledgementUnavailable(error)) throw error;
    const refusal = await recordedRefusal(options, frozen, error);
    if (refusal !== undefined) {
      await writeFlight(workspace.database, frozen, { kind: "refused", refusal });
      throw error;
    }
    if (!(error instanceof AcceptanceDeliveryUnknown)) throw error;
    try {
      response = await submit();
    } catch (retryError) {
      const retryRefusal = await recordedRefusal(options, frozen, retryError);
      if (retryRefusal !== undefined) {
        await writeFlight(workspace.database, frozen, { kind: "refused", refusal: retryRefusal });
      } else if (retryError instanceof AcceptanceDeliveryUnknown) {
        try { await writeFlight(workspace.database, { ...frozen, settlement: "delivery_unknown" }); }
        catch { /* The frozen record remains available. */ }
      }
      throw retryError;
    }
  }
  if (response.schema_id !== "storyos.command.accept-proposal.response.v1"
    || typeof response.command_id !== "string" || response.command_id.length === 0
    || typeof response.author_command_admission_id !== "string"
    || response.author_command_admission_id.length === 0
    || typeof response.receipt.receipt_id !== "string"
    || response.receipt.receipt_id.length === 0
    || response.correlation_id !== frozen.request.accept_proposal_input.correlation_id
    || response.project_scope.owner_user_id !== frozen.project_scope.owner_user_id
    || response.project_scope.project_id !== frozen.project_scope.project_id
    || response.receipt.project_scope.owner_user_id !== frozen.project_scope.owner_user_id
    || response.receipt.project_scope.project_id !== frozen.project_scope.project_id
    || response.author_command_admission_id
      !== response.receipt.author_command_admission_id
    || response.receipt.result !== response.effect.kind
    || response.receipt.proposal_id !== frozen.proposalId
    || response.receipt.proposal_revision_id
      !== frozen.request.accept_proposal_input.proposal_revision_id
    || response.receipt.validation_receipt_id
      !== frozen.request.accept_proposal_input.validation_receipt_id
    || JSON.stringify(response.receipt.selected_operation_ids)
      !== JSON.stringify(frozen.request.accept_proposal_input.selected_operation_ids)
    || response.receipt.idempotency_key !== frozen.idempotencyKey
    || JSON.stringify(response.receipt.command_digest)
      !== JSON.stringify(frozen.frozen_request_digest)) {
    throw new Error("Acceptance acknowledgement does not match the frozen command");
  }
  try { await writeFlight(workspace.database, frozen, { kind: "settled", response }); }
  catch { /* Exact replay remains safe. */ }
  return response;
}

async function recordedRefusal(
  options: { baseUrl: string; fetchImpl: typeof fetch; workspace: EditorReadyState },
  flight: AcceptanceFlight,
  error: unknown,
): Promise<AcceptanceRefusal | undefined> {
  if (!(error instanceof StoryOSProtocolError)
    || (error.status !== 409 && error.status !== 422)) return undefined;
  try {
    const inspected = await getProposal({
      baseUrl: options.baseUrl,
      fetchImpl: options.fetchImpl,
      projectId: flight.project_scope.project_id,
      proposalId: flight.proposalId,
    });
    const refusal = inspected.proposal.latest_acceptance_refusal;
    return inspected.project_scope.owner_user_id === flight.project_scope.owner_user_id
      && inspected.project_scope.project_id === flight.project_scope.project_id
      && refusal.kind === "present"
      && refusal.correlation_id === flight.request.accept_proposal_input.correlation_id
      && refusal.command_schema === flight.request.command_schema
      && refusal.client_contract_revision
        === flight.request.accept_proposal_input.client_contract_revision
      && refusal.security_policy_revision
        === flight.request.accept_proposal_input.security_policy_revision
      ? refusal : undefined;
  } catch { return undefined; }
}

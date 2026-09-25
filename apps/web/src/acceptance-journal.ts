import { digestAcceptProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, AcceptProposalResponse, BlockProposalInspect, DigestValue, ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorReadyState, EditorWorkspace } from "./editor-types.ts";

export type AcceptanceFlight = {
  key: string;
  kind: "explicit_editor_command";
  explicit_command_record_id: string;
  journal_submission_group_id: string;
  local_intent_sequence: number;
  journal_partition_id: string;
  project_scope: ProjectScope;
  editor_session_id: string;
  writer_generation: string;
  command_kind: "acceptProposal";
  author_visible_decision_ref: { proposal_id: string; operation_id: string; revision_id: string };
  frozen_request_digest: DigestValue;
  settlement: "frozen" | "delivery_unknown" | "known_problem";
  problem?: { status: number; code: string; responseBody: string;
    retryAfterSeconds?: number };
  proposalId: string;
  idempotencyKey: string;
  nonce?: string;
  challengeExpiresAt?: string;
  request: AcceptProposalRequest;
};
export type AcceptanceRefusal = Extract<BlockProposalInspect["latest_acceptance_refusal"],
  { kind: "present" }>;
export type AcceptanceSettlement =
  | { kind: "settled"; response: AcceptProposalResponse }
  | { kind: "refused"; refusal: AcceptanceRefusal };

export async function readAcceptanceJournal(workspace: EditorWorkspace,
  snapshotTransaction?: IDBTransaction): Promise<{
  records: Record<string, unknown>[];
  groups: Record<string, unknown>[];
}> {
  const partitionId = workspace.partition.journal_partition_id;
  const transaction = snapshotTransaction ?? workspace.database.transaction(
    ["intents", "submission_groups", "transport_capsules", "transport_attempts"], "readonly");
  const intentsRequest = transaction.objectStore("intents").index("partition").getAll(partitionId);
  const groupsRequest = transaction.objectStore("submission_groups").index("partition")
    .getAll(partitionId);
  const capsulesRequest = transaction.objectStore("transport_capsules").getAll();
  const attemptsRequest = transaction.objectStore("transport_attempts").getAll();
  const read = (request: IDBRequest) => new Promise<Record<string, unknown>[]>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result as Record<string, unknown>[]);
    request.onerror = () => reject(request.error ?? new Error("Acceptance Journal read failed"));
  });
  const [allRecords, allGroups, allCapsules, allAttempts] = await Promise.all([
    read(intentsRequest), read(groupsRequest), read(capsulesRequest), read(attemptsRequest),
  ]);
  const records = allRecords.filter((record) => record.completed_intent_record_id === undefined);
  const explicitIds = new Set(records.map((record) => record.explicit_command_record_id));
  const groups = allGroups.filter((group) => group.action_class !== "direct_editor_action"
    || explicitIds.has((group.ordered_coverage as { intent_record_ref?: string }[] | undefined)?.[
      0]?.intent_record_ref));
  if (records.length !== groups.length || records.length > 2400) {
    throw new Error("Acceptance Journal is corrupt");
  }
  for (const group of groups) {
    const coverage = group.ordered_coverage as { local_intent_sequence?: number;
      intent_record_ref?: string; payload_digest?: DigestValue }[] | undefined;
    const record = records.find((item) => item.explicit_command_record_id
      === coverage?.[0]?.intent_record_ref);
    const request = group.frozen_request_body as AcceptProposalRequest | undefined;
    const digest = request === undefined ? undefined
      : await digestAcceptProposal(request, workspace.cryptoImpl);
    const input = request?.accept_proposal_input;
    const coverageBytes = new TextEncoder().encode(JSON.stringify({
      ordered_coverage: coverage, covered_sequence_range: group.covered_sequence_range,
    }));
    const coverageHash = new Uint8Array(await workspace.cryptoImpl.subtle.digest(
      "SHA-256", coverageBytes));
    const coverageDigest: DigestValue = { algorithm: "sha256",
      profile: "storyos.local-edit-journal.submission-coverage.sha256.v1",
      value_hex_lowercase: [...coverageHash]
        .map((byte) => byte.toString(16).padStart(2, "0")).join("") };
    const capsule = allCapsules.filter((item) => item.journal_submission_group_id
      === group.journal_submission_group_id);
    const attempts = allAttempts.filter((item) => item.journal_submission_group_id
      === group.journal_submission_group_id).sort((left, right) =>
        (left.attempt_ordinal as number) - (right.attempt_ordinal as number));
    const settlement = group.settlement as AcceptanceSettlement | { kind: "unsettled" };
    if (record === undefined || coverage?.length !== 1
      || record.journal_partition_id !== partitionId
      || record.command_kind !== "acceptProposal"
      || group.journal_partition_id !== partitionId
      || group.command_kind !== "acceptProposal"
      || JSON.stringify(record.project_scope) !== JSON.stringify(workspace.partition.project_scope)
      || JSON.stringify(group.project_scope) !== JSON.stringify(workspace.partition.project_scope)
      || record.editor_session_id !== workspace.partition.editor_session_id
      || group.editor_session_id !== workspace.partition.editor_session_id
      || record.writer_generation !== workspace.partition.writer_generation
      || group.writer_generation !== workspace.partition.writer_generation
      || group.action_class !== "explicit_editor_command"
      || group.batch_policy_revision !== "storyos.explicit-command-batch.release-1.v1"
      || group.command_schema !== "storyos.command.accept-proposal.request.v1"
      || group.method !== "POST"
      || group.route_template !== "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances"
      || !Number.isSafeInteger(record.local_intent_sequence)
      || record.local_intent_sequence !== coverage[0]?.local_intent_sequence
      || record.editor_contract_revision !== "storyos.editor-contract.release-1.v2"
      || record.exact_semantic_payload_ref !== group.journal_submission_group_id
      || group.frozen_request_body_ref !== group.journal_submission_group_id
      || JSON.stringify(record.author_visible_decision_ref) !== JSON.stringify({
        proposal_id: group.proposal_id,
        operation_id: input?.selected_operation_ids?.[0],
        revision_id: input?.proposal_revision_id,
      })
      || input?.selected_operation_ids?.length !== 1
      || record.proposal_id !== group.proposal_id
      || JSON.stringify(record.exact_target_head_anchor_bindings) !== JSON.stringify({
        proposal_revision_id: input?.proposal_revision_id,
        validation_receipt_id: input?.validation_receipt_id,
        authoritative_revision_id: input?.expected_authoritative_revision_id,
      })
      || JSON.stringify(group.covered_sequence_range) !== JSON.stringify({
        first: record.local_intent_sequence, last: record.local_intent_sequence,
      })
      || JSON.stringify(record.semantic_payload_digest) !== JSON.stringify(digest)
      || JSON.stringify(group.frozen_request_digest) !== JSON.stringify(digest)
      || JSON.stringify(coverage[0]?.payload_digest) !== JSON.stringify(digest)
      || JSON.stringify(group.frozen_payload_coverage_digest)
        !== JSON.stringify(coverageDigest)
      || !["unsettled", "settled", "refused"].includes(settlement?.kind ?? "")
      || capsule.length > 1 || (attempts.length > 0 && capsule.length !== 1)
      || attempts.some((attempt, index) => attempt.attempt_ordinal !== index + 1
        || attempt.exact_transport_retry_capsule_id
          !== capsule[0]?.exact_transport_retry_capsule_id)
      || (capsule.length === 1
        && (capsule[0]?.exact_client_controlled_headers as Record<string, string>)?.[
          "Idempotency-Key"] !== group.idempotency_key)
      || (settlement.kind === "settled"
        && (settlement.response.receipt.idempotency_key !== group.idempotency_key
          || settlement.response.receipt.proposal_id !== group.proposal_id
          || JSON.stringify(settlement.response.project_scope)
            !== JSON.stringify(workspace.partition.project_scope)
          || JSON.stringify(settlement.response.receipt.project_scope)
            !== JSON.stringify(workspace.partition.project_scope)
          || settlement.response.receipt.proposal_revision_id !== input?.proposal_revision_id
          || settlement.response.receipt.validation_receipt_id !== input?.validation_receipt_id
          || JSON.stringify(settlement.response.receipt.selected_operation_ids)
            !== JSON.stringify(input?.selected_operation_ids)
          || settlement.response.receipt.result !== settlement.response.effect.kind
          || JSON.stringify(settlement.response.receipt.command_digest) !== JSON.stringify(digest)
          || settlement.response.correlation_id
            !== request?.accept_proposal_input.correlation_id))
      || (settlement.kind === "refused"
        && settlement.refusal.correlation_id
          !== request?.accept_proposal_input.correlation_id)) {
      throw new Error("Acceptance Journal is corrupt");
    }
  }
  return { records, groups };
}

export async function settledDisplayedAcceptance(workspace: EditorWorkspace,
  proposalId: string): Promise<AcceptanceSettlement | undefined> {
  const journal = await readAcceptanceJournal(workspace);
  const records = journal.records.filter((record) =>
    (record.author_visible_decision_ref as { proposal_id?: string })?.proposal_id === proposalId);
  const latest = records.sort((left, right) =>
    (right.local_intent_sequence as number) - (left.local_intent_sequence as number))[0];
  const group = journal.groups.find((item) =>
    (item.ordered_coverage as { intent_record_ref: string }[])?.[0]?.intent_record_ref
      === latest?.explicit_command_record_id);
  const settlement = group?.settlement as AcceptanceSettlement | undefined;
  return settlement?.kind === "settled" || settlement?.kind === "refused"
    ? settlement : undefined;
}

export async function readFlight(database: IDBDatabase, key: string): Promise<AcceptanceFlight | undefined> {
  const request = database.transaction("metadata", "readonly").objectStore("metadata").get(key);
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve((request.result as AcceptanceFlight | undefined));
    request.onerror = () => reject(request.error ?? new Error("Acceptance record read failed"));
  });
}

export async function hasPendingDisplayedAcceptance(workspace: EditorReadyState,
  proposalId: string): Promise<boolean> {
  const prefix = `acceptance:${workspace.partition.journal_partition_id}:${proposalId}:`;
  const request = workspace.database.transaction("metadata", "readonly").objectStore("metadata")
    .getAllKeys(IDBKeyRange.bound(prefix, `${prefix}\uffff`));
  return new Promise<boolean>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result.length > 0);
    request.onerror = () => reject(request.error ?? new Error("Acceptance record read failed"));
  });
}

export async function reconcileDisplayedAcceptance(workspace: EditorReadyState,
  proposal: BlockProposalInspect): Promise<"none" | "pending" | "applied" | "refused"> {
  const prefix = `acceptance:${workspace.partition.journal_partition_id}:${proposal.proposal_id}:`;
  const request = workspace.database.transaction("metadata", "readonly").objectStore("metadata")
    .getAll(IDBKeyRange.bound(prefix, `${prefix}\uffff`));
  const flights = await new Promise<AcceptanceFlight[]>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result as AcceptanceFlight[]);
    request.onerror = () => reject(request.error ?? new Error("Acceptance record read failed"));
  });
  const flight = flights[0];
  if (flight === undefined) return "none";
  if (flight.project_scope.owner_user_id !== workspace.partition.project_scope.owner_user_id
    || flight.project_scope.project_id !== workspace.partition.project_scope.project_id
    || flight.editor_session_id !== workspace.partition.editor_session_id
    || flight.writer_generation !== workspace.partition.writer_generation) return "pending";
  if (proposal.revision_id === flight.request.accept_proposal_input.proposal_revision_id
    && proposal.operation_id === flight.author_visible_decision_ref.operation_id
    && proposal.operation_resolution === "applied") return "applied";
  if (proposal.latest_acceptance_refusal.kind === "present"
    && proposal.latest_acceptance_refusal.correlation_id
      === flight.request.accept_proposal_input.correlation_id) {
    await writeFlight(workspace.database, flight,
      { kind: "refused", refusal: proposal.latest_acceptance_refusal });
    return "refused";
  }
  return "pending";
}

export async function writeFlight(database: IDBDatabase, flight: AcceptanceFlight,
  settlement?: AcceptanceSettlement): Promise<void> {
  const transaction = database.transaction(["metadata", "submission_groups"], "readwrite",
    { durability: "strict" });
  const metadata = transaction.objectStore("metadata");
  const groups = transaction.objectStore("submission_groups");
  const groupRequest = groups.get(flight.journal_submission_group_id);
  groupRequest.onsuccess = () => {
    const group = groupRequest.result as Record<string, unknown> | undefined;
    if (group === undefined) { transaction.abort(); return; }
    if (settlement === undefined) {
      metadata.put(flight);
      groups.put({ ...group, acceptance_delivery: flight.problem === undefined
        ? flight.settlement : { kind: "known_problem", ...flight.problem } });
    } else {
      metadata.delete(flight.key);
      groups.put({ ...group, settlement });
    }
  };
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Acceptance record write failed"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Acceptance record write failed"));
  });
}

export async function createFlight(workspace: EditorReadyState,
  flight: Omit<AcceptanceFlight, "local_intent_sequence">): Promise<AcceptanceFlight> {
  const database = workspace.database;
  const requestDigest = await digestAcceptProposal(flight.request, workspace.cryptoImpl);
  if (JSON.stringify(requestDigest) !== JSON.stringify(flight.frozen_request_digest)
    || flight.key !== `acceptance:${flight.journal_partition_id}:${flight.proposalId}:${flight.author_visible_decision_ref.revision_id}:${flight.author_visible_decision_ref.operation_id}`
    || flight.request.command_schema !== "storyos.command.accept-proposal.request.v1"
    || flight.request.accept_proposal_input.selected_operation_ids.length !== 1
    || flight.author_visible_decision_ref.proposal_id !== flight.proposalId
    || flight.author_visible_decision_ref.operation_id
      !== flight.request.accept_proposal_input.selected_operation_ids[0]
    || flight.author_visible_decision_ref.revision_id
      !== flight.request.accept_proposal_input.proposal_revision_id) {
    throw new Error("Acceptance decision does not match the frozen command");
  }
  for (let retry = 0; retry < 3; retry += 1) {
    const read = database.transaction("metadata", "readonly").objectStore("metadata")
      .get("local_intent_sequence");
    const previous = await new Promise<{ value?: number } | undefined>((resolve, reject) => {
      read.onsuccess = () => resolve(read.result as { value?: number } | undefined);
      read.onerror = () => reject(read.error ?? new Error("Journal sequence read failed"));
    });
    const priorSequence = previous?.value ?? 0;
    const sequence = priorSequence + 1;
    if (!Number.isSafeInteger(sequence)) throw new Error("Local Edit Journal sequence is invalid");
    const coverage = { ordered_coverage: [{ local_intent_sequence: sequence,
      intent_record_ref: flight.explicit_command_record_id,
      payload_digest: flight.frozen_request_digest }],
    covered_sequence_range: { first: sequence, last: sequence } };
    const bytes = new TextEncoder().encode(JSON.stringify(coverage));
    const hash = new Uint8Array(await workspace.cryptoImpl.subtle.digest("SHA-256", bytes));
    const coverageDigest: DigestValue = { algorithm: "sha256",
      profile: "storyos.local-edit-journal.submission-coverage.sha256.v1",
      value_hex_lowercase: [...hash].map((byte) => byte.toString(16).padStart(2, "0")).join("") };
    try { return await commitFlight(workspace, flight, sequence, priorSequence, coverageDigest); }
    catch (error) {
      if (!(error instanceof Error) || error.message !== "Journal sequence changed") throw error;
    }
  }
  throw new Error("Journal sequence changed");
}

async function commitFlight(workspace: EditorReadyState,
  flight: Omit<AcceptanceFlight, "local_intent_sequence">,
  sequence: number, priorSequence: number, coverageDigest: DigestValue): Promise<AcceptanceFlight> {
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "intents", "submission_groups"],
    "readwrite", { durability: "strict" });
  const metadata = transaction.objectStore("metadata");
  const sequenceRequest = metadata.get("local_intent_sequence");
  const schemaRequest = metadata.get("schema");
  const prefix = `acceptance:${flight.journal_partition_id}:${flight.proposalId}:`;
  const pendingRequest = metadata.getAllKeys(IDBKeyRange.bound(prefix, `${prefix}\uffff`));
  const partitionRequest = transaction.objectStore("partitions")
    .get(workspace.partition.journal_partition_id);
  const previous = await new Promise<{ value?: number } | undefined>((resolve, reject) => {
    sequenceRequest.onsuccess = () => resolve(sequenceRequest.result as { value?: number } | undefined);
    sequenceRequest.onerror = () => reject(sequenceRequest.error ?? new Error("Journal sequence read failed"));
  });
  const [schema, partition, pendingKeys] = await Promise.all([
    new Promise<{ version?: number } | undefined>((resolve, reject) => {
      schemaRequest.onsuccess = () => resolve(schemaRequest.result as { version?: number } | undefined);
      schemaRequest.onerror = () => reject(schemaRequest.error ?? new Error("Journal schema read failed"));
    }),
    new Promise<unknown>((resolve, reject) => {
      partitionRequest.onsuccess = () => resolve(partitionRequest.result);
      partitionRequest.onerror = () => reject(partitionRequest.error
        ?? new Error("Journal partition read failed"));
    }),
    new Promise<IDBValidKey[]>((resolve, reject) => {
      pendingRequest.onsuccess = () => resolve(pendingRequest.result);
      pendingRequest.onerror = () => reject(pendingRequest.error
        ?? new Error("Pending Acceptance read failed"));
    }),
  ]);
  if ((previous?.value ?? 0) !== priorSequence) {
    transaction.abort();
    throw new Error("Journal sequence changed");
  }
  if (schema?.version !== 4
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || workspace.partition.disposition !== "current_writer_open"
    || flight.journal_partition_id !== workspace.partition.journal_partition_id
    || JSON.stringify(flight.project_scope) !== JSON.stringify(workspace.partition.project_scope)
    || flight.editor_session_id !== workspace.partition.editor_session_id
    || flight.writer_generation !== workspace.partition.writer_generation
    || flight.request.accept_proposal_input.editor_session_id
      !== workspace.partition.editor_session_id
    || flight.request.accept_proposal_input.client_contract_revision
      !== workspace.partition.client_contract_revision
    || flight.request.accept_proposal_input.security_policy_revision
      !== workspace.partition.security_policy_revision) {
    transaction.abort();
    throw new Error("Acceptance Journal partition changed");
  }
  if (pendingKeys.length > 0) {
    transaction.abort();
    throw new Error("A prior Acceptance decision is unresolved");
  }
  const created = { ...flight, local_intent_sequence: sequence };
  const createdAt = new Date().toISOString();
  const record = {
    explicit_command_record_id: flight.explicit_command_record_id,
    local_intent_sequence: sequence,
    journal_partition_id: flight.journal_partition_id,
    project_scope: flight.project_scope,
    proposal_id: flight.proposalId,
    editor_session_id: flight.editor_session_id,
    writer_generation: flight.writer_generation,
    command_kind: flight.command_kind,
    exact_semantic_payload_ref: flight.journal_submission_group_id,
    semantic_payload_digest: flight.frozen_request_digest,
    exact_target_head_anchor_bindings: {
      proposal_revision_id: flight.request.accept_proposal_input.proposal_revision_id,
      validation_receipt_id: flight.request.accept_proposal_input.validation_receipt_id,
      authoritative_revision_id: flight.request.accept_proposal_input.expected_authoritative_revision_id,
    },
    editor_contract_revision: "storyos.editor-contract.release-1.v2",
    author_visible_decision_ref: flight.author_visible_decision_ref,
    created_at: createdAt,
  };
  const group = {
    journal_submission_group_id: flight.journal_submission_group_id,
    journal_partition_id: flight.journal_partition_id,
    project_scope: flight.project_scope,
    proposal_id: flight.proposalId,
    editor_session_id: flight.editor_session_id,
    writer_generation: flight.writer_generation,
    ordered_coverage: [{ local_intent_sequence: sequence,
      intent_record_ref: flight.explicit_command_record_id,
      payload_digest: flight.frozen_request_digest }],
    covered_sequence_range: { first: sequence, last: sequence },
    action_class: "explicit_editor_command",
    batch_policy_revision: "storyos.explicit-command-batch.release-1.v1",
    api_major: 1,
    method: "POST",
    route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
    command_schema: flight.request.command_schema,
    command_kind: flight.command_kind,
    digest_profile: "storyos.command.acceptProposal.jcs.v1",
    idempotency_key: flight.idempotencyKey,
    frozen_request_body_ref: flight.journal_submission_group_id,
    frozen_request_body: flight.request,
    frozen_request_digest_input_ref: flight.journal_submission_group_id,
    frozen_request_digest: flight.frozen_request_digest,
    frozen_payload_coverage_digest: coverageDigest,
    settlement: { kind: "unsettled" },
    acceptance_delivery: "frozen",
    frozen_at: createdAt,
  };
  metadata.put({ key: "local_intent_sequence", value: sequence });
  metadata.add(created);
  transaction.objectStore("intents").add(record);
  transaction.objectStore("submission_groups").add(group);
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Acceptance record write failed"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Acceptance record write failed"));
  });
  return created;
}

export function uuidV7(cryptoImpl: Crypto, now = Date.now()): string {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

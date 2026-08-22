import { getApplyAuthorEditOutcome }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorBaseSnapshot }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  EditorWorkspace,
  JournalPartition,
  JournalSubmissionGroup,
  OutcomeQueryAttempt,
  OutcomeQueryReconciliation,
  ReconciliationEvidence,
  SubmissionSettlement,
  TransportCapsule,
} from "./editor-types.ts";
import { createJournalUuid, JOURNAL_DATABASE_VERSION } from "./local-edit-journal.ts";
import {
  beginOutcomeQueryAttempt,
  completeOutcomeQueryUnavailable,
  readAvailableCommandProof,
} from "./protected-transport-capsule.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

interface GroupEvidenceView {
  settlement?: SubmissionSettlement;
  reconciliation?: OutcomeQueryReconciliation;
}

type ClosedOutcome =
  | { kind: "committed"; response: unknown }
  | { kind: "rejected"; reason: "challenge_expired_unconsumed" }
  | { kind: "still_unknown"; strongest: Exclude<ReconciliationEvidence,
      { kind: "no_outcome_observed" }> };

type ClosedStillUnknown = Extract<ClosedOutcome, { kind: "still_unknown" }>;

interface ValidatedOutcomeEnvelope {
  correlation_id: string;
  project_scope: unknown;
}

export interface DurableOutcomeQuery {
  group: JournalSubmissionGroup;
  attempt: OutcomeQueryAttempt;
  capsule: TransportCapsule;
  payload: ValidatedOutcomeEnvelope;
  captured: string;
  closed: ClosedOutcome;
}

interface SettleFromCommandResponseOptions {
  workspace: EditorWorkspace;
  group: JournalSubmissionGroup;
  response: unknown;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  durableQuery: DurableOutcomeQuery;
}

const requestResult = (request: IDBRequest): Promise<unknown> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction: IDBTransaction): Promise<void> => new Promise(
  (resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(
      transaction.error ?? new Error("IndexedDB transaction aborted"),
    );
    transaction.onerror = () => reject(
      transaction.error ?? new Error("IndexedDB transaction failed"),
    );
  },
);

export function groupEvidenceRank(group: GroupEvidenceView | undefined): number {
  const settlement = group?.settlement?.kind;
  if (settlement === "applied_receipt_settled"
    || settlement === "zero_authority_receipt_settled"
    || settlement === "outcome_query_rejected_no_admission") {
    return 4;
  }
  const strongest = group?.reconciliation?.strongest?.kind;
  if (strongest === "admission_committed") return 3;
  if (strongest === "challenge_issued") return 2;
  if (group?.reconciliation?.kind === "outcome_query_unresolved") return 1;
  return 0;
}

export function sameTerminalIdentity(
  left: GroupEvidenceView | undefined,
  right: GroupEvidenceView | undefined,
): boolean {
  const leftSettlement = left?.settlement as Record<string, unknown> | undefined;
  const rightSettlement = right?.settlement as Record<string, unknown> | undefined;
  const leftReceipt = leftSettlement?.receipt as Record<string, unknown> | undefined;
  const rightReceipt = rightSettlement?.receipt as Record<string, unknown> | undefined;
  return leftSettlement?.kind === rightSettlement?.kind
    && leftSettlement?.command_id === rightSettlement?.command_id
    && leftSettlement?.author_command_admission_id
      === rightSettlement?.author_command_admission_id
    && leftReceipt?.receipt_id === rightReceipt?.receipt_id
    && leftReceipt?.result === rightReceipt?.result
    && leftSettlement?.reason === rightSettlement?.reason;
}

function decideStrongerGroup(
  durable: JournalSubmissionGroup,
  next: JournalSubmissionGroup,
): JournalSubmissionGroup {
  const nextRank = groupEvidenceRank(next);
  const durableRank = groupEvidenceRank(durable);
  if (nextRank < durableRank) {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  if (nextRank === durableRank) {
    if (JSON.stringify(durable) !== JSON.stringify(next)
      && !(nextRank === 4 && sameTerminalIdentity(durable, next))) {
      throw new Error("Author Edit acknowledgement does not converge");
    }
    return durable;
  }
  return next;
}

function putStrongerGroup(
  transaction: IDBTransaction,
  durable: JournalSubmissionGroup,
  next: JournalSubmissionGroup,
  activeBase: EditorBaseSnapshot | undefined,
  partitionId: string,
): JournalSubmissionGroup {
  let committed: JournalSubmissionGroup;
  try {
    committed = decideStrongerGroup(durable, next);
  } catch (error) {
    transaction.abort();
    throw error;
  }
  if (committed !== durable) {
    transaction.objectStore("submission_groups").put(next);
    if (activeBase !== undefined) {
      transaction.objectStore("metadata").put({
        key: `active_base:${partitionId}`,
        value: activeBase,
      });
    }
  }
  return committed;
}

function takeoverFencePartitionConverges(
  durablePartition: unknown,
  nextPartition: JournalPartition,
): boolean {
  if (JSON.stringify(durablePartition) === JSON.stringify(nextPartition)) return true;
  if (!durablePartition || !nextPartition) return false;
  const { disposition: durableDisposition, ...durableRest } =
    durablePartition as Record<string, unknown>;
  const { disposition: nextDisposition, ...nextRest } = nextPartition;
  // Takeover may fence the still-open partition in the same settlement write.
  // Generation, session binding, and journal identity must stay unchanged.
  return durableDisposition === "current_writer_open"
    && nextDisposition === "read_only_observer"
    && JSON.stringify(durableRest) === JSON.stringify(nextRest);
}

function putTakeoverFencePartition(
  transaction: IDBTransaction,
  durablePartition: unknown,
  nextPartition: JournalPartition,
): void {
  if (JSON.stringify(durablePartition) === JSON.stringify(nextPartition)) return;
  transaction.objectStore("partitions").put(nextPartition);
}

function assertMatchingDurableGroup(
  durable: unknown,
  next: JournalSubmissionGroup,
  transaction: IDBTransaction,
): asserts durable is JournalSubmissionGroup {
  const stored = durable as JournalSubmissionGroup | undefined;
  if (!stored
    || stored.idempotency_key !== next.idempotency_key
    || JSON.stringify(stored.frozen_request_digest)
      !== JSON.stringify(next.frozen_request_digest)
    || JSON.stringify(stored.project_scope) !== JSON.stringify(next.project_scope)) {
    transaction.abort();
    throw new Error("Journal Submission Group is corrupt");
  }
}

export async function commitStrongerGroup(
  workspace: EditorWorkspace,
  next: JournalSubmissionGroup,
  activeBase?: EditorBaseSnapshot,
): Promise<JournalSubmissionGroup> {
  const stores = [
    ...(activeBase === undefined ? [] : ["metadata"]),
    ...(workspace.partition.disposition === "read_only_observer" ? ["partitions"] : []),
    "submission_groups",
  ];
  const transaction = workspace.database.transaction(stores, "readwrite");
  const groups = transaction.objectStore("submission_groups");
  const durable = await requestResult(groups.get(next.journal_submission_group_id));
  assertMatchingDurableGroup(durable, next, transaction);
  if (workspace.partition.disposition === "read_only_observer") {
    const durablePartition = await requestResult(
      transaction.objectStore("partitions").get(workspace.partition.journal_partition_id),
    );
    if (!takeoverFencePartitionConverges(durablePartition, workspace.partition)) {
      transaction.abort();
      throw new Error("Author Edit acknowledgement does not converge");
    }
    putTakeoverFencePartition(transaction, durablePartition, workspace.partition);
  }
  const committed = putStrongerGroup(
    transaction, durable, next, activeBase, workspace.partition.journal_partition_id,
  );
  await transactionResult(transaction);
  return committed;
}

export async function commitOutcomeQueryWithGroup(
  workspace: EditorWorkspace,
  {
    group, attempt, capsule, payload, captured, closed, next, activeBase,
  }: DurableOutcomeQuery & {
    next: JournalSubmissionGroup;
    activeBase?: EditorBaseSnapshot;
  },
): Promise<JournalSubmissionGroup> {
  const observationId = createJournalUuid(workspace.cryptoImpl);
  const digest = new Uint8Array(await workspace.cryptoImpl.subtle.digest(
    "SHA-256", new TextEncoder().encode(captured),
  ));
  const observation = {
    outcome_query_observation_id: observationId,
    outcome_query_attempt_id: attempt.outcome_query_attempt_id,
    journal_submission_group_id: group.journal_submission_group_id,
    exact_transport_retry_capsule_id: capsule.exact_transport_retry_capsule_id,
    local_response_payload_ref: observationId,
    local_response_payload: captured,
    exact_response_payload_digest: {
      algorithm: "sha256",
      profile: "storyos.local-edit-journal.payload.sha256.v1",
      value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
    },
    schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
    query_correlation_id: payload.correlation_id,
    project_scope: payload.project_scope,
    observed_at: new Date().toISOString(),
    outcome: closed.kind === "committed"
      ? { kind: "committed", exact_apply_author_edit_response_v2: closed.response }
      : closed.kind === "rejected"
        ? { kind: "rejected_no_admission", reason: "challenge_expired_unconsumed" }
        : { kind: "still_unknown", observation: closed.strongest },
  };
  const transaction = workspace.database.transaction([
    "metadata",
    "partitions",
    "submission_groups",
    "transport_capsules",
    "outcome_query_attempts",
    "outcome_query_observations",
  ], "readwrite");
  const attempts = transaction.objectStore("outcome_query_attempts");
  const [schema, partition, durable, storedCapsule, stored] = await Promise.all([
    requestResult(transaction.objectStore("metadata").get("schema")),
    requestResult(transaction.objectStore("partitions")
      .get(workspace.partition.journal_partition_id)),
    requestResult(transaction.objectStore("submission_groups")
      .get(next.journal_submission_group_id)),
    requestResult(transaction.objectStore("transport_capsules")
      .get(capsule.exact_transport_retry_capsule_id)),
    requestResult(attempts.get(attempt.outcome_query_attempt_id)),
  ]);
  assertMatchingDurableGroup(durable, next, transaction);
  const schemaRecord = schema as { version?: unknown } | undefined;
  const storedAttempt = stored as OutcomeQueryAttempt | undefined;
  const availableCapsule = storedCapsule as TransportCapsule | undefined;
  if (schemaRecord?.version !== JOURNAL_DATABASE_VERSION
    || !takeoverFencePartitionConverges(partition, workspace.partition)
    || next.journal_submission_group_id !== group.journal_submission_group_id
    || storedAttempt?.outcome?.kind !== "in_flight"
    || storedAttempt.journal_submission_group_id !== group.journal_submission_group_id
    || storedAttempt.exact_transport_retry_capsule_id !== capsule.exact_transport_retry_capsule_id
    || availableCapsule?.disposition?.kind !== "available"
    || availableCapsule.exact_transport_retry_capsule_id
      !== capsule.exact_transport_retry_capsule_id
    || availableCapsule.journal_submission_group_id !== group.journal_submission_group_id) {
    transaction.abort();
    throw new Error("Author Edit acknowledgement does not converge");
  }
  putTakeoverFencePartition(transaction, partition, workspace.partition);
  const committed = putStrongerGroup(
    transaction, durable, next, activeBase, workspace.partition.journal_partition_id,
  );
  attempts.put({
    ...storedAttempt,
    outcome: { kind: "response_observed", outcome_query_observation_id: observationId },
  });
  transaction.objectStore("outcome_query_observations").add(observation);
  await transactionResult(transaction);
  return committed;
}

function closedOutcome(payload: unknown): ClosedOutcome | null {
  const candidate = payload as {
    schema_id?: unknown;
    correlation_id?: unknown;
    project_scope?: unknown;
    outcome?: {
      outcome_kind?: unknown;
      response?: { schema_id?: unknown };
      reason?: unknown;
      observation?: {
        observation_kind?: unknown;
        expires_at?: unknown;
        reconciliation_required?: unknown;
        command_id?: unknown;
        author_command_admission_id?: unknown;
      };
    };
  };
  if (candidate?.schema_id !== "storyos.query.apply-author-edit-outcome.response.v1"
    || !UUID.test((candidate?.correlation_id ?? "") as string)
    || !candidate?.outcome
    || typeof candidate.outcome !== "object") {
    return null;
  }
  const { outcome } = candidate;
  if (outcome.outcome_kind === "committed" && outcome.response?.schema_id
    === "storyos.command.apply-author-edit.response.v2") {
    return { kind: "committed", response: outcome.response };
  }
  if (outcome.outcome_kind === "rejected"
    && outcome.reason === "challenge_expired_unconsumed") {
    return { kind: "rejected", reason: "challenge_expired_unconsumed" };
  }
  if (outcome.outcome_kind === "still_unknown"
    && outcome.observation?.observation_kind === "challenge_issued"
    && typeof outcome.observation.expires_at === "string") {
    return {
      kind: "still_unknown",
      strongest: { kind: "challenge_issued", expires_at: outcome.observation.expires_at },
    };
  }
  if (outcome.outcome_kind === "still_unknown"
    && outcome.observation?.observation_kind === "admission_committed"
    && outcome.observation.reconciliation_required === true
    && UUID.test((outcome.observation.command_id ?? "") as string)
    && UUID.test((outcome.observation.author_command_admission_id ?? "") as string)) {
    return {
      kind: "still_unknown",
      strongest: {
        kind: "admission_committed",
        command_id: outcome.observation.command_id as string,
        author_command_admission_id:
          outcome.observation.author_command_admission_id as string,
        reconciliation_required: true,
      },
    };
  }
  return null;
}

export function reduceStrongest(
  prior: OutcomeQueryReconciliation | undefined,
  nextClosed?: ClosedStillUnknown,
): OutcomeQueryReconciliation {
  const candidate = {
    kind: "outcome_query_unresolved" as const,
    strongest: nextClosed?.strongest ?? { kind: "no_outcome_observed" as const },
  };
  const priorGroup = { reconciliation: prior } as GroupEvidenceView;
  const nextGroup = { reconciliation: candidate };
  if (groupEvidenceRank(nextGroup) < groupEvidenceRank(priorGroup)) return prior!;
  if (prior?.strongest?.kind === "challenge_issued"
    && candidate.strongest.kind === "challenge_issued"
    && prior.strongest.expires_at !== candidate.strongest.expires_at) {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  if (prior?.strongest?.kind === "admission_committed"
    && candidate.strongest.kind === "admission_committed"
    && (prior.strongest.command_id !== candidate.strongest.command_id
      || prior.strongest.author_command_admission_id
        !== candidate.strongest.author_command_admission_id)) {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  return candidate;
}

export async function markOutcomeQueryUnresolved(
  workspace: EditorWorkspace,
  group: JournalSubmissionGroup,
): Promise<JournalSubmissionGroup> {
  return commitStrongerGroup(workspace, {
    ...group,
    settlement: { kind: "unsettled" },
    reconciliation: {
      kind: "outcome_query_unresolved",
      strongest: { kind: "no_outcome_observed" },
    },
  });
}

export async function reconcileLostAcknowledgement({
  workspace, group, baseUrl, fetchImpl, cryptoImpl, settleFromCommandResponse,
}: {
  workspace: EditorWorkspace;
  group: JournalSubmissionGroup;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  settleFromCommandResponse: (
    options: SettleFromCommandResponseOptions,
  ) => Promise<void>;
}): Promise<JournalSubmissionGroup | void> {
  const proof = await readAvailableCommandProof(workspace, group);
  if (!proof) return group;
  const attempt = await beginOutcomeQueryAttempt(workspace, group, proof.capsule);
  let payload: unknown;
  let captured = "";
  try {
    payload = await getApplyAuthorEditOutcome({
      baseUrl,
      projectId: workspace.partition.project_scope.project_id,
      idempotencyKey: group.idempotency_key,
      antiForgery: proof.nonce,
      fetchImpl: async (url, options) => {
        const response = await fetchImpl(url, options);
        captured = await response.clone().text();
        return response;
      },
    });
  } catch {
    payload = null;
  }
  const closed = payload ? closedOutcome(payload) : null;
  if (!closed) {
    await completeOutcomeQueryUnavailable(
      workspace, group, attempt, payload ? "invalid_envelope" : "delivery_unknown",
    );
    return group;
  }
  const validatedPayload = payload as ValidatedOutcomeEnvelope;
  if (JSON.stringify(validatedPayload.project_scope) !== JSON.stringify(group.project_scope)
    || (closed.kind === "still_unknown"
      && closed.strongest.kind === "challenge_issued"
      && closed.strongest.expires_at !== proof.expiresAt)) {
    await completeOutcomeQueryUnavailable(workspace, group, attempt, "invalid_envelope");
    throw new Error("Author Edit acknowledgement does not converge");
  }
  const durableQuery = {
    group, attempt, capsule: proof.capsule, payload: validatedPayload, captured, closed,
  };
  if (closed.kind === "committed") {
    return settleFromCommandResponse({
      workspace, group, response: closed.response, baseUrl, fetchImpl, cryptoImpl,
      durableQuery,
    });
  }
  if (closed.kind === "rejected") {
    const rest = { ...group };
    delete rest.reconciliation;
    return commitOutcomeQueryWithGroup(workspace, {
      ...durableQuery,
      next: {
        ...rest,
        settlement: {
          kind: "outcome_query_rejected_no_admission",
          reason: "challenge_expired_unconsumed",
        },
      },
    });
  }
  return commitOutcomeQueryWithGroup(workspace, {
    ...durableQuery,
    next: {
      ...group,
      settlement: { kind: "unsettled" },
      reconciliation: reduceStrongest(group.reconciliation, closed),
    },
  });
}

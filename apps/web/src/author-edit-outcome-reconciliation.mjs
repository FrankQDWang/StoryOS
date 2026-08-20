import { getApplyAuthorEditOutcome }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createJournalUuid, JOURNAL_DATABASE_VERSION } from "./local-edit-journal.mjs";
import {
  beginOutcomeQueryAttempt,
  completeOutcomeQueryUnavailable,
  readAvailableCommandProof,
} from "./protected-transport-capsule.mjs";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction) => new Promise((resolve, reject) => {
  transaction.oncomplete = resolve;
  transaction.onabort = () => reject(
    transaction.error ?? new Error("IndexedDB transaction aborted"),
  );
  transaction.onerror = () => reject(
    transaction.error ?? new Error("IndexedDB transaction failed"),
  );
});

export function groupEvidenceRank(group) {
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

export function sameTerminalIdentity(left, right) {
  return left?.settlement?.kind === right?.settlement?.kind
    && left?.settlement?.command_id === right?.settlement?.command_id
    && left?.settlement?.author_command_admission_id
      === right?.settlement?.author_command_admission_id
    && left?.settlement?.receipt?.receipt_id === right?.settlement?.receipt?.receipt_id
    && left?.settlement?.receipt?.result === right?.settlement?.receipt?.result
    && left?.settlement?.reason === right?.settlement?.reason;
}

function decideStrongerGroup(durable, next) {
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

function putStrongerGroup(transaction, durable, next, activeBase, partitionId) {
  let committed;
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

function takeoverFencePartitionConverges(durablePartition, nextPartition) {
  if (JSON.stringify(durablePartition) === JSON.stringify(nextPartition)) return true;
  if (!durablePartition || !nextPartition) return false;
  const { disposition: durableDisposition, ...durableRest } = durablePartition;
  const { disposition: nextDisposition, ...nextRest } = nextPartition;
  // Takeover may fence the still-open partition in the same settlement write.
  // Generation, session binding, and journal identity must stay unchanged.
  return durableDisposition === "current_writer_open"
    && nextDisposition === "read_only_observer"
    && JSON.stringify(durableRest) === JSON.stringify(nextRest);
}

function putTakeoverFencePartition(transaction, durablePartition, nextPartition) {
  if (JSON.stringify(durablePartition) === JSON.stringify(nextPartition)) return;
  transaction.objectStore("partitions").put(nextPartition);
}

export async function commitStrongerGroup(workspace, next, activeBase) {
  const stores = [
    ...(activeBase === undefined ? [] : ["metadata"]),
    ...(workspace.partition.disposition === "read_only_observer" ? ["partitions"] : []),
    "submission_groups",
  ];
  const transaction = workspace.database.transaction(stores, "readwrite");
  const groups = transaction.objectStore("submission_groups");
  const durable = await requestResult(groups.get(next.journal_submission_group_id));
  if (!durable
    || durable.idempotency_key !== next.idempotency_key
    || JSON.stringify(durable.frozen_request_digest)
      !== JSON.stringify(next.frozen_request_digest)
    || JSON.stringify(durable.project_scope) !== JSON.stringify(next.project_scope)) {
    transaction.abort();
    throw new Error("Journal Submission Group is corrupt");
  }
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
  workspace, { group, attempt, capsule, payload, captured, closed, next, activeBase },
) {
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
  if (!durable
    || durable.idempotency_key !== next.idempotency_key
    || JSON.stringify(durable.frozen_request_digest)
      !== JSON.stringify(next.frozen_request_digest)
    || JSON.stringify(durable.project_scope) !== JSON.stringify(next.project_scope)) {
    transaction.abort();
    throw new Error("Journal Submission Group is corrupt");
  }
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || !takeoverFencePartitionConverges(partition, workspace.partition)
    || next.journal_submission_group_id !== group.journal_submission_group_id
    || stored?.outcome?.kind !== "in_flight"
    || stored.journal_submission_group_id !== group.journal_submission_group_id
    || stored.exact_transport_retry_capsule_id !== capsule.exact_transport_retry_capsule_id
    || storedCapsule?.disposition?.kind !== "available"
    || storedCapsule.exact_transport_retry_capsule_id
      !== capsule.exact_transport_retry_capsule_id
    || storedCapsule.journal_submission_group_id !== group.journal_submission_group_id) {
    transaction.abort();
    throw new Error("Author Edit acknowledgement does not converge");
  }
  putTakeoverFencePartition(transaction, partition, workspace.partition);
  const committed = putStrongerGroup(
    transaction, durable, next, activeBase, workspace.partition.journal_partition_id,
  );
  attempts.put({
    ...stored,
    outcome: { kind: "response_observed", outcome_query_observation_id: observationId },
  });
  transaction.objectStore("outcome_query_observations").add(observation);
  await transactionResult(transaction);
  return committed;
}

function closedOutcome(payload) {
  if (payload?.schema_id !== "storyos.query.apply-author-edit-outcome.response.v1"
    || !UUID.test(payload?.correlation_id ?? "")
    || !payload?.outcome
    || typeof payload.outcome !== "object") {
    return null;
  }
  const { outcome } = payload;
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
    && UUID.test(outcome.observation.command_id ?? "")
    && UUID.test(outcome.observation.author_command_admission_id ?? "")) {
    return {
      kind: "still_unknown",
      strongest: {
        kind: "admission_committed",
        command_id: outcome.observation.command_id,
        author_command_admission_id: outcome.observation.author_command_admission_id,
        reconciliation_required: true,
      },
    };
  }
  return null;
}

export function reduceStrongest(prior, nextClosed) {
  const candidate = {
    kind: "outcome_query_unresolved",
    strongest: nextClosed?.strongest ?? { kind: "no_outcome_observed" },
  };
  const priorGroup = { reconciliation: prior };
  const nextGroup = { reconciliation: candidate };
  if (groupEvidenceRank(nextGroup) < groupEvidenceRank(priorGroup)) return prior;
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

export async function markOutcomeQueryUnresolved(workspace, group) {
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
}) {
  const proof = await readAvailableCommandProof(workspace, group);
  if (!proof) return group;
  const attempt = await beginOutcomeQueryAttempt(workspace, group, proof.capsule);
  let payload;
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
  if (JSON.stringify(payload.project_scope) !== JSON.stringify(group.project_scope)
    || (closed.kind === "still_unknown"
      && closed.strongest.kind === "challenge_issued"
      && closed.strongest.expires_at !== proof.expiresAt)) {
    await completeOutcomeQueryUnavailable(workspace, group, attempt, "invalid_envelope");
    throw new Error("Author Edit acknowledgement does not converge");
  }
  const durableQuery = {
    group, attempt, capsule: proof.capsule, payload, captured, closed,
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

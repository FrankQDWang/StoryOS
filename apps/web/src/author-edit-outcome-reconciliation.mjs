import { getApplyAuthorEditOutcome }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  beginOutcomeQueryAttempt,
  completeOutcomeQueryObserved,
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

export async function commitStrongerGroup(workspace, next, activeBase) {
  const stores = activeBase === undefined
    ? ["submission_groups"]
    : ["metadata", "submission_groups"];
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
  const nextRank = groupEvidenceRank(next);
  const durableRank = groupEvidenceRank(durable);
  if (nextRank < durableRank) {
    transaction.abort();
    throw new Error("Author Edit acknowledgement does not converge");
  }
  if (nextRank === durableRank) {
    if (JSON.stringify(durable) !== JSON.stringify(next)
      && !(nextRank === 4 && sameTerminalIdentity(durable, next))) {
      transaction.abort();
      throw new Error("Author Edit acknowledgement does not converge");
    }
    await transactionResult(transaction);
    return durable;
  }
  groups.put(next);
  if (activeBase !== undefined) {
    transaction.objectStore("metadata").put({
      key: `active_base:${workspace.partition.journal_partition_id}`,
      value: activeBase,
    });
  }
  await transactionResult(transaction);
  return next;
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
  await completeOutcomeQueryObserved(workspace, group, attempt, proof.capsule, {
    payload, captured, closed,
  });
  if (closed.kind === "committed") {
    return settleFromCommandResponse({
      workspace, group, response: closed.response, baseUrl, fetchImpl, cryptoImpl,
    });
  }
  if (closed.kind === "rejected") {
    const rest = { ...group };
    delete rest.reconciliation;
    return commitStrongerGroup(workspace, {
      ...rest,
      settlement: {
        kind: "outcome_query_rejected_no_admission",
        reason: "challenge_expired_unconsumed",
      },
    });
  }
  return commitStrongerGroup(workspace, {
    ...group,
    settlement: { kind: "unsettled" },
    reconciliation: reduceStrongest(group.reconciliation, closed),
  });
}

import { createJournalUuid, JOURNAL_DATABASE_VERSION } from "./local-edit-journal.mjs";

const CAPSULE_STORE = "transport_capsules";
const ATTEMPT_STORE = "transport_attempts";
const QUERY_ATTEMPT_STORE = "outcome_query_attempts";
const QUERY_ROUTE =
  "/api/v1/projects/{project_id}/manuscript/author-edit-outcomes/{idempotency_key}";

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

function availableProof(capsule, workspace, group) {
  const headers = capsule?.exact_client_controlled_headers;
  if (capsule?.disposition?.kind !== "available"
    || capsule.journal_submission_group_id !== group.journal_submission_group_id
    || JSON.stringify(capsule.project_scope) !== JSON.stringify(group.project_scope)
    || capsule.client_session_binding_ref !== workspace.partition.client_session_binding_ref
    || capsule.client_session_generation !== workspace.partition.client_session_generation
    || capsule.request_identity?.api_major !== group.api_major
    || capsule.request_identity?.method !== group.method
    || capsule.request_identity?.route_template !== group.route_template
    || capsule.request_identity?.command_schema !== group.command_schema
    || capsule.request_identity?.command_kind !== group.command_kind
    || capsule.exact_request_body_ref !== group.journal_submission_group_id
    || JSON.stringify(capsule.canonical_command_digest)
      !== JSON.stringify(group.frozen_request_digest)
    || capsule.digest_profile !== group.digest_profile
    || headers?.["Idempotency-Key"] !== group.idempotency_key
    || typeof headers?.["X-StoryOS-Anti-Forgery"] !== "string"
    || headers["X-StoryOS-Anti-Forgery"].length === 0
    || headers?.["Content-Type"] !== "application/json"
    || typeof capsule.challenge_expires_at !== "string") {
    return null;
  }
  return {
    nonce: headers["X-StoryOS-Anti-Forgery"],
    expiresAt: capsule.challenge_expires_at,
    idempotencyKey: headers["Idempotency-Key"],
    capsule,
  };
}

export async function readAvailableCommandProof(workspace, group) {
  if (!workspace.database.objectStoreNames.contains(CAPSULE_STORE)) return null;
  const transaction = workspace.database.transaction(CAPSULE_STORE, "readonly");
  const capsules = await requestResult(
    transaction.objectStore(CAPSULE_STORE).index("group")
      .getAll(group.journal_submission_group_id),
  );
  const proofs = capsules.map((capsule) => availableProof(capsule, workspace, group))
    .filter((proof) => proof !== null);
  if (proofs.length !== 1) return null;
  return proofs[0];
}

export async function commitProtectedTransportSend(workspace, group, challenge) {
  if (typeof challenge?.nonce !== "string" || typeof challenge.expires_at !== "string") {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  const committedAt = new Date();
  const startedAt = new Date(committedAt.getTime() + 1);
  const capsule = {
    exact_transport_retry_capsule_id: createJournalUuid(workspace.cryptoImpl),
    journal_submission_group_id: group.journal_submission_group_id,
    project_scope: group.project_scope,
    client_session_binding_ref: workspace.partition.client_session_binding_ref,
    client_session_generation: workspace.partition.client_session_generation,
    request_identity: {
      api_major: group.api_major,
      method: group.method,
      route_template: group.route_template,
      command_schema: group.command_schema,
      command_kind: group.command_kind,
    },
    exact_request_body_ref: group.journal_submission_group_id,
    canonical_command_digest: group.frozen_request_digest,
    digest_profile: group.digest_profile,
    exact_client_controlled_headers: {
      "Idempotency-Key": group.idempotency_key,
      "X-StoryOS-Anti-Forgery": challenge.nonce,
      "Content-Type": "application/json",
    },
    challenge_expires_at: challenge.expires_at,
    committed_before_send_at: committedAt.toISOString(),
    disposition: { kind: "available" },
  };
  const transaction = workspace.database.transaction([
    "metadata", "partitions", "submission_groups", CAPSULE_STORE, ATTEMPT_STORE,
  ], "readwrite");
  const capsules = transaction.objectStore(CAPSULE_STORE);
  const attempts = transaction.objectStore(ATTEMPT_STORE);
  const [schema, partition, durable, existingCapsules, existingAttempts] = await Promise.all([
    requestResult(transaction.objectStore("metadata").get("schema")),
    requestResult(transaction.objectStore("partitions")
      .get(workspace.partition.journal_partition_id)),
    requestResult(transaction.objectStore("submission_groups")
      .get(group.journal_submission_group_id)),
    requestResult(capsules.index("group").getAll(group.journal_submission_group_id)),
    requestResult(attempts.index("group").getAll(group.journal_submission_group_id)),
  ]);
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || !durable
    || durable.idempotency_key !== group.idempotency_key
    || JSON.stringify(durable.frozen_request_digest)
      !== JSON.stringify(group.frozen_request_digest)
    || JSON.stringify(durable.project_scope) !== JSON.stringify(group.project_scope)
    || durable.settlement?.kind !== "unsettled"
    || existingCapsules.some((row) => row.disposition?.kind === "available")) {
    transaction.abort();
    throw new Error("Author Edit acknowledgement does not converge");
  }
  capsules.add(capsule);
  attempts.add({
    transport_attempt_id: createJournalUuid(workspace.cryptoImpl),
    journal_submission_group_id: group.journal_submission_group_id,
    attempt_ordinal: existingAttempts.length + 1,
    attempt_kind: { kind: "initial" },
    exact_transport_retry_capsule_id: capsule.exact_transport_retry_capsule_id,
    started_at: startedAt.toISOString(),
    outcome: { kind: "in_flight" },
  });
  await transactionResult(transaction);
}

export async function beginOutcomeQueryAttempt(workspace, group, capsule) {
  const attempt = {
    outcome_query_attempt_id: createJournalUuid(workspace.cryptoImpl),
    journal_submission_group_id: group.journal_submission_group_id,
    exact_transport_retry_capsule_id: capsule.exact_transport_retry_capsule_id,
    request_identity: {
      method: "GET",
      route_template: QUERY_ROUTE,
      request_schema: "storyos.query.apply-author-edit-outcome.request.v1",
      idempotency_key: group.idempotency_key,
    },
    started_at: new Date().toISOString(),
    outcome: { kind: "in_flight" },
  };
  const transaction = workspace.database.transaction([
    "metadata", "partitions", "submission_groups", CAPSULE_STORE, QUERY_ATTEMPT_STORE,
  ], "readwrite");
  const [schema, partition, durable, storedCapsule] = await Promise.all([
    requestResult(transaction.objectStore("metadata").get("schema")),
    requestResult(transaction.objectStore("partitions")
      .get(workspace.partition.journal_partition_id)),
    requestResult(transaction.objectStore("submission_groups")
      .get(group.journal_submission_group_id)),
    requestResult(transaction.objectStore(CAPSULE_STORE)
      .get(capsule.exact_transport_retry_capsule_id)),
  ]);
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || !durable
    || durable.idempotency_key !== group.idempotency_key
    || JSON.stringify(durable.frozen_request_digest)
      !== JSON.stringify(group.frozen_request_digest)
    || JSON.stringify(durable.project_scope) !== JSON.stringify(group.project_scope)
    || durable.settlement?.kind !== "unsettled"
    || storedCapsule?.disposition?.kind !== "available"
    || storedCapsule.exact_transport_retry_capsule_id
      !== capsule.exact_transport_retry_capsule_id) {
    transaction.abort();
    throw new Error("Author Edit acknowledgement does not converge");
  }
  transaction.objectStore(QUERY_ATTEMPT_STORE).add(attempt);
  await transactionResult(transaction);
  return attempt;
}

export async function completeOutcomeQueryUnavailable(workspace, group, attempt, evidence) {
  const transaction = workspace.database.transaction([
    "submission_groups", QUERY_ATTEMPT_STORE,
  ], "readwrite");
  const attempts = transaction.objectStore(QUERY_ATTEMPT_STORE);
  const [durable, stored] = await Promise.all([
    requestResult(transaction.objectStore("submission_groups")
      .get(group.journal_submission_group_id)),
    requestResult(attempts.get(attempt.outcome_query_attempt_id)),
  ]);
  if (!durable
    || durable.idempotency_key !== group.idempotency_key
    || stored?.outcome?.kind !== "in_flight"
    || stored.journal_submission_group_id !== group.journal_submission_group_id) {
    transaction.abort();
    throw new Error("Author Edit acknowledgement does not converge");
  }
  attempts.put({
    ...stored,
    outcome: {
      kind: "query_unavailable",
      observed_at: new Date().toISOString(),
      evidence,
      local_response_payload_ref: null,
      exact_response_payload_digest: null,
    },
  });
  await transactionResult(transaction);
}

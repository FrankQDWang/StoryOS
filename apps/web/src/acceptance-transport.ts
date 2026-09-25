import type { EditorReadyState, TransportCapsule } from "./editor-types.ts";
import { decisionDigestProfile, decisionRoute, uuidV7,
  type DecisionFlight } from "./acceptance-journal.ts";

export async function beginAcceptanceAttempt(workspace: EditorReadyState,
  flight: DecisionFlight): Promise<{ id: string; ordinal: number }> {
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "submission_groups", "transport_capsules", "transport_attempts"],
    "readwrite", { durability: "strict" });
  const read = (request: IDBRequest) => new Promise<unknown>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Acceptance transport read failed"));
  });
  const [schema, stored, partition, group, capsuleRows, attemptRows] = await Promise.all([
    read(transaction.objectStore("metadata").get("schema")),
    read(transaction.objectStore("metadata").get(flight.key)),
    read(transaction.objectStore("partitions").get(flight.journal_partition_id)),
    read(transaction.objectStore("submission_groups").get(flight.journal_submission_group_id)),
    read(transaction.objectStore("transport_capsules").index("group")
      .getAll(flight.journal_submission_group_id)),
    read(transaction.objectStore("transport_attempts").index("group")
      .getAll(flight.journal_submission_group_id)),
  ]);
  const capsules = capsuleRows as TransportCapsule[];
  const attempts = (attemptRows as { attempt_ordinal: number;
    exact_transport_retry_capsule_id: string; outcome: { kind: string } }[])
    .sort((left, right) => left.attempt_ordinal - right.attempt_ordinal);
  const capsule = capsules[0];
  if ((schema as { version?: number } | undefined)?.version !== 4
    || JSON.stringify(stored) !== JSON.stringify(flight)
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || workspace.partition.disposition !== "current_writer_open"
    || (group as { settlement?: { kind?: string } })?.settlement?.kind !== "unsettled"
    || JSON.stringify((group as { frozen_request_body?: unknown })?.frozen_request_body)
      !== JSON.stringify(flight.request)
    || (group as { idempotency_key?: string })?.idempotency_key !== flight.idempotencyKey
    || flight.nonce === undefined || flight.challengeExpiresAt === undefined
    || capsules.length > 1
    || (attempts.length > 0 && capsule === undefined)
    || attempts.some((attempt, index) => attempt.attempt_ordinal !== index + 1
      || attempt.exact_transport_retry_capsule_id
        !== capsule?.exact_transport_retry_capsule_id
      || (attempt.outcome?.kind !== "delivery_unknown"
        && (index !== attempts.length - 1 || attempt.outcome?.kind !== "in_flight")))
    || (capsule !== undefined
      && (capsule.disposition.kind !== "available"
        || JSON.stringify(capsule.project_scope) !== JSON.stringify(flight.project_scope)
        || capsule.client_session_binding_ref
          !== workspace.partition.client_session_binding_ref
        || capsule.client_session_generation
          !== workspace.partition.client_session_generation
        || capsule.journal_submission_group_id !== flight.journal_submission_group_id
        || capsule.request_identity.api_major !== 1
        || capsule.request_identity.method !== "POST"
        || capsule.request_identity.route_template !== decisionRoute(flight.command_kind)
        || capsule.request_identity.command_schema !== flight.request.command_schema
        || capsule.request_identity.command_kind !== flight.command_kind
        || capsule.exact_request_body_ref !== flight.journal_submission_group_id
        || capsule.digest_profile !== decisionDigestProfile(flight.command_kind)
        || capsule.exact_client_controlled_headers["Idempotency-Key"]
          !== flight.idempotencyKey
        || capsule.exact_client_controlled_headers["X-StoryOS-Anti-Forgery"]
          !== flight.nonce
        || capsule.exact_client_controlled_headers["Content-Type"] !== "application/json"
        || capsule.challenge_expires_at !== flight.challengeExpiresAt
        || JSON.stringify(capsule.canonical_command_digest)
          !== JSON.stringify(flight.frozen_request_digest)))) {
    transaction.abort();
    throw new Error("Acceptance transport does not match the frozen command");
  }
  const capsuleId = capsules[0]?.exact_transport_retry_capsule_id
    ?? uuidV7(workspace.cryptoImpl);
  if (capsules.length === 0) {
    const capsule: TransportCapsule = {
      exact_transport_retry_capsule_id: capsuleId,
      journal_submission_group_id: flight.journal_submission_group_id,
      project_scope: flight.project_scope,
      client_session_binding_ref: workspace.partition.client_session_binding_ref,
      client_session_generation: workspace.partition.client_session_generation,
      request_identity: { api_major: 1, method: "POST",
        route_template: decisionRoute(flight.command_kind),
        command_schema: flight.request.command_schema, command_kind: flight.command_kind },
      exact_request_body_ref: flight.journal_submission_group_id,
      canonical_command_digest: flight.frozen_request_digest,
      digest_profile: decisionDigestProfile(flight.command_kind),
      exact_client_controlled_headers: { "Idempotency-Key": flight.idempotencyKey,
        "X-StoryOS-Anti-Forgery": flight.nonce, "Content-Type": "application/json" },
      challenge_expires_at: flight.challengeExpiresAt,
      committed_before_send_at: new Date().toISOString(),
      disposition: { kind: "available" },
    };
    transaction.objectStore("transport_capsules").add(capsule);
  }
  const attemptId = uuidV7(workspace.cryptoImpl);
  const abandoned = attempts.at(-1);
  if (abandoned?.outcome.kind === "in_flight") {
    transaction.objectStore("transport_attempts").put({ ...abandoned,
      outcome: { kind: "delivery_unknown", observed_at: new Date().toISOString(),
        evidence: "process_crashed" } });
  }
  transaction.objectStore("transport_attempts").add({
    transport_attempt_id: attemptId,
    journal_submission_group_id: flight.journal_submission_group_id,
    attempt_ordinal: attempts.length + 1,
    attempt_kind: { kind: attempts.length === 0 ? "initial" : "exact_retry" },
    exact_transport_retry_capsule_id: capsuleId,
    started_at: new Date().toISOString(),
    outcome: { kind: "in_flight" },
  });
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Acceptance send was not saved"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Acceptance send was not saved"));
  });
  return { id: attemptId, ordinal: attempts.length + 1 };
}

export async function finishAcceptanceAttempt(database: IDBDatabase, attemptId: string,
  outcome: { kind: "response_observed" } | { kind: "delivery_unknown";
    evidence: "connection_lost" | "response_unreadable" }): Promise<void> {
  const transaction = database.transaction("transport_attempts", "readwrite",
    { durability: "strict" });
  const store = transaction.objectStore("transport_attempts");
  const request = store.get(attemptId);
  request.onsuccess = () => {
    const attempt = request.result as Record<string, unknown> | undefined;
    if (attempt === undefined) { transaction.abort(); return; }
    store.put({ ...attempt, outcome: { ...outcome, observed_at: new Date().toISOString() } });
  };
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Acceptance attempt was not saved"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Acceptance attempt was not saved"));
  });
}

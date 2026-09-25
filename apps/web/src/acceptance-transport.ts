import type { EditorReadyState, TransportCapsule } from "./editor-types.ts";
import { uuidV7, type AcceptanceFlight } from "./acceptance-journal.ts";

export async function beginAcceptanceAttempt(workspace: EditorReadyState,
  flight: AcceptanceFlight): Promise<string> {
  const transaction = workspace.database.transaction(
    ["metadata", "submission_groups", "transport_capsules", "transport_attempts"],
    "readwrite", { durability: "strict" });
  const read = (request: IDBRequest) => new Promise<unknown>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Acceptance transport read failed"));
  });
  const [stored, group, capsuleRows, attemptRows] = await Promise.all([
    read(transaction.objectStore("metadata").get(flight.key)),
    read(transaction.objectStore("submission_groups").get(flight.journal_submission_group_id)),
    read(transaction.objectStore("transport_capsules").index("group")
      .getAll(flight.journal_submission_group_id)),
    read(transaction.objectStore("transport_attempts").index("group")
      .getAll(flight.journal_submission_group_id)),
  ]);
  const capsules = capsuleRows as TransportCapsule[];
  const attempts = attemptRows as unknown[];
  if (JSON.stringify(stored) !== JSON.stringify(flight)
    || (group as { settlement?: { kind?: string } })?.settlement?.kind !== "unsettled"
    || flight.nonce === undefined || flight.challengeExpiresAt === undefined
    || capsules.length > 1
    || (capsules.length === 1
      && (capsules[0]?.exact_client_controlled_headers["Idempotency-Key"]
        !== flight.idempotencyKey
        || capsules[0]?.exact_client_controlled_headers["X-StoryOS-Anti-Forgery"]
          !== flight.nonce
        || JSON.stringify(capsules[0]?.canonical_command_digest)
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
        route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
        command_schema: flight.request.command_schema, command_kind: "acceptProposal" },
      exact_request_body_ref: flight.journal_submission_group_id,
      canonical_command_digest: flight.frozen_request_digest,
      digest_profile: "storyos.command.acceptProposal.jcs.v1",
      exact_client_controlled_headers: { "Idempotency-Key": flight.idempotencyKey,
        "X-StoryOS-Anti-Forgery": flight.nonce, "Content-Type": "application/json" },
      challenge_expires_at: flight.challengeExpiresAt,
      committed_before_send_at: new Date().toISOString(),
      disposition: { kind: "available" },
    };
    transaction.objectStore("transport_capsules").add(capsule);
  }
  const attemptId = uuidV7(workspace.cryptoImpl);
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
  return attemptId;
}

export async function finishAcceptanceAttempt(database: IDBDatabase, attemptId: string,
  outcome: "delivery_unknown" | "response_observed"): Promise<void> {
  const transaction = database.transaction("transport_attempts", "readwrite",
    { durability: "strict" });
  const store = transaction.objectStore("transport_attempts");
  const request = store.get(attemptId);
  request.onsuccess = () => {
    const attempt = request.result as Record<string, unknown> | undefined;
    if (attempt === undefined) { transaction.abort(); return; }
    store.put({ ...attempt, outcome: { kind: outcome, observed_at: new Date().toISOString() } });
  };
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Acceptance attempt was not saved"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Acceptance attempt was not saved"));
  });
}

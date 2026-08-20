import { getSnapshot }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { JOURNAL_DATABASE_VERSION } from "./local-edit-journal.mjs";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const boundedU64 = (value) => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;
const isoInstant = (value) => typeof value === "string"
  && !Number.isNaN(Date.parse(value))
  && new Date(Date.parse(value)).toISOString() === value;

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

function ingestKey(scope) {
  return `project_activity_ingest:${scope.owner_user_id}:${scope.project_id}`;
}

function emptyIngest(workspace) {
  return {
    replay_generation: "1",
    processed_through_stream_sequence: workspace.session.base_snapshot.project_activity_position,
    events: [],
    held: [],
  };
}

function validatedEvent(frame, workspace) {
  if (typeof frame?.id !== "string" || frame.id.length === 0
    || frame.event !== "storyos.project-activity") {
    throw new Error("Project Activity frame is invalid");
  }
  const event = frame.data;
  const scope = workspace.partition.project_scope;
  const payload = event?.payload;
  if (event?.envelope_version !== 1
    || event.activity_profile !== "storyos.project-activity.v1"
    || !UUID.test(event.event_id ?? "")
    || event.event_schema !== "storyos.event.authoritative-author-edit-applied.v1"
    || event.event_kind !== "authoritative_author_edit_applied"
    || JSON.stringify(event.project_scope) !== JSON.stringify(scope)
    || event.requester_user_id !== scope.owner_user_id
    || event.actor?.kind !== "author"
    || event.actor?.id !== scope.owner_user_id
    || !boundedU64(event.project_sequence)
    || event.stream_sequence !== event.project_sequence
    || event.agent_run_id !== null
    || event.run_step_id !== null
    || event.run_sequence !== null
    || event.aggregate_ref?.kind !== "chapter"
    || event.aggregate_ref?.id !== payload?.chapter_id
    || !UUID.test(event.correlation_id ?? "")
    || event.causation?.kind !== "command"
    || event.causation?.id !== event.command_id
    || !UUID.test(event.command_id ?? "")
    || event.receipt_ref?.kind !== "domain_receipt"
    || !UUID.test(event.receipt_ref?.id ?? "")
    || !isoInstant(event.occurred_at)
    || event.recorded_at !== event.occurred_at
    || payload?.chapter_id !== workspace.session.base_snapshot.chapter_id
    || !UUID.test(payload?.authoritative_revision_id ?? "")
    || !UUID.test(payload?.authoritative_commit_id ?? "")
    || payload?.author_action_sequence !== event.stream_sequence
    || event.payload_digest?.algorithm !== "sha256"
    || event.payload_digest?.profile !== "storyos.event-payload.jcs.v1"
    || !/^[0-9a-f]{64}$/.test(event.payload_digest?.value_hex_lowercase ?? "")
    || event.application_wire_record_ref !== event.event_id
    || event.limit_profile_revision !== workspace.partition.limit_profile_revision) {
    throw new Error("Project Activity Event is invalid");
  }
  return event;
}

function applyFrames(state, frames, workspace) {
  const next = {
    replay_generation: state.replay_generation,
    processed_through_stream_sequence: state.processed_through_stream_sequence,
    events: [...state.events],
    held: [...state.held],
  };
  if (!boundedU64(next.replay_generation)) {
    throw new Error("Project Activity replay generation mismatch");
  }
  for (const frame of frames) {
    const event = validatedEvent(frame, workspace);
    const knownById = [...next.events, ...next.held]
      .find((candidate) => candidate.event_id === event.event_id);
    if (knownById) {
      if (JSON.stringify(knownById) !== JSON.stringify(event)) {
        throw new Error("Project Activity Event conflict");
      }
      continue;
    }
    const knownBySequence = [...next.events, ...next.held]
      .find((candidate) => candidate.stream_sequence === event.stream_sequence);
    if (knownBySequence
      || BigInt(event.stream_sequence) <= BigInt(next.processed_through_stream_sequence)) {
      throw new Error("Project Activity Event conflict");
    }
    next.held.push(event);
  }
  next.held.sort((left, right) => {
    const delta = BigInt(left.stream_sequence) - BigInt(right.stream_sequence);
    return delta < 0n ? -1 : delta > 0n ? 1 : 0;
  });
  while (next.held[0]
    && BigInt(next.held[0].stream_sequence)
      === BigInt(next.processed_through_stream_sequence) + 1n) {
    const event = next.held.shift();
    next.events.push(event);
    next.processed_through_stream_sequence = event.stream_sequence;
  }
  if (BigInt(next.processed_through_stream_sequence)
    < BigInt(state.processed_through_stream_sequence)
    || next.events.length < state.events.length) {
    throw new Error("Project Activity ingest refused a weaker snapshot");
  }
  return next;
}

function validatedCanonicalSnapshot(response, workspace, snapshotId) {
  const scope = workspace.partition.project_scope;
  const snapshot = response?.snapshot;
  if (response?.schema_id !== "storyos.query.snapshot.response.v1"
    || !UUID.test(response.correlation_id ?? "")
    || JSON.stringify(response.project_scope) !== JSON.stringify(scope)
    || snapshot?.snapshot_id !== snapshotId
    || JSON.stringify(snapshot.project_scope) !== JSON.stringify(scope)
    || snapshot.snapshot_kind !== "canonical"
    || !boundedU64(snapshot.project_activity_position)
    || JSON.stringify(snapshot.source_watermarks) !== "{}"
    || JSON.stringify(snapshot.projection_generations) !== "{}"
    || snapshot.redaction_profile !== "storyos.author.v1"
    || snapshot.schema_profile !== "storyos.public.release.1"
    || !boundedU64(snapshot.replay_generation)
    || !isoInstant(snapshot.created_at)
    || (snapshot.expires_at !== null && !isoInstant(snapshot.expires_at))) {
    throw new Error("Canonical Snapshot is invalid");
  }
  return {
    snapshot_id: snapshot.snapshot_id,
    project_scope: snapshot.project_scope,
    snapshot_kind: snapshot.snapshot_kind,
    project_activity_position: snapshot.project_activity_position,
    source_watermarks: snapshot.source_watermarks,
    projection_generations: snapshot.projection_generations,
    redaction_profile: snapshot.redaction_profile,
    schema_profile: snapshot.schema_profile,
    replay_generation: snapshot.replay_generation,
    created_at: snapshot.created_at,
    expires_at: snapshot.expires_at ?? null,
  };
}

function ingestAfterSnapshot(current, snapshot) {
  const nextGeneration = BigInt(snapshot.replay_generation);
  const currentGeneration = BigInt(current.replay_generation);
  if (nextGeneration < currentGeneration
    || (nextGeneration === currentGeneration
      && BigInt(snapshot.project_activity_position)
        < BigInt(current.processed_through_stream_sequence))) {
    throw new Error("Project Activity ingest refused a weaker snapshot");
  }
  return {
    replay_generation: snapshot.replay_generation,
    processed_through_stream_sequence: snapshot.project_activity_position,
    events: [],
    held: [],
  };
}

export async function readProjectActivityIngest(workspace) {
  const transaction = workspace.database.transaction(["metadata"], "readonly");
  const stored = await requestResult(
    transaction.objectStore("metadata").get(ingestKey(workspace.partition.project_scope)),
  );
  return stored?.value ?? emptyIngest(workspace);
}

export async function resyncProjectActivityFromSnapshot(workspace, {
  baseUrl, snapshotId, fetchImpl = globalThis.fetch,
}) {
  const scope = workspace.partition.project_scope;
  const snapshot = validatedCanonicalSnapshot(
    await getSnapshot({
      baseUrl, projectId: scope.project_id, snapshotId, fetchImpl,
    }),
    workspace,
    snapshotId,
  );
  const key = ingestKey(scope);
  const partitionId = workspace.partition.journal_partition_id;
  // Read retained journal rows in this transaction so the ingest rewrite cannot
  // land beside a concurrent payload write. This layer does not mutate those rows.
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "payload_chains", "intents", "submission_groups"],
    "readwrite",
  );
  const metadata = transaction.objectStore("metadata");
  const [schema, partition, stored] = await Promise.all([
    requestResult(metadata.get("schema")),
    requestResult(transaction.objectStore("partitions").get(partitionId)),
    requestResult(metadata.get(key)),
    requestResult(transaction.objectStore("intents").index("partition").getAll(partitionId)),
    requestResult(transaction.objectStore("payload_chains").index("partition")
      .getAll(partitionId)),
    requestResult(transaction.objectStore("submission_groups").index("partition")
      .getAll(partitionId)),
  ]);
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)) {
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const next = ingestAfterSnapshot(stored?.value ?? emptyIngest(workspace), snapshot);
  metadata.put({ key, value: next });
  await transactionResult(transaction);
  return { snapshot, ingest: next };
}

export async function ingestProjectActivityFrames(workspace, frames) {
  const key = ingestKey(workspace.partition.project_scope);
  const partitionId = workspace.partition.journal_partition_id;
  const transaction = workspace.database.transaction(
    ["metadata", "partitions"],
    "readwrite",
  );
  const metadata = transaction.objectStore("metadata");
  const [schema, partition, stored] = await Promise.all([
    requestResult(metadata.get("schema")),
    requestResult(transaction.objectStore("partitions").get(partitionId)),
    requestResult(metadata.get(key)),
  ]);
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)) {
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const current = stored?.value ?? emptyIngest(workspace);
  const next = applyFrames(current, frames, workspace);
  metadata.put({ key, value: next });
  await transactionResult(transaction);
  return next;
}

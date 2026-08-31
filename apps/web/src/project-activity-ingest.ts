import { getSnapshot }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetSnapshotResponse,
  ProjectScope,
  SnapshotDescriptor,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  EditorWorkspace,
  ProjectActivityEvent,
  ProjectActivityIngest,
} from "./editor-types.ts";
import { JOURNAL_DATABASE_VERSION } from "./local-edit-journal.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const boundedU64 = (value: unknown): value is string => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;
const isoInstant = (value: unknown): value is string => typeof value === "string"
  && !Number.isNaN(Date.parse(value))
  && new Date(Date.parse(value)).toISOString() === value;

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

function ingestKey(scope: ProjectScope): string {
  return `project_activity_ingest:${scope.owner_user_id}:${scope.project_id}`;
}

function emptyIngest(workspace: EditorWorkspace): ProjectActivityIngest {
  return {
    replay_generation: "1",
    processed_through_stream_sequence: workspace.session.base_snapshot.project_activity_position,
    events: [],
    held: [],
  };
}

function validatedEvent(frameValue: unknown, workspace: EditorWorkspace): ProjectActivityEvent {
  const frame = frameValue as {
    id?: unknown;
    event?: unknown;
    data?: ProjectActivityEvent;
  };
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
    || !boundedU64(payload?.author_action_sequence) || payload.author_action_sequence === "0"
    || event.payload_digest?.algorithm !== "sha256"
    || event.payload_digest?.profile !== "storyos.event-payload.jcs.v1"
    || !/^[0-9a-f]{64}$/.test(event.payload_digest?.value_hex_lowercase ?? "")
    || event.application_wire_record_ref !== event.event_id
    || event.limit_profile_revision !== workspace.partition.limit_profile_revision) {
    throw new Error("Project Activity Event is invalid");
  }
  return event;
}

function applyFrames(
  state: ProjectActivityIngest,
  frames: readonly unknown[],
  workspace: EditorWorkspace,
): ProjectActivityIngest {
  const next = {
    replay_generation: state.replay_generation,
    processed_through_stream_sequence: state.processed_through_stream_sequence,
    events: [...state.events],
    held: [...state.held],
  };
  if (!boundedU64(next.replay_generation)) {
    throw new Error("Project Activity replay generation mismatch");
  }
  const known = [...next.events, ...next.held];
  for (const frame of frames) {
    const event = validatedEvent(frame, workspace);
    const knownById = known.find((candidate) => candidate.event_id === event.event_id);
    if (knownById) {
      if (JSON.stringify(knownById) !== JSON.stringify(event)) {
        throw new Error("Project Activity Event conflict");
      }
      continue;
    }
    const knownBySequence = known.find(
      (candidate) => candidate.stream_sequence === event.stream_sequence,
    );
    if (knownBySequence
      || BigInt(event.stream_sequence) <= BigInt(next.processed_through_stream_sequence)) {
      throw new Error("Project Activity Event conflict");
    }
    next.held.push(event);
    known.push(event);
  }
  next.held.sort((left, right) => {
    const delta = BigInt(left.stream_sequence) - BigInt(right.stream_sequence);
    return delta < 0n ? -1 : delta > 0n ? 1 : 0;
  });
  while (next.held[0]
    && BigInt(next.held[0].stream_sequence)
      === BigInt(next.processed_through_stream_sequence) + 1n) {
    const event = next.held.shift()!;
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

export function validatedCanonicalSnapshot(
  response: unknown,
  workspace: EditorWorkspace,
  snapshotId: string,
): SnapshotDescriptor {
  const candidate = response as GetSnapshotResponse;
  const scope = workspace.partition.project_scope;
  const snapshot = candidate?.snapshot;
  if (candidate?.schema_id !== "storyos.query.snapshot.response.v1"
    || !UUID.test(candidate.correlation_id ?? "")
    || JSON.stringify(candidate.project_scope) !== JSON.stringify(scope)
    || !UUID.test(snapshotId)
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
  } as SnapshotDescriptor;
}

function ingestAfterSnapshot(
  current: ProjectActivityIngest,
  snapshot: SnapshotDescriptor,
): ProjectActivityIngest {
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

export async function readProjectActivityIngest(
  workspace: EditorWorkspace,
): Promise<ProjectActivityIngest> {
  const transaction = workspace.database.transaction(["metadata"], "readonly");
  const stored = await requestResult(
    transaction.objectStore("metadata").get(ingestKey(workspace.partition.project_scope)),
  );
  const record = stored as { value?: ProjectActivityIngest } | undefined;
  return record?.value ?? emptyIngest(workspace);
}

/** Stage canonical ingest state in the caller's journal installation transaction. */
export async function stageProjectActivitySnapshot(
  workspace: EditorWorkspace,
  transaction: IDBTransaction,
  snapshot: SnapshotDescriptor,
): Promise<ProjectActivityIngest> {
  const key = ingestKey(workspace.partition.project_scope);
  const metadata = transaction.objectStore("metadata");
  const stored = await requestResult(metadata.get(key)) as { value?: ProjectActivityIngest } | undefined;
  if (snapshot.expires_at !== null && Date.parse(snapshot.expires_at) <= Date.now()) {
    throw new Error("Canonical Snapshot has expired");
  }
  const next = ingestAfterSnapshot(stored?.value ?? emptyIngest(workspace), snapshot);
  metadata.put({ key, value: next });
  return next;
}

export async function resyncProjectActivityFromSnapshot(workspace: EditorWorkspace, {
  baseUrl, snapshotId, fetchImpl = globalThis.fetch, signal,
}: {
  baseUrl: string;
  snapshotId: string;
  fetchImpl?: typeof fetch;
  signal?: AbortSignal;
}): Promise<{ snapshot: SnapshotDescriptor; ingest: ProjectActivityIngest }> {
  const scope = workspace.partition.project_scope;
  const response: unknown = await getSnapshot({
    baseUrl, projectId: scope.project_id, snapshotId, fetchImpl,
    ...(signal === undefined ? {} : { signal }),
  });
  const snapshot = validatedCanonicalSnapshot(
    response,
    workspace,
    snapshotId,
  );
  const partitionId = workspace.partition.journal_partition_id;
  // Read retained journal rows in this transaction so the ingest rewrite cannot
  // land beside a concurrent payload write. This layer does not mutate those rows.
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "payload_chains", "intents", "submission_groups"],
    "readwrite",
  );
  const metadata = transaction.objectStore("metadata");
  const [schemaValue, partition] = await Promise.all([
    requestResult(metadata.get("schema")),
    requestResult(transaction.objectStore("partitions").get(partitionId)),
    requestResult(transaction.objectStore("intents").index("partition").getAll(partitionId)),
    requestResult(transaction.objectStore("payload_chains").index("partition")
      .getAll(partitionId)),
    requestResult(transaction.objectStore("submission_groups").index("partition")
      .getAll(partitionId)),
  ]);
  const schema = schemaValue as { version?: unknown } | undefined;
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)) {
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const next = await stageProjectActivitySnapshot(workspace, transaction, snapshot);
  await transactionResult(transaction);
  return { snapshot, ingest: next };
}

export async function ingestProjectActivityFrames(
  workspace: EditorWorkspace,
  frames: readonly unknown[],
): Promise<ProjectActivityIngest> {
  const key = ingestKey(workspace.partition.project_scope);
  const partitionId = workspace.partition.journal_partition_id;
  const transaction = workspace.database.transaction(
    ["metadata", "partitions"],
    "readwrite",
  );
  const metadata = transaction.objectStore("metadata");
  const [schemaValue, partition, storedValue] = await Promise.all([
    requestResult(metadata.get("schema")),
    requestResult(transaction.objectStore("partitions").get(partitionId)),
    requestResult(metadata.get(key)),
  ]);
  const schema = schemaValue as { version?: unknown } | undefined;
  const stored = storedValue as { value?: ProjectActivityIngest } | undefined;
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

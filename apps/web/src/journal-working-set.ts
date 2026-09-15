import type { EditorWorkspace, ValidatedJournalSnapshot, JournalSubmissionGroup,
  JournalIntentRecord, JournalPayloadChain }
  from "./editor-types.ts";

export const MAX_WORKING_JOURNAL_ITEMS = 2400;
const STORES = ["intents", "payload_chains", "submission_groups"] as const;
const equal = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);
const result = (request: IDBRequest): Promise<unknown> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("Journal request failed"));
});

function objectValue(value: unknown): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Journal persisted object is corrupt");
  }
  return value as Record<string, unknown>;
}

function metadataValue(value: unknown): unknown {
  return value === undefined ? undefined : objectValue(value).value;
}

export interface JournalWorkingBoundary {
  last_sequence: number;
  completed_intent_record_id: string;
  journal_submission_group_id: string;
  collection_fence_id: string;
}

export function upgradeJournalWorkingIndexes(transaction: IDBTransaction) {
  for (const name of STORES) {
    const store = transaction.objectStore(name);
    store.createIndex("working_partition", "working_set_partition_id");
    const request = store.openCursor();
    request.onsuccess = () => {
      const cursor = request.result;
      if (!cursor) return;
      try {
        const value: unknown = cursor.value;
        const row = objectValue(value);
        if (typeof row.journal_partition_id !== "string") throw new Error("Journal partition is corrupt");
        cursor.update({ ...row, working_set_partition_id: row.journal_partition_id });
        cursor.continue();
      } catch { transaction.abort(); }
    };
  }
}

export async function readJournalWorkingBoundary(
  transaction: IDBTransaction, workspace: EditorWorkspace,
): Promise<JournalWorkingBoundary | undefined> {
  const partitionId = workspace.partition.journal_partition_id;
  const metadata = transaction.objectStore("metadata");
  const stored = await result(metadata.get(`working_boundary:${partitionId}`));
  if (stored === undefined) return undefined;
  const candidate = objectValue(metadataValue(stored));
  const boundary = candidate as Partial<JournalWorkingBoundary>;
  if (typeof boundary.last_sequence !== "number"
    || !Number.isSafeInteger(boundary.last_sequence) || boundary.last_sequence < 1
    || typeof boundary.completed_intent_record_id !== "string"
    || typeof boundary.journal_submission_group_id !== "string"
    || typeof boundary.collection_fence_id !== "string") {
    throw new Error("Journal working boundary is corrupt");
  }
  const [recordValue, groupValue, retainedFence] = await Promise.all([
    result(transaction.objectStore("intents").get([partitionId, boundary.last_sequence])),
    result(transaction.objectStore("submission_groups").get(boundary.journal_submission_group_id)),
    result(metadata.get(`retained_collection_fence:${boundary.collection_fence_id}`)),
  ]);
  const record = objectValue(recordValue);
  const group = objectValue(groupValue) as Partial<JournalSubmissionGroup>;
  if (record?.completed_intent_record_id !== boundary.completed_intent_record_id
    || record?.author_edit_unit !== undefined || record?.working_set_partition_id !== undefined
    || group?.working_set_partition_id !== undefined
    || group?.covered_sequence_range?.last !== boundary.last_sequence
    || group?.journal_submission_group_id !== boundary.journal_submission_group_id
    || record?.journal_partition_id !== partitionId
    || record?.editor_session_id !== workspace.partition.editor_session_id
    || record?.writer_generation !== workspace.partition.writer_generation
    || !equal(record?.project_scope, workspace.partition.project_scope)
    || group?.payload_collection?.collection_fence_id !== boundary.collection_fence_id
    || group?.journal_partition_id !== partitionId
    || group?.writer_generation !== workspace.partition.writer_generation
    || group?.editor_session_id !== workspace.partition.editor_session_id
    || !equal(group?.project_scope, workspace.partition.project_scope)
    || !equal(group?.ordered_coverage?.at(-1), {
      local_intent_sequence: boundary.last_sequence,
      intent_record_ref: boundary.completed_intent_record_id,
      payload_digest: record?.payload_digest,
    }) || !collectionProofMatches(group, metadataValue(retainedFence))) {
    throw new Error("Journal working boundary is corrupt");
  }
  return { last_sequence: boundary.last_sequence,
    completed_intent_record_id: boundary.completed_intent_record_id,
    journal_submission_group_id: boundary.journal_submission_group_id,
    collection_fence_id: boundary.collection_fence_id };
}

function collectionProofMatches(group: Partial<JournalSubmissionGroup>, fence: unknown) {
  if (fence === null || typeof fence !== "object"
    || group.payload_collection?.kind !== "collected"
    || group.settlement?.kind !== "applied_receipt_settled"
    || group.reconciliation !== undefined
    || !Array.isArray(group.ordered_coverage)) return false;
  const settlement = group.settlement;
  const successor = settlement.installed_base_snapshot;
  return Reflect.get(fence, "partition_disposition") === "current_writer_open"
    && Reflect.get(fence, "resulting_writer_generation") === undefined
    && Reflect.get(fence, "collection_fence_id") === group.payload_collection.collection_fence_id
    && Reflect.get(fence, "journal_partition_id") === group.journal_partition_id
    && Reflect.get(fence, "writer_generation") === group.writer_generation
    && equal(Reflect.get(fence, "project_scope"), group.project_scope)
    && equal(Reflect.get(fence, "collected_groups"), [{
      journal_submission_group_id: group.journal_submission_group_id,
      covered_sequence_range: group.covered_sequence_range,
      payload_digests: group.ordered_coverage.map((item) => item.payload_digest),
      settlement_kind: settlement.kind,
      command_id: settlement.command_id,
      author_command_admission_id: settlement.author_command_admission_id,
      receipt_id: settlement.receipt.receipt_id,
      project_activity_position: settlement.project_activity_position,
    }]) && equal(Reflect.get(fence, "successor"), {
      kind: "authoritative_revision", snapshot_id: successor.snapshot_id,
      revision_id: successor.authoritative_head_revision_id,
      materialized_payload_digest: successor.materialized_payload_digest,
    }) && equal(Reflect.get(fence, "collected_intent_sequences"),
      group.ordered_coverage.map((item) => item.local_intent_sequence))
    && Reflect.get(fence, "reason") === "applied_receipt_converged_with_durable_successor";
}

export async function retireCollectedJournalPrefix(
  workspace: EditorWorkspace, snapshot: ValidatedJournalSnapshot,
): Promise<boolean> {
  if (snapshot.records.length < MAX_WORKING_JOURNAL_ITEMS
    && snapshot.payloadChains.length < MAX_WORKING_JOURNAL_ITEMS
    && snapshot.groups.length < MAX_WORKING_JOURNAL_ITEMS) return false;
  if (workspace.partition.disposition !== "current_writer_open"
    || workspace.session.writer.kind !== "current_writer"
    || workspace.session.writer.writer_generation !== workspace.partition.writer_generation) {
    return false;
  }
  const prefix: JournalSubmissionGroup[] = [];
  const fences = new Map(snapshot.fences.map((fence) => [
    fence !== null && typeof fence === "object" ? Reflect.get(fence, "collection_fence_id") : undefined,
    fence,
  ]));
  let recordCount = 0;
  const chainIds = new Set<string>();
  for (const group of snapshot.groups) {
    const fenceId = group.payload_collection?.collection_fence_id;
    if (!collectionProofMatches(group, fences.get(fenceId))) break;
    const records = snapshot.records.slice(recordCount, recordCount + group.ordered_coverage.length);
    const ids = new Set(records.map((record) => record.payload_chain_ref));
    const chains = snapshot.payloadChains.filter((chain) => ids.has(chain.payload_chain_id));
    if (records.some((record) => record.author_edit_unit !== undefined)
      || chains.length !== ids.size
      || chains.some((chain) => chain.payload_collection?.collection_fence_id !== fenceId)
      || !equal(Reflect.get(fences.get(fenceId) as object, "collected_payload_chain_ids"), [...ids])
      || snapshot.records.some((record) => ids.has(record.payload_chain_ref)
        && !records.includes(record))) break;
    prefix.push(group);
    recordCount += records.length;
    for (const id of ids) chainIds.add(id);
  }
  if (prefix.length === 0) return false;
  const partitionId = workspace.partition.journal_partition_id;
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", ...STORES], "readwrite", { durability: "strict" },
  );
  const completed = new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Journal transaction aborted"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Journal transaction failed"));
  });
  // Attach before awaiting reads so a failed request cannot leave an unhandled abort.
  void completed.catch(() => {});
  try {
    const metadata = transaction.objectStore("metadata");
    const [schema, partition, activeBase, watermark, boundary, durableFences, ...rows] = await Promise.all([
      result(metadata.get("schema")),
      result(transaction.objectStore("partitions").get(partitionId)),
      result(metadata.get(`active_base:${partitionId}`)),
      result(metadata.get(`durable_high_watermark:${partitionId}`)),
      result(metadata.get(`working_boundary:${partitionId}`)),
      result(metadata.get(`collection_fences:${partitionId}`)),
      ...STORES.map((name) => result(transaction.objectStore(name).index("working_partition")
        .getAll(partitionId, MAX_WORKING_JOURNAL_ITEMS + 1))),
    ]);
    if (rows.some((row) => !Array.isArray(row))) throw new Error("Journal working rows are corrupt");
    const durableRows = rows as [JournalIntentRecord[], JournalPayloadChain[], JournalSubmissionGroup[]];
    durableRows[0].sort((left, right) => left.local_intent_sequence - right.local_intent_sequence);
    durableRows[2].sort((left, right) => left.covered_sequence_range.first - right.covered_sequence_range.first);
    if (objectValue(schema).version !== workspace.database.version
      || !equal(partition, workspace.partition) || !equal(metadataValue(activeBase), snapshot.activeBase)
      || !equal(watermark, snapshot.watermark) || !equal(metadataValue(boundary), snapshot.workingBoundary)
      || !equal(metadataValue(durableFences) ?? [], snapshot.fences)
      || !equal(durableRows, [snapshot.records, snapshot.payloadChains, snapshot.groups])) {
      throw new Error("Local Edit Journal changed before working boundary advance");
    }
    const retired = [snapshot.records.slice(0, recordCount),
      snapshot.payloadChains.filter((chain) => chainIds.has(chain.payload_chain_id)), prefix];
    for (const [index, name] of STORES.entries()) {
      for (const row of retired[index]!) {
        const { working_set_partition_id: _working, ...retained } = row;
        transaction.objectStore(name).put(retained);
      }
    }
    const retiredFences = new Set(prefix.map((group) => group.payload_collection!.collection_fence_id));
    for (const id of retiredFences) {
      metadata.add({ key: `retained_collection_fence:${id}`, value: fences.get(id) });
    }
    metadata.put({ key: `collection_fences:${partitionId}`,
      value: snapshot.fences.filter((fence) => !retiredFences.has(
        Reflect.get(fence as object, "collection_fence_id"))),
    });
    const last = snapshot.records[recordCount - 1]!;
    const lastGroup = prefix.at(-1)!;
    metadata.put({ key: `working_boundary:${partitionId}`, value: {
      last_sequence: last.local_intent_sequence,
      completed_intent_record_id: last.completed_intent_record_id,
      journal_submission_group_id: lastGroup.journal_submission_group_id,
      collection_fence_id: lastGroup.payload_collection!.collection_fence_id,
    } satisfies JournalWorkingBoundary });
    await completed;
    return true;
  } catch (error) {
    try { transaction.abort(); } catch { /* The failed transaction can already be closed. */ }
    throw error;
  }
}

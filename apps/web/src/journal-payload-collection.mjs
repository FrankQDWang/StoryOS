import {
  JOURNAL_DATABASE_VERSION,
  createJournalUuid,
} from "./local-edit-journal.mjs";

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

function fencesKey(partitionId) {
  return `collection_fences:${partitionId}`;
}

function partitionAllowsCollection(workspace, group) {
  const partition = workspace.partition;
  if (group.writer_generation !== partition.writer_generation) return false;
  if (partition.disposition === "current_writer_open") {
    return workspace.session.writer?.kind === "current_writer"
      && workspace.session.writer.writer_generation === partition.writer_generation;
  }
  const writer = workspace.session.writer;
  const resulting = writer?.observed_writer_generation;
  return partition.disposition === "read_only_observer"
    && writer?.kind === "read_only"
    && writer.reason === "superseded_by_takeover"
    && typeof resulting === "string"
    && /^(?:0|[1-9][0-9]{0,19})$/.test(resulting)
    && BigInt(resulting) <= 18446744073709551615n
    && BigInt(resulting) > BigInt(partition.writer_generation);
}

function appliedSuccessor(group, durableActiveBase, workspace) {
  const successor = group.settlement?.installed_base_snapshot;
  return group.settlement?.kind === "applied_receipt_settled"
    && group.payload_collection?.kind !== "collected"
    && successor
    && JSON.stringify(successor) === JSON.stringify(durableActiveBase)
    && JSON.stringify(durableActiveBase) === JSON.stringify(workspace.session.base_snapshot)
    && group.settlement.project_activity_position === successor.project_activity_position
    && group.settlement.authoritative_revision?.revision_id
      === successor.authoritative_head_revision_id
    && JSON.stringify(group.settlement.authoritative_revision)
      === JSON.stringify(successor.materialized_revision)
    && successor.materialized_payload_digest?.algorithm === "sha256"
    && successor.materialized_payload_digest?.profile === "storyos.canonical-payload.sha256.v1";
}

function collectRecord(record) {
  const next = { ...record };
  delete next.author_edit_unit;
  return next;
}

function collectChain(chain, collectionFenceId) {
  const { materialized_payload: _payload, ...checkpoint } = chain.checkpoint_ref;
  return {
    ...chain,
    payload_collection: { kind: "collected", collection_fence_id: collectionFenceId },
    checkpoint_ref: checkpoint,
    ordered_patch_refs: chain.ordered_patch_refs.map((patch) => {
      const { normalized_primitives: _primitives, ...rest } = patch;
      return rest;
    }),
  };
}

function collectGroup(group, collectionFenceId) {
  return {
    ...group,
    frozen_request_body: { ...group.frozen_request_body, author_edit_units: [] },
    payload_collection: { kind: "collected", collection_fence_id: collectionFenceId },
  };
}

export async function collectEligibleJournalPayload(workspace) {
  const partitionId = workspace.partition.journal_partition_id;
  const key = fencesKey(partitionId);
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "payload_chains", "intents", "submission_groups"],
    "readwrite",
  );
  const metadata = transaction.objectStore("metadata");
  const intents = transaction.objectStore("intents");
  const chains = transaction.objectStore("payload_chains");
  const groups = transaction.objectStore("submission_groups");
  const [schema, partition, storedFences, activeBase, records, payloadChains, durableGroups] =
    await Promise.all([
      requestResult(metadata.get("schema")),
      requestResult(transaction.objectStore("partitions").get(partitionId)),
      requestResult(metadata.get(key)),
      requestResult(metadata.get(`active_base:${partitionId}`)),
      requestResult(intents.index("partition").getAll(partitionId)),
      requestResult(chains.index("partition").getAll(partitionId)),
      requestResult(groups.index("partition").getAll(partitionId)),
    ]);
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || JSON.stringify(activeBase?.value) !== JSON.stringify(workspace.session.base_snapshot)) {
    throw new Error("Local Edit Journal schema is incompatible");
  }
  const fences = [...(storedFences?.value ?? [])];
  const covered = new Set(durableGroups.flatMap((group) =>
    (group.ordered_coverage ?? []).map((item) => item.local_intent_sequence)));
  const eligible = durableGroups.filter((group) =>
    appliedSuccessor(group, activeBase?.value, workspace)
      && partitionAllowsCollection(workspace, group));
  for (const group of eligible) {
    const sequences = new Set(group.ordered_coverage.map((item) => item.local_intent_sequence));
    const groupRecords = records.filter((record) => sequences.has(record.local_intent_sequence));
    const chainIds = [...new Set(groupRecords.map((record) => record.payload_chain_ref))];
    if (groupRecords.some((record) => !covered.has(record.local_intent_sequence))
      || chainIds.some((chainId) => records.some((record) =>
        record.payload_chain_ref === chainId && !sequences.has(record.local_intent_sequence)))) {
      continue;
    }
    const collectionFenceId = createJournalUuid(workspace.cryptoImpl);
    const fence = {
      collection_fence_id: collectionFenceId,
      journal_partition_id: partitionId,
      project_scope: workspace.partition.project_scope,
      writer_generation: workspace.partition.writer_generation,
      partition_disposition: workspace.partition.disposition,
      ...(workspace.partition.disposition === "read_only_observer"
        ? { resulting_writer_generation: workspace.session.writer.observed_writer_generation }
        : {}),
      collected_groups: [{
        journal_submission_group_id: group.journal_submission_group_id,
        covered_sequence_range: group.covered_sequence_range,
        payload_digests: group.ordered_coverage.map((item) => item.payload_digest),
        settlement_kind: group.settlement.kind,
        command_id: group.settlement.command_id,
        author_command_admission_id: group.settlement.author_command_admission_id,
        receipt_id: group.settlement.receipt.receipt_id,
        project_activity_position: group.settlement.project_activity_position,
      }],
      successor: {
        kind: "authoritative_revision",
        snapshot_id: group.settlement.installed_base_snapshot.snapshot_id,
        revision_id: group.settlement.installed_base_snapshot.authoritative_head_revision_id,
        materialized_payload_digest:
          group.settlement.installed_base_snapshot.materialized_payload_digest,
      },
      collected_intent_sequences: [...sequences].sort((left, right) => left - right),
      collected_payload_chain_ids: chainIds,
      reason: "applied_receipt_converged_with_durable_successor",
    };
    fences.push(fence);
    groups.put(collectGroup(group, collectionFenceId));
    for (const record of groupRecords) intents.put(collectRecord(record));
    for (const chainId of chainIds) {
      const chain = payloadChains.find((candidate) => candidate.payload_chain_id === chainId);
      if (chain) chains.put(collectChain(chain, collectionFenceId));
    }
  }
  if (JSON.stringify(fences) !== JSON.stringify(storedFences?.value ?? [])) {
    metadata.put({ key, value: fences });
  }
  await transactionResult(transaction);
  return { fences };
}

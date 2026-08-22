import {
  JOURNAL_DATABASE_VERSION,
  createJournalUuid,
} from "./local-edit-journal.ts";
import type { EditorWriterProjection }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  EditorWorkspace,
  JournalIntentRecord,
  JournalPayloadChain,
  JournalSubmissionGroup,
  SubmissionSettlement,
} from "./editor-types.ts";

type AppliedSettlement = Extract<
  SubmissionSettlement,
  { kind: "applied_receipt_settled" }
>;
type AppliedSubmissionGroup = JournalSubmissionGroup & { settlement: AppliedSettlement };

const requestResult = (request: IDBRequest): Promise<unknown> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction: IDBTransaction) => new Promise<void>((resolve, reject) => {
  transaction.oncomplete = () => resolve();
  transaction.onabort = () => reject(
    transaction.error ?? new Error("IndexedDB transaction aborted"),
  );
  transaction.onerror = () => reject(
    transaction.error ?? new Error("IndexedDB transaction failed"),
  );
});

function fencesKey(partitionId: string) {
  return `collection_fences:${partitionId}`;
}

function partitionAllowsCollection(
  workspace: EditorWorkspace,
  group: JournalSubmissionGroup,
) {
  const partition = workspace.partition;
  if (group.writer_generation !== partition.writer_generation) return false;
  if (partition.disposition === "current_writer_open") {
    return workspace.session.writer?.kind === "current_writer"
      && workspace.session.writer.writer_generation === partition.writer_generation;
  }
  const writer = workspace.session.writer as
    (EditorWriterProjection & { observed_writer_generation?: string }) | undefined;
  const resulting = writer?.observed_writer_generation;
  return partition.disposition === "read_only_observer"
    && writer?.kind === "read_only"
    && writer.reason === "superseded_by_takeover"
    && typeof resulting === "string"
    && /^(?:0|[1-9][0-9]{0,19})$/.test(resulting)
    && BigInt(resulting) <= 18446744073709551615n
    && BigInt(resulting) > BigInt(partition.writer_generation);
}

function appliedSuccessor(
  group: JournalSubmissionGroup,
  durableActiveBase: unknown,
  workspace: EditorWorkspace,
) {
  const settlement = group.settlement as Partial<AppliedSettlement>;
  const successor = settlement?.installed_base_snapshot;
  return settlement?.kind === "applied_receipt_settled"
    && group.payload_collection?.kind !== "collected"
    && successor
    && JSON.stringify(successor) === JSON.stringify(durableActiveBase)
    && JSON.stringify(durableActiveBase) === JSON.stringify(workspace.session.base_snapshot)
    && settlement.project_activity_position === successor.project_activity_position
    && settlement.authoritative_revision?.revision_id
      === successor.authoritative_head_revision_id
    && JSON.stringify(settlement.authoritative_revision)
      === JSON.stringify(successor.materialized_revision)
    && successor.materialized_payload_digest?.algorithm === "sha256"
    && successor.materialized_payload_digest?.profile === "storyos.canonical-payload.sha256.v1";
}

function collectRecord(record: JournalIntentRecord): JournalIntentRecord {
  const next = { ...record };
  delete next.author_edit_unit;
  return next;
}

function collectChain(
  chain: JournalPayloadChain,
  collectionFenceId: string,
): JournalPayloadChain {
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

function collectGroup(
  group: JournalSubmissionGroup,
  collectionFenceId: string,
): JournalSubmissionGroup {
  return {
    ...group,
    frozen_request_body: { ...group.frozen_request_body, author_edit_units: [] },
    payload_collection: { kind: "collected", collection_fence_id: collectionFenceId },
  };
}

export async function collectEligibleJournalPayload(workspace: EditorWorkspace) {
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
  const [schemaValue, partition, storedFencesValue, activeBaseValue, recordsValue,
    payloadChainsValue, durableGroupsValue] = await Promise.all([
      requestResult(metadata.get("schema")),
      requestResult(transaction.objectStore("partitions").get(partitionId)),
      requestResult(metadata.get(key)),
      requestResult(metadata.get(`active_base:${partitionId}`)),
      requestResult(intents.index("partition").getAll(partitionId)),
      requestResult(chains.index("partition").getAll(partitionId)),
      requestResult(groups.index("partition").getAll(partitionId)),
    ]);
  const schema = schemaValue as { version?: unknown } | undefined;
  const activeBase = activeBaseValue as { value?: unknown } | undefined;
  const storedFences = storedFencesValue as { value?: unknown[] } | undefined;
  const records = recordsValue as JournalIntentRecord[];
  const payloadChains = payloadChainsValue as JournalPayloadChain[];
  const durableGroups = durableGroupsValue as JournalSubmissionGroup[];
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
      && partitionAllowsCollection(workspace, group)) as AppliedSubmissionGroup[];
  for (const group of eligible) {
    const settlement = group.settlement;
    const successor = settlement.installed_base_snapshot;
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
        ? { resulting_writer_generation:
          (workspace.session.writer as Extract<EditorWriterProjection, { kind: "read_only" }>)
            .observed_writer_generation }
        : {}),
      collected_groups: [{
        journal_submission_group_id: group.journal_submission_group_id,
        covered_sequence_range: group.covered_sequence_range,
        payload_digests: group.ordered_coverage.map((item) => item.payload_digest),
        settlement_kind: settlement.kind,
        command_id: settlement.command_id,
        author_command_admission_id: settlement.author_command_admission_id,
        receipt_id: settlement.receipt.receipt_id,
        project_activity_position: settlement.project_activity_position,
      }],
      successor: {
        kind: "authoritative_revision",
        snapshot_id: successor.snapshot_id,
        revision_id: successor.authoritative_head_revision_id,
        materialized_payload_digest: successor.materialized_payload_digest,
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

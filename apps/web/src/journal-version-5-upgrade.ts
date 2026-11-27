import { digestApplyAuthorEdit } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetEditorSessionResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace, JournalPartition, JournalSubmissionGroup } from "./editor-types.ts";
import { uuidV7 } from "./client-identity.ts";
import { requestResult, transactionResult } from "./indexed-db-request.ts";
import { journalHasOpenWork, rebuildPendingProjection } from "./local-edit-journal.ts";
import { earlierJournalKey, type EarlierJournalText } from "./local-recovery-record.ts";

/**
 * The upgrade of a version 5 Local Edit Journal to version 6 (the ADR 0099 rule for a later version, ADR 0107). A
 * version 5 partition identifier has the writer generation after the Editor Session identifier, and the partition row
 * has the writer generation and a writer disposition. The upgrade changes the identifier in each row and removes the
 * writer fields of the partition row. The writer fields of the other rows stay: no reader reads them, and a recovery
 * row stays equal to its group. A group that the client sent keeps its v1 request, because the Server keeps that
 * request and its digest for the outcome Query. A group that the client did not send gets a new v2 freeze after the
 * open.
 */
const V1_AUTHOR_EDIT = "storyos.command.apply-author-edit.request.v1";
const V2_AUTHOR_EDIT = "storyos.command.apply-author-edit.request.v2";
const UNSENT_KEY = "version_5_unsent_groups";
const KEPT_KEY = "version_5_kept_partitions";
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const positiveU64 = (value: unknown): value is string => typeof value === "string" && U64.test(value)
  && BigInt(value) > 0n && BigInt(value) <= 18446744073709551615n;

type Row = Record<string, unknown>;
type Version5Partition = Row & {
  journal_partition_id: string; project_scope: { owner_user_id: string; project_id: string };
  editor_session_id: string; writer_generation: string; client_session_binding_ref: string;
  client_session_generation: string; client_contract_revision: string; security_policy_revision: string;
  limit_profile_revision: string;
};

/** Tells if `group` has the v1 request of before ADR 0107: the v1 form with a positive writer generation. */
export function hasVersion5Request(group: { command_schema?: unknown; frozen_request_body?: unknown }): boolean {
  const request = group.frozen_request_body as { command_schema?: unknown; writer_generation?: unknown } | undefined;
  return group.command_schema === V1_AUTHOR_EDIT && request?.command_schema === V1_AUTHOR_EDIT
    && positiveU64(request.writer_generation);
}

function identifier(partition: Version5Partition, generation: string[]): string {
  return [partition.project_scope.owner_user_id, partition.project_scope.project_id, partition.editor_session_id,
    ...generation, partition.client_session_binding_ref, partition.client_session_generation,
    partition.client_contract_revision, partition.security_policy_revision, partition.limit_profile_revision].join(":");
}

function without(row: Row, fields: readonly string[]): Row {
  return Object.fromEntries(Object.entries(row).filter(([field]) => !fields.includes(field)));
}

/**
 * Changes each row of a version 5 Journal in the version-change `transaction`. For each version 6 identifier, it
 * changes the partition of the highest writer generation that has records or groups. The rows of each other partition
 * stay as they are, and a kept partition with rows gets its text in local recovery after the open. A failure aborts
 * the transaction, and the open fails.
 */
export function upgradeVersion5Journal(transaction: IDBTransaction): void {
  void (async () => {
    const partitions = await requestResult(transaction.objectStore("partitions").getAll()) as Version5Partition[];
    const chosen = new Map<string, { partition: Version5Partition; rows: boolean }>();
    const withRows: Version5Partition[] = [];
    for (const partition of partitions) {
      if (!positiveU64(partition.writer_generation)
        || partition.journal_partition_id !== identifier(partition, [partition.writer_generation])) continue;
      const count = (store: string) => requestResult(transaction.objectStore(store).index("partition")
        .count(partition.journal_partition_id));
      const rows = (await count("intents")) + (await count("submission_groups")) > 0;
      if (rows) withRows.push(partition);
      const next = identifier(partition, []);
      const prior = chosen.get(next);
      if (prior === undefined || (rows && !prior.rows) || (rows === prior.rows
        && BigInt(partition.writer_generation) > BigInt(prior.partition.writer_generation))) {
        chosen.set(next, { partition, rows });
      }
    }
    const changes = [...chosen].map(([next, { partition }]) => [partition.journal_partition_id, next] as const);
    const rename = (row: Row) => JSON.parse(changes.reduce((text, [prior, next]) =>
      text.replaceAll(prior, next), JSON.stringify(row))) as Row;
    const unsent: string[] = [];
    for (const [prior, next] of changes) {
      const store = transaction.objectStore("partitions");
      const partition = await requestResult(store.get(prior)) as Row;
      store.delete(prior);
      store.put({ ...without(partition, ["writer_generation", "disposition"]), journal_partition_id: next });
      for (const name of ["intents", "payload_chains", "submission_groups"]) {
        const rows = transaction.objectStore(name);
        for (const row of await requestResult(rows.index("partition").getAll(prior)) as Row[]) {
          if (name === "intents") rows.delete([prior, row.local_intent_sequence as number]);
          rows.put(rename(row));
          const group = row as Partial<JournalSubmissionGroup>;
          if (name === "submission_groups" && hasVersion5Request(group) && group.settlement?.kind === "unsettled"
            && group.reconciliation === undefined && await requestResult(transaction.objectStore("transport_capsules")
            .index("group").count(group.journal_submission_group_id!)) === 0) unsent.push(group.journal_submission_group_id!);
        }
      }
    }
    const metadata = transaction.objectStore("metadata");
    for (const row of await requestResult(metadata.getAll()) as Row[]) {
      const text = JSON.stringify(row);
      if (!changes.some(([prior]) => text.includes(prior))) continue;
      metadata.delete(row.key as string);
      metadata.put(rename(row));
    }
    if (unsent.length > 0) metadata.put({ key: UNSENT_KEY, value: unsent });
    const kept = withRows.filter((partition) => !changes.some(([prior]) => prior === partition.journal_partition_id));
    if (kept.length > 0) metadata.put({ key: KEPT_KEY, value: kept.map((partition) => partition.journal_partition_id) });
  })().catch(() => {
    try {
      transaction.abort();
    } catch {
      // The transaction ended already.
    }
  });
}

/**
 * Freezes each version 5 group that the client did not send again as v2, after the open: the same coverage and
 * correlation, no writer generation, a new idempotency key, and a new digest. No POST of that group went to the
 * Server, because the client commits a capsule before it sends, so a new key cannot repeat a command.
 */
export async function freezeUnsentVersion5Groups(database: IDBDatabase, cryptoImpl: Crypto): Promise<void> {
  const marker = await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(UNSENT_KEY)) as { value: string[] } | undefined;
  if (marker === undefined) return;
  const read = database.transaction("submission_groups", "readonly").objectStore("submission_groups");
  const groups = await Promise.all(marker.value.map((id) => requestResult(read.get(id)))) as JournalSubmissionGroup[];
  const frozen = await Promise.all(groups.map(async (group) => {
    const { writer_generation: _writer, ...rest } = group.frozen_request_body as { writer_generation?: string };
    const request = { ...rest, command_schema: V2_AUTHOR_EDIT } as JournalSubmissionGroup["frozen_request_body"];
    return { ...group, command_schema: V2_AUTHOR_EDIT, idempotency_key: uuidV7(cryptoImpl),
      frozen_request_body: request, frozen_request_digest: await digestApplyAuthorEdit(request, cryptoImpl) };
  }));
  const transaction = database.transaction(["metadata", "submission_groups", "transport_capsules"], "readwrite");
  const store = transaction.objectStore("submission_groups");
  for (const [index, group] of groups.entries()) {
    const current = await requestResult(store.get(group.journal_submission_group_id));
    const sent = await requestResult(transaction.objectStore("transport_capsules").index("group")
      .count(group.journal_submission_group_id));
    if (JSON.stringify(current) !== JSON.stringify(group) || sent !== 0) {
      transaction.abort();
      throw new Error("A version 5 group changed before its new freeze");
    }
    store.put(frozen[index]);
  }
  transaction.objectStore("metadata").delete(UNSENT_KEY);
  await transactionResult(transaction);
}

/**
 * Keeps the text of each version 5 partition that the upgrade did not change, after the open. When the partition has
 * work that the Server does not have, its last text goes to local recovery, which each Editor Session of the Project
 * shows. Its rows stay as they are. A partition that the client cannot read shows no text, as a corrupt row today.
 */
export async function retainVersion5PartitionText(
  database: IDBDatabase, cryptoImpl: Crypto, maxJsonStringUtf8Bytes: number,
): Promise<void> {
  const marker = await requestResult(database.transaction("metadata", "readonly").objectStore("metadata")
    .get(KEPT_KEY)) as { value: string[] } | undefined;
  if (marker === undefined) return;
  const texts: EarlierJournalText[] = [];
  for (const partitionId of marker.value) {
    const transaction = database.transaction(["partitions", "metadata"], "readonly");
    const [partition, base] = await Promise.all([
      requestResult(transaction.objectStore("partitions").get(partitionId)),
      requestResult(transaction.objectStore("metadata").get(`active_base:${partitionId}`)),
    ]) as [JournalPartition, { value: GetEditorSessionResponse["base_snapshot"] } | undefined];
    const workspace: EditorWorkspace = { database, partition, cryptoImpl, maxJsonStringUtf8Bytes,
      session: { base_snapshot: base?.value } as GetEditorSessionResponse };
    try {
      if (!await journalHasOpenWork(workspace)) continue;
      texts.push({ format_version: 1, disposition: "retained_from_earlier_journal",
        project_scope: partition.project_scope, journal_partition_id: partitionId,
        text: (await rebuildPendingProjection(workspace)).body });
    } catch {
      // The rows stay, and the page shows no text of them.
    }
  }
  const transaction = database.transaction("metadata", "readwrite");
  const metadata = transaction.objectStore("metadata");
  for (const text of texts) metadata.put({ key: earlierJournalKey(text), value: text });
  metadata.delete(KEPT_KEY);
  await transactionResult(transaction);
}

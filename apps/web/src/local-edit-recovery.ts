import type { EditorWorkspace, JournalSnapshot, JournalSubmissionGroup } from "./editor-types.ts";
import { readJournalSnapshot, validateJournalSnapshot } from "./local-edit-journal.ts";
import { MAX_WORKING_JOURNAL_ITEMS } from "./journal-working-set.ts";
import { readRecoveryDispositions, recoveryKey, recoveryPrefix, recoveryMaterial, recoveryRequest as result, RECOVERY_PAGE_SIZE, type LocalRecoveryMaterial } from "./local-recovery-record.ts";
export type { LocalRecoveryMaterial } from "./local-recovery-record.ts";
const equal = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);

/** Keep a terminal local source for manual re-entry without changing its settlement. */
export async function retainLocalRecovery(workspace: EditorWorkspace, groupId: string): Promise<void> {
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const group = snapshot.groups.find((item) => item.journal_submission_group_id === groupId);
  if (workspace.partition.disposition !== "current_writer_open"
    || !group || group.settlement.kind !== "outcome_query_requires_reconfirmation"
    || snapshot.groups.some((item) => item.settlement.kind === "unsettled")
    || snapshot.records.some((record) => !snapshot.covered.has(record.local_intent_sequence))) {
    throw new Error("Only a closed Author Edit can be kept for manual re-entry");
  }
  if (snapshot.localRecovery?.some((item) => item.group.journal_submission_group_id === groupId)) return;
  const material: LocalRecoveryMaterial = { ...recoveryMaterial(snapshot, group), disposition: "retained_for_manual_reentry" };
  const transaction = workspace.database.transaction(["metadata", "partitions", "submission_groups", "intents"], "readwrite", { durability: "strict" });
  const completed = new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Local recovery changed"));
    transaction.onerror = () => reject(transaction.error);
  });
  const [currentGroups, currentRecords, dispositions, partition, base] = await Promise.all([
    result(transaction.objectStore("submission_groups").index("working_partition").getAll(workspace.partition.journal_partition_id, MAX_WORKING_JOURNAL_ITEMS + 1)),
    result(transaction.objectStore("intents").index("working_partition").getAll(workspace.partition.journal_partition_id, MAX_WORKING_JOURNAL_ITEMS + 1)),
    readRecoveryDispositions(transaction, workspace),
    result(transaction.objectStore("partitions").get(workspace.partition.journal_partition_id)),
    result(transaction.objectStore("metadata").get(`active_base:${workspace.partition.journal_partition_id}`)),
  ]);
  const groups = (currentGroups as JournalSubmissionGroup[]).sort((a, b) => a.covered_sequence_range.first - b.covered_sequence_range.first);
  const intents = (currentRecords as JournalSnapshot["records"]).sort((a, b) => a.local_intent_sequence - b.local_intent_sequence);
  if (!equal(partition, workspace.partition) || !equal((base as { value?: unknown } | undefined)?.value, snapshot.activeBase)
    || !equal(groups, snapshot.groups) || !equal(intents, snapshot.records) || !equal(dispositions, snapshot.localRecovery ?? [])) {
    transaction.abort();
    return completed;
  }
  transaction.objectStore("metadata").put({ key: recoveryKey(material), value: material });
  await completed;
}

/** Read retained local material across this Project's Editor Sessions. */
export async function readLocalRecovery(workspace: EditorWorkspace, after?: LocalRecoveryMaterial): Promise<LocalRecoveryMaterial[]> {
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const rows = await result(workspace.database.transaction("metadata").objectStore("metadata").getAll(
    IDBKeyRange.bound(after ? recoveryKey(after) : recoveryPrefix, recoveryPrefix + "\uffff", after !== undefined), RECOVERY_PAGE_SIZE,
  )) as { value: LocalRecoveryMaterial }[];
  const retained = rows.map((row) => row.value);
  if (retained.some((item) => item.format_version !== 1 || item.disposition !== "retained_for_manual_reentry"
    || item.group.settlement.kind !== "outcome_query_requires_reconfirmation"
    || !equal(item.group.project_scope, workspace.partition.project_scope))) throw new Error("Local recovery record is corrupt");
  const pending = after ? [] : snapshot.groups.filter((group) => group.settlement.kind === "outcome_query_requires_reconfirmation"
    && !snapshot.localRecovery?.some((item) => item.group.journal_submission_group_id === group.journal_submission_group_id))
    .map((group) => recoveryMaterial(snapshot, group));
  return [...pending, ...retained];
}

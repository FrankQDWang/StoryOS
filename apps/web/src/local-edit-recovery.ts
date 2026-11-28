import type { EditorWorkspace, JournalSnapshot, JournalSubmissionGroup } from "./editor-types.ts";
import { hasCurrentSubmissionShape, readJournalSnapshot, validateJournalSnapshot } from "./local-edit-journal.ts";
import { MAX_WORKING_JOURNAL_ITEMS } from "./journal-working-set.ts";
import { hasVersion5Request } from "./journal-version-5-upgrade.ts";
import { earlierJournalKey, readRecoveryDispositions, recoveryKey, recoveryPrefix, recoveryMaterial, recoveryRequest as result, RECOVERY_PAGE_SIZE, type EarlierJournalText, type LocalRecoveryMaterial } from "./local-recovery-record.ts";
export type { EarlierJournalText, LocalRecoveryMaterial } from "./local-recovery-record.ts";

type RecoveryItem = LocalRecoveryMaterial | EarlierJournalText;
const itemKey = (item: RecoveryItem) => item.disposition === "retained_from_earlier_journal" ? earlierJournalKey(item) : recoveryKey(item);
const equal = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);

/** Keep a terminal local source for manual re-entry without changing its settlement. */
export async function retainLocalRecovery(workspace: EditorWorkspace, groupId: string): Promise<void> {
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const group = snapshot.groups.find((item) => item.journal_submission_group_id === groupId);
  if (!group || group.settlement.kind !== "outcome_query_requires_reconfirmation"
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
export async function readLocalRecovery(workspace: EditorWorkspace, after?: RecoveryItem): Promise<RecoveryItem[]> {
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const retained: RecoveryItem[] = [];
  for (let cursor = after; retained.length < RECOVERY_PAGE_SIZE;) {
    const limit = RECOVERY_PAGE_SIZE - retained.length;
    const rows = await result(workspace.database.transaction("metadata").objectStore("metadata").getAll(
      IDBKeyRange.bound(cursor ? itemKey(cursor) : recoveryPrefix, recoveryPrefix + "\uffff", cursor !== undefined), limit,
    )) as { value: RecoveryItem }[];
    const page = rows.map((row) => row.value);
    if (page.some((item) => item.format_version !== 1 || (item.disposition === "retained_from_earlier_journal"
      ? typeof item.text !== "string" || !equal(item.project_scope, workspace.partition.project_scope)
      : item.disposition !== "retained_for_manual_reentry"
        || item.group.settlement.kind !== "outcome_query_requires_reconfirmation"
        || !equal(item.group.project_scope, workspace.partition.project_scope)))) {
      throw new Error("Local recovery record is corrupt");
    }
    // A retained record with a shape of a removed path stays in the store, and the page shows no text of it (ADR 0099).
    // The other records of the page stay available.
    retained.push(...page.filter((item) => item.disposition === "retained_from_earlier_journal"
      || hasCurrentSubmissionShape(item.group, item.records)));
    if (page.length < limit) break;
    cursor = page.at(-1);
  }
  const pending = after ? [] : snapshot.groups.filter((group) => group.settlement.kind === "outcome_query_requires_reconfirmation"
    && !snapshot.localRecovery?.some((item) => item.group.journal_submission_group_id === group.journal_submission_group_id))
    .map((group) => recoveryMaterial(snapshot, group));
  // The Server has no Admission of a rejected version 5 group, and the client sends no v1 request again. Its text is
  // in the editor and here, for manual re-entry.
  const rejected = after ? [] : snapshot.groups.filter((group) => hasVersion5Request(group)
    && group.settlement.kind === "outcome_query_rejected_no_admission").map((group): EarlierJournalText => {
    const target = group.frozen_request_body.proposal_target;
    return { format_version: 1, disposition: "retained_from_earlier_journal", project_scope: group.project_scope,
      journal_partition_id: group.journal_partition_id, journal_submission_group_id: group.journal_submission_group_id,
      ...(target ? { proposal_target: { proposal_id: target.proposal_id, operation_id: target.operation_id } } : {}),
      text: snapshot.bodyBySequence.get(group.covered_sequence_range.last)! };
  });
  return [...pending, ...rejected, ...retained];
}

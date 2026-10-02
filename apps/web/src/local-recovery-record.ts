import type { EditorWorkspace, JournalSnapshot, JournalSubmissionGroup, ValidatedJournalSnapshot } from "./editor-types.ts";
import { MAX_WORKING_JOURNAL_ITEMS } from "./journal-working-set.ts";
const equal = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);
export const recoveryRequest = (request: IDBRequest): Promise<unknown> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error);
});

export interface LocalRecoveryMaterial {
  disposition: "awaiting_author_choice" | "retained_for_manual_reentry";
  format_version: 1;
  group: JournalSubmissionGroup;
  records: JournalSnapshot["records"];
  chains: JournalSnapshot["payloadChains"];
  text: string;
  blocks: ValidatedJournalSnapshot["blocksBySequence"] extends Map<number, infer T> ? T : never;
}

export const RECOVERY_PAGE_SIZE = 20;
export const recoveryPrefix = "local_retained_recovery:";
export const recoveryKey = (material: LocalRecoveryMaterial) => recoveryPrefix + material.group.journal_partition_id + ":" + material.group.journal_submission_group_id;

export async function readRecoveryDispositions(transaction: IDBTransaction, workspace: EditorWorkspace): Promise<LocalRecoveryMaterial[]> {
  const prefix = recoveryPrefix + workspace.partition.journal_partition_id + ":";
  const rows = await recoveryRequest(transaction.objectStore("metadata").getAll(
    IDBKeyRange.bound(prefix, prefix + "\uffff"), MAX_WORKING_JOURNAL_ITEMS + 1,
  )) as { value: LocalRecoveryMaterial }[];
  if (rows.length > MAX_WORKING_JOURNAL_ITEMS) throw new Error("Local recovery limit failed");
  return rows.map((row) => row.value);
}

export function validateRecoveryDispositions(snapshot: ValidatedJournalSnapshot): void {
  const seen = new Set<string>();
  for (const material of snapshot.localRecovery ?? []) {
    const group = snapshot.groups.find((item) => item.journal_submission_group_id === material.group?.journal_submission_group_id);
    const records = snapshot.records.filter((record) => group?.ordered_coverage.some((item) => item.local_intent_sequence === record.local_intent_sequence));
    const chains = snapshot.payloadChains.filter((chain) => records.some((record) => record.payload_chain_ref === chain.payload_chain_id));
    const last = group?.covered_sequence_range.last;
    if (material.format_version !== 1 || !group || group.settlement.kind !== "outcome_query_requires_reconfirmation"
      || material.disposition !== "retained_for_manual_reentry" || seen.has(group.journal_submission_group_id)
      || !equal(material.group, group) || !equal(material.records, records) || !equal(material.chains, chains)
      || material.text !== snapshot.bodyBySequence.get(last!) || !equal(material.blocks, snapshot.blocksBySequence.get(last!))) {
      throw new Error("Local recovery record is corrupt");
    }
    seen.add(group.journal_submission_group_id);
  }
}

export function retainedRecoverySequences(snapshot: JournalSnapshot): Set<number> {
  return new Set((snapshot.localRecovery ?? []).flatMap((item) => item.group.ordered_coverage.map((coverage) => coverage.local_intent_sequence)));
}

export function recoveryMaterial(snapshot: ValidatedJournalSnapshot, group: JournalSubmissionGroup): LocalRecoveryMaterial {
  const records = snapshot.records.filter((record) => group.ordered_coverage.some((item) => item.local_intent_sequence === record.local_intent_sequence));
  return {
    format_version: 1, disposition: "awaiting_author_choice", group, records,
    chains: snapshot.payloadChains.filter((chain) => records.some((record) => record.payload_chain_ref === chain.payload_chain_id)),
    text: snapshot.bodyBySequence.get(group.covered_sequence_range.last)!,
    blocks: snapshot.blocksBySequence.get(group.covered_sequence_range.last)!,
  };
}

import type { EditorBaseSnapshot, ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace, JournalIntentRecord, JournalPartition, ValidatedJournalSnapshot } from "./editor-types.ts";
import { JOURNAL_OBJECT_STORES, readJournalSnapshot, validateJournalSnapshot } from "./local-edit-journal.ts";
import { isRemovedDraftGroup, isRemovedDraftRecord } from "./removed-draft-path.ts";

/** The unsent text of a Local Edit Journal that cannot upgrade (contract 3.2). */
export interface LocalUnsentText {
  /** False when the Journal has records that this reader cannot read. The text of each readable partition stays. */
  readable: boolean;
  chapters: Array<{ chapter_id: string; body: string }>;
  proposals: Array<{ proposal_id: string; text: string }>;
}

/**
 * Gives the local text of each payload chain that has a record with no proof of save. Only an applied group and a
 * group that gives a Proposal Revision prove a save. A record and a group of a removed Draft path have no text
 * (ADR 0073). The last record of a chain carries the text of each earlier record of that chain.
 */
function unsavedTexts(snapshot: ValidatedJournalSnapshot) {
  const done = new Set(snapshot.groups.filter((group) => group.settlement.kind === "applied_receipt_settled"
    || (group.settlement.kind === "zero_authority_receipt_settled" && group.settlement.effect.kind === "proposal_revised")
    || isRemovedDraftGroup(group))
    .flatMap((group) => group.ordered_coverage.map((item) => item.local_intent_sequence)));
  for (const record of snapshot.records) if (isRemovedDraftRecord(record)) done.add(record.local_intent_sequence);
  const last = new Map<string, JournalIntentRecord>();
  for (const record of snapshot.records) last.set(record.payload_chain_ref, record);
  const unsaved = new Set(snapshot.records.filter((record) => !done.has(record.local_intent_sequence))
    .map((record) => record.payload_chain_ref));
  return [...unsaved].map((chain) => {
    const record = last.get(chain)!;
    const text = snapshot.bodyBySequence.get(record.local_intent_sequence);
    if (text === undefined) throw new Error("Local Edit Journal text is unavailable");
    return { record, text };
  });
}

const requestResult = <T>(request: IDBRequest<T>): Promise<T> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});

/**
 * Opens the Journal with no version request, so the open does not upgrade it, and reads it in read-only
 * transactions. Gives undefined when the Journal has no records.
 */
export async function readLocalUnsentText(
  indexedDBImpl: IDBFactory,
  scope: ProjectScope,
  cryptoImpl: Crypto,
  maxJsonStringUtf8Bytes: number,
): Promise<LocalUnsentText | undefined> {
  const request = indexedDBImpl.open(`storyos-local-edit-journal:${scope.owner_user_id}:${scope.project_id}`);
  // An upgrade event occurs only when no database exists. Then the reader creates nothing.
  request.onupgradeneeded = () => request.transaction!.abort();
  let database: IDBDatabase;
  try {
    database = await requestResult(request);
  } catch {
    return undefined;
  }
  try {
    if (!database.objectStoreNames.contains("intents")
      || await requestResult(database.transaction("intents").objectStore("intents").count()) === 0) return undefined;
    const unreadable = { readable: false, chapters: [], proposals: [] };
    if (database.version !== 4 || JOURNAL_OBJECT_STORES.some((name) => !database.objectStoreNames.contains(name))) {
      return unreadable;
    }
    const indexed = database.transaction(["intents", "payload_chains", "submission_groups"]);
    if (["intents", "payload_chains", "submission_groups"]
      .some((name) => !indexed.objectStore(name).indexNames.contains("working_partition"))) return unreadable;
    const transaction = database.transaction(["partitions", "metadata"]);
    const partitions = await requestResult(transaction.objectStore("partitions").getAll()) as JournalPartition[];
    const chapters: LocalUnsentText["chapters"] = [];
    const proposals: LocalUnsentText["proposals"] = [];
    let readable = true;
    for (const partition of partitions) {
      const stored = await requestResult(database.transaction("metadata").objectStore("metadata")
        .get(`active_base:${partition.journal_partition_id}`)) as { value?: EditorBaseSnapshot } | undefined;
      if (stored?.value === undefined) {
        readable = false;
        continue;
      }
      const workspace: EditorWorkspace = { database, partition, cryptoImpl, maxJsonStringUtf8Bytes,
        readOnlyJournalVersion: 4,
        session: { schema_id: "storyos.query.editor-session.response.v1", correlation_id: partition.editor_session_id,
          project_scope: partition.project_scope,
          editor_session: { ...partition, opened_at: partition.created_at, disposition: "open" },
          writer: { kind: "current_writer", writer_generation: partition.writer_generation },
          base_snapshot: stored.value } };
      let texts: ReturnType<typeof unsavedTexts>;
      try {
        texts = unsavedTexts(await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace)));
      } catch {
        readable = false;
        continue;
      }
      for (const { record, text } of texts) {
        const target = record.proposal_target;
        if (target !== undefined) {
          if (!proposals.some((item) => item.proposal_id === target.proposal_id && item.text === text)) {
            proposals.push({ proposal_id: target.proposal_id, text });
          }
        } else if (!chapters.some((item) => item.chapter_id === record.chapter_object_id && item.body === text)
          // A Chapter text that is equal to the Server text of the base has nothing to save.
          && !(record.chapter_object_id === stored.value.chapter_id && text === stored.value.materialized_revision.body)) {
          chapters.push({ chapter_id: record.chapter_object_id, body: text });
        }
      }
    }
    return { readable, chapters, proposals };
  } catch {
    return { readable: false, chapters: [], proposals: [] };
  } finally {
    database.close();
  }
}

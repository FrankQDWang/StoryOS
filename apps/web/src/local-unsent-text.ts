import type { EditorBaseSnapshot, ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace, JournalPartition } from "./editor-types.ts";
import {
  JOURNAL_OBJECT_STORES,
  candidateTextsFromSnapshot,
  pendingProjectionFromSnapshot,
  readJournalSnapshot,
  validateJournalSnapshot,
} from "./local-edit-journal.ts";

/** The unsent text of a Local Edit Journal that cannot upgrade (contract 3.2). */
export interface LocalUnsentText {
  /** False when the Journal has records that this reader cannot read. */
  readable: boolean;
  chapters: Array<{ chapter_id: string; body: string }>;
  proposals: Array<{ proposal_id: string; text: string }>;
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
    const chapters = new Map<string, { generation: bigint; body: string }>();
    const proposals = new Map<string, string>();
    for (const partition of partitions) {
      const stored = await requestResult(database.transaction("metadata").objectStore("metadata")
        .get(`active_base:${partition.journal_partition_id}`)) as { value?: EditorBaseSnapshot } | undefined;
      if (stored?.value === undefined) continue;
      const workspace: EditorWorkspace = { database, partition, cryptoImpl, maxJsonStringUtf8Bytes,
        readOnlyJournalVersion: 4,
        session: { schema_id: "storyos.query.editor-session.response.v1", correlation_id: partition.editor_session_id,
          project_scope: partition.project_scope,
          editor_session: { ...partition, opened_at: partition.created_at, disposition: "open" },
          writer: { kind: "current_writer", writer_generation: partition.writer_generation },
          base_snapshot: stored.value } };
      const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
      const projection = pendingProjectionFromSnapshot(workspace, snapshot);
      const generation = BigInt(partition.writer_generation);
      const earlier = chapters.get(stored.value.chapter_id);
      // A Chapter text that is equal to the Server text has nothing to save.
      if (projection.unsettled_intent_count > 0 && projection.body !== stored.value.materialized_revision.body
        && (earlier === undefined || earlier.generation < generation)) {
        chapters.set(stored.value.chapter_id, { generation, body: projection.body });
      }
      for (const [key, value] of candidateTextsFromSnapshot(snapshot, true)) proposals.set(key.split(":")[0]!, value.text);
    }
    return { readable: true,
      chapters: [...chapters].map(([chapter_id, value]) => ({ chapter_id, body: value.body })),
      proposals: [...proposals].map(([proposal_id, text]) => ({ proposal_id, text })) };
  } catch {
    return { readable: false, chapters: [], proposals: [] };
  } finally {
    database.close();
  }
}

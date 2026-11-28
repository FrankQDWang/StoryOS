import { deleteJournal, requestResult, requireRecord } from "./scenario.ts";

/**
 * Rewrite the Journal as an earlier database version. Return the rows that the earlier version keeps. A version 5
 * partition identifier has the writer generation "1" after the Editor Session identifier (before ADR 0107).
 */
export async function restoreJournalVersion(
  database: IDBDatabase, version: 3 | 4 | 5,
): Promise<Record<string, Record<string, unknown>[]>> {
  const name = database.name;
  const transaction = database.transaction([...database.objectStoreNames]);
  const read = await Promise.all([...database.objectStoreNames].map(async (name) => {
    const store = transaction.objectStore(name);
    const indexes = [...store.indexNames].filter((name) => version !== 3 || name !== "working_partition")
      .map((name) => ({ name, keyPath: store.index(name).keyPath, unique: store.index(name).unique }));
    return { name, keyPath: store.keyPath, indexes, rows: await requestResult(store.getAll()) as unknown };
  }));
  const partitionIds = read.filter((store) => store.name === "partitions").flatMap((store) =>
    (store.rows as { journal_partition_id: string }[]).map((row) => row.journal_partition_id));
  const earlierId = (id: string) => { const parts = id.split(":"); return [...parts.slice(0, 3), "1", ...parts.slice(3)]
    .join(":"); };
  const earlierForm = (row: Record<string, unknown>) => version !== 5 ? row : JSON.parse(partitionIds.reduce(
    (text, id) => text.replaceAll(id, earlierId(id)), JSON.stringify(row))) as Record<string, unknown>;
  const stores = read.map(({ name, keyPath, indexes, rows }) => {
    if (!Array.isArray(rows)) throw new Error("Journal rows are unavailable");
    return { name, keyPath, indexes,
      rows: rows.map((row: unknown) => {
        // Versions 4 and 5 keep each row with its field order, because some stored values compare as JSON text.
        const { working_set_partition_id: _working, ...value } = requireRecord(row, "Journal row");
        const kept = version === 3 ? value : earlierForm(requireRecord(row, "Journal row"));
        return name === "metadata" && kept.key === "schema" ? { ...kept, version } : kept;
      }),
    };
  });
  database.close();
  await deleteJournal(name);
  const request = indexedDB.open(name, version);
  request.onupgradeneeded = () => {
    for (const { name, keyPath, indexes, rows } of stores) {
      const store = request.result.createObjectStore(name, { keyPath });
      for (const index of indexes) store.createIndex(index.name, index.keyPath, { unique: index.unique });
      for (const row of rows) store.add(row);
    }
  };
  (await requestResult(request)).close();
  return Object.fromEntries(stores.map(({ name, rows }) => [name, rows]));
}

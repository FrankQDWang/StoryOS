import { deleteJournal, requestResult, requireRecord } from "./scenario.ts";

/** Rewrite the Journal as an earlier database version. Return the rows that the earlier version keeps. */
export async function restoreJournalVersion(
  database: IDBDatabase, version: 3 | 4,
): Promise<Record<string, Record<string, unknown>[]>> {
  const name = database.name;
  const transaction = database.transaction([...database.objectStoreNames]);
  const stores = await Promise.all([...database.objectStoreNames].map(async (name) => {
    const store = transaction.objectStore(name);
    const rows: unknown = await requestResult(store.getAll());
    if (!Array.isArray(rows)) throw new Error("Journal rows are unavailable");
    return { name, keyPath: store.keyPath, indexes: [...store.indexNames]
      .filter((name) => version === 4 || name !== "working_partition")
      .map((name) => ({ name, keyPath: store.index(name).keyPath,
        unique: store.index(name).unique })),
      rows: rows.map((row: unknown) => {
        const { working_set_partition_id: working, ...value } = requireRecord(row, "Journal row");
        const kept = version === 4 && working !== undefined ? { ...value, working_set_partition_id: working } : value;
        return name === "metadata" && kept.key === "schema" ? { ...kept, version } : kept;
      }),
    };
  }));
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

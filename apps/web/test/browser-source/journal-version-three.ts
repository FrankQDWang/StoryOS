import { deleteJournal, requestResult } from "./scenario.ts";

export async function restoreVersionThreeJournal(database: IDBDatabase): Promise<void> {
  const name = database.name;
  const transaction = database.transaction([...database.objectStoreNames]);
  const stores = await Promise.all([...database.objectStoreNames].map(async (name) => {
    const store = transaction.objectStore(name);
    return { name, keyPath: store.keyPath, indexes: [...store.indexNames]
      .filter((name) => name !== "working_partition")
      .map((name) => ({ name, keyPath: store.index(name).keyPath,
        unique: store.index(name).unique })),
      rows: await requestResult(store.getAll()),
    };
  }));
  database.close();
  await deleteJournal(name);
  const request = indexedDB.open(name, 3);
  request.onupgradeneeded = () => {
    for (const { name, keyPath, indexes, rows } of stores) {
      const store = request.result.createObjectStore(name, { keyPath });
      for (const index of indexes) store.createIndex(index.name, index.keyPath, { unique: index.unique });
      for (const { working_set_partition_id: _working, ...row } of rows) {
        store.add(name === "metadata" && row.key === "schema" ? { ...row, version: 3 } : row);
      }
    }
  };
  (await requestResult(request)).close();
}

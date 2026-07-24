import { JOURNAL_SCHEMA } from "../shared/contract.mjs";

const DATABASE_NAME = "storyos-issue-69-editor-journal";
const DATABASE_VERSION = 1;
const ENTRY_STORE = "entries";
const META_STORE = "metadata";

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => resolve(request.result), {
      once: true,
    });
    request.addEventListener("error", () => reject(request.error), {
      once: true,
    });
  });
}

function transactionComplete(transaction) {
  return new Promise((resolve, reject) => {
    transaction.addEventListener("complete", resolve, { once: true });
    transaction.addEventListener("abort", () => reject(transaction.error), {
      once: true,
    });
    transaction.addEventListener("error", () => reject(transaction.error), {
      once: true,
    });
  });
}

export class LocalEditJournal {
  constructor(database) {
    this.database = database;
  }

  static async open() {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.addEventListener("upgradeneeded", () => {
      const database = request.result;
      const entries = database.createObjectStore(ENTRY_STORE, {
        keyPath: "journal_id",
      });
      entries.createIndex("local_order", "local_order", { unique: true });
      entries.createIndex("disposition", "disposition", { unique: false });
      entries.createIndex("chapter_id", "chapter_id", { unique: false });
      database.createObjectStore(META_STORE, { keyPath: "key" });
    });
    const database = await requestResult(request);
    const journal = new LocalEditJournal(database);
    await journal.verify();
    return journal;
  }

  async verify() {
    if (
      !this.database.objectStoreNames.contains(ENTRY_STORE) ||
      !this.database.objectStoreNames.contains(META_STORE)
    ) {
      throw new Error("journal_schema_missing");
    }
    const transaction = this.database.transaction(META_STORE, "readwrite");
    transaction.objectStore(META_STORE).put({
      key: "schema",
      value: JOURNAL_SCHEMA,
    });
    await transactionComplete(transaction);
    return { schema: JOURNAL_SCHEMA, durable: true };
  }

  async put(entry) {
    const transaction = this.database.transaction(ENTRY_STORE, "readwrite", {
      durability: "strict",
    });
    transaction.objectStore(ENTRY_STORE).put(structuredClone(entry));
    await transactionComplete(transaction);
    return structuredClone(entry);
  }

  async append(entry) {
    const transaction = this.database.transaction(
      [ENTRY_STORE, META_STORE],
      "readwrite",
      { durability: "strict" },
    );
    const metadata = transaction.objectStore(META_STORE);
    const counter =
      (await requestResult(metadata.get("next_local_order")))?.value ?? 1;
    const stored = { ...structuredClone(entry), local_order: counter };
    transaction.objectStore(ENTRY_STORE).put(stored);
    metadata.put({ key: "next_local_order", value: counter + 1 });
    await transactionComplete(transaction);
    return stored;
  }

  async update(journalId, patch) {
    const transaction = this.database.transaction(ENTRY_STORE, "readwrite", {
      durability: "strict",
    });
    const store = transaction.objectStore(ENTRY_STORE);
    const current = await requestResult(store.get(journalId));
    if (!current) throw new Error(`journal_entry_missing:${journalId}`);
    const next = { ...current, ...structuredClone(patch) };
    store.put(next);
    await transactionComplete(transaction);
    return structuredClone(next);
  }

  async remove(journalId) {
    const transaction = this.database.transaction(ENTRY_STORE, "readwrite", {
      durability: "strict",
    });
    transaction.objectStore(ENTRY_STORE).delete(journalId);
    await transactionComplete(transaction);
  }

  async get(journalId) {
    const transaction = this.database.transaction(ENTRY_STORE, "readonly");
    return requestResult(transaction.objectStore(ENTRY_STORE).get(journalId));
  }

  async list() {
    const transaction = this.database.transaction(ENTRY_STORE, "readonly");
    const entries = await requestResult(
      transaction.objectStore(ENTRY_STORE).getAll(),
    );
    return entries.sort((left, right) => left.local_order - right.local_order);
  }

  async clear() {
    const transaction = this.database.transaction(ENTRY_STORE, "readwrite", {
      durability: "strict",
    });
    transaction.objectStore(ENTRY_STORE).clear();
    await transactionComplete(transaction);
  }

  async metrics() {
    const entries = await this.list();
    const serialized = JSON.stringify(entries);
    return {
      records: entries.length,
      bytes: new TextEncoder().encode(serialized).byteLength,
      unresolved: entries.filter(
        (entry) =>
          !["gc_completed", "settled_authoritative"].includes(entry.disposition),
      ).length,
    };
  }
}

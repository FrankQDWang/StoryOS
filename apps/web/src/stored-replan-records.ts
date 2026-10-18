import type { EditorWorkspace } from "./editor-types.ts";

const SCHEMA = "storyos.local-edit-journal.replan-proposal.v1";

// The editor sends no Replan (ADR 0064). A Replan entry that an earlier page stored is removed at load, with no request.
export function dropStoredReplans(workspace: EditorWorkspace): Promise<void> {
  return new Promise((resolve, reject) => {
    const transaction = workspace.database.transaction("metadata", "readwrite", { durability: "strict" });
    const cursor = transaction.objectStore("metadata").openCursor(IDBKeyRange.bound("replan:", "replan:￿"));
    cursor.onsuccess = () => {
      const row = cursor.result;
      if (row === null) return;
      if ((row.value as { schema_id?: unknown }).schema_id === SCHEMA) row.delete();
      row.continue();
    };
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(transaction.error ?? new Error("Stored Replan removal failed"));
    transaction.onerror = () => reject(transaction.error ?? new Error("Stored Replan removal failed"));
  });
}

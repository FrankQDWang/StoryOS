import { expect, it } from "vitest";

import { persistReplaceSelection } from "../../src/editor-session.ts";
import { readJournalSnapshot } from "../../src/local-edit-journal.ts";
import {
  FIRST_APPEND_EDIT,
  SECOND_APPEND_EDIT,
  createPausedDigestCrypto,
  openJournalAppendTestWorkspace,
} from "./local-edit-journal-append-fixture.ts";

it("rejects an append when the durable Journal changes after validation", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await persistReplaceSelection(test.workspace, FIRST_APPEND_EDIT);
    const before = await readJournalSnapshot(test.workspace);
    const paused = createPausedDigestCrypto(crypto);
    const append = persistReplaceSelection(
      test.workspace,
      SECOND_APPEND_EDIT,
      paused.cryptoImpl,
    );
    await paused.reached;

    const corruptRecord = structuredClone(before.records[0]);
    if (!corruptRecord) throw new Error("the first Journal intent is unavailable");
    corruptRecord.payload_digest.value_hex_lowercase = "0".repeat(64);
    const transaction = test.workspace.database.transaction("intents", "readwrite");
    transaction.objectStore("intents").put(corruptRecord);
    const changed = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onabort = () => reject(
        transaction.error ?? new Error("IndexedDB transaction aborted"),
      );
      transaction.onerror = () => reject(
        transaction.error ?? new Error("IndexedDB transaction failed"),
      );
    });
    await changed;
    paused.release();

    await expect(append).rejects.toThrow(/changed before append/);
    const after = await readJournalSnapshot(test.workspace);
    expect({
      watermark: after.watermark,
      payloadChains: after.payloadChains,
      groups: after.groups,
      fences: after.fences,
      intentSequences: after.records.map((record) => record.local_intent_sequence),
    }).toEqual({
      watermark: before.watermark,
      payloadChains: before.payloadChains,
      groups: before.groups,
      fences: before.fences,
      intentSequences: [1],
    });
  } finally {
    await test.close();
  }
});

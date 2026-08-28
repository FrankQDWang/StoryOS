import { expect, it } from "vitest";

import { freezeOneIntentSubmission, persistReplaceSelection } from "../../src/editor-session.ts";
import { readJournalSnapshot, rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import {
  FIRST_APPEND_EDIT,
  SECOND_APPEND_EDIT,
  createPausedDigestCrypto,
  openJournalAppendTestWorkspace,
} from "./local-edit-journal-append-fixture.ts";

it.each(["intent_payload", "project_allocator"] as const)(
  "checks partition history during an append after %s changes", async (change) => {
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

    const transaction = test.workspace.database.transaction(["metadata", "intents"], "readwrite");
    if (change === "project_allocator") {
      // Model a different partition's allocation before this append commits.
      // Its sequence must not become a dependency in this partition's history.
      transaction.objectStore("metadata").put({ key: "local_intent_sequence", value: 2 });
    } else {
      const corruptRecord = structuredClone(before.records[0]);
      if (!corruptRecord) throw new Error("the first Journal intent is unavailable");
      corruptRecord.payload_digest.value_hex_lowercase = "0".repeat(64);
      transaction.objectStore("intents").put(corruptRecord);
    }
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

    if (change === "project_allocator") {
      const pending = await append;
      expect(pending).toEqual({ body: "Base!?", save_state: "saving", unsettled_intent_count: 2,
        authoritative_revision_id: test.workspace.session.base_snapshot.authoritative_head_revision_id });
      const appended = await readJournalSnapshot(test.workspace);
      expect(appended.records[0]).toEqual(before.records[0]);
      expect(appended.records.map((record) => ({ sequence: record.local_intent_sequence,
        prior: record.projection_dependency.prior_sequence })))
        .toEqual([{ sequence: 1, prior: 0 }, { sequence: 3, prior: 1 }]);
      const group = await freezeOneIntentSubmission(test.workspace);
      expect(group.ordered_coverage).toEqual(appended.records.map((record) => ({
        local_intent_sequence: record.local_intent_sequence,
        intent_record_ref: record.completed_intent_record_id, payload_digest: record.payload_digest,
      })));
      expect(group.covered_sequence_range).toEqual({ first: 1, last: 3 });
      await expect(freezeOneIntentSubmission(test.workspace)).resolves.toEqual(group);
      await expect(rebuildPendingProjection(test.workspace)).resolves.toEqual(pending);
      return;
    }
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

import { restoreJournalVersion } from "./journal-version-fixture.ts";
import { expect, it, vi } from "vitest";
import { openEditorWorkspace, appendAuthorEdit, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import type { JournalSnapshot } from "../../src/editor-types.ts";
import { createJournalUuid, readJournalSnapshot } from "../../src/local-edit-journal.ts";
import { appliedAuthorEditTransport } from "./journal-working-set-transport.ts";
import {
  OWNER, PROJECT, createBrowserScenario, deleteJournal, requireEditorReady, requestResult,
} from "./scenario.ts";

it("continues writing after 2400 saved and collected input intents", async () => {
  const scenario = createBrowserScenario();
  let body = "Base";
  let firstUncollected: JournalSnapshot | undefined;
  const transport = appliedAuthorEditTransport(scenario, () => body);
  const { fetchImpl } = transport;
  await deleteJournal(scenario.journalName);
  let workspace = await openEditorWorkspace({ baseUrl: location.origin,
    project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  try {
    for (let batch = 0; batch < 10; batch++) {
      const undoGroupId = createJournalUuid();
      for (let offset = 0; offset < 240; offset++) {
        const from = body.length;
        body += "a";
        await appendAuthorEdit(workspace, { from, to: from, text: "a",
          resultingBody: body, inputOrigin: "typing", undoGroupId,
          createdAt: new Date(Date.UTC(2026, 7, 15, 8) + batch * 10000 + offset).toISOString(),
        });
      }
      expect(await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
        fetchImpl, cryptoImpl: crypto })).toMatchObject({ body, save_state: "saved",
        unsettled_intent_count: 0 });
      if (batch === 0) firstUncollected = await readJournalSnapshot(workspace);
      await collectEligibleJournalPayload(workspace);
    }
    const saved = await readJournalSnapshot(workspace);
    expect(saved.records).toHaveLength(2400);
    expect(saved.records.every((record) => record.author_edit_unit === undefined)).toBe(true);
    expect(saved.groups.every((group) => group.payload_collection?.kind === "collected")).toBe(true);
    await restoreJournalVersion(workspace.database, 3);
    const putBeforeUpgrade = IDBObjectStore.prototype.put;
    const failedUpgrade = vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
      this: IDBObjectStore, value: unknown, key?: IDBValidKey,
    ) {
      const request = key === undefined ? putBeforeUpgrade.call(this, value) : putBeforeUpgrade.call(this, value, key);
      if (this.name === "metadata" && value !== null && typeof value === "object"
        && Reflect.get(value, "key") === "schema" && Reflect.get(value, "version") === 5) this.transaction.abort();
      return request;
    });
    try {
      expect(await openEditorWorkspace({ baseUrl: location.origin,
        project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
        fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto }))
        .toMatchObject({ kind: "editor-read-only-recovery", code: "local_journal_unavailable" });
    } finally { failedUpgrade.mockRestore(); }
    const prior = await requestResult(indexedDB.open(scenario.journalName));
    expect(prior.version).toBe(3);
    expect(prior.transaction("intents").objectStore("intents").indexNames.contains("working_partition")).toBe(false);
    prior.close();
    workspace = await openEditorWorkspace({ baseUrl: location.origin,
      project: scenario.project, chapter: { ...scenario.chapter,
        project_activity_position: transport.session().base_snapshot.project_activity_position,
        chapter: { ...scenario.chapter.chapter,
          current_revision: transport.session().base_snapshot.materialized_revision },
      }, profile: scenario.profile,
      fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    requireEditorReady(workspace);
    expect(await readJournalSnapshot(workspace)).toEqual(saved);
    const partitionId = workspace.partition.journal_partition_id;
    const ready = workspace;
    async function replacePrefix(prefix: JournalSnapshot) {
      const transaction = ready.database.transaction(["intents", "payload_chains", "submission_groups"], "readwrite");
      const completed = new Promise<void>((resolve, reject) => {
        transaction.oncomplete = () => resolve();
        transaction.onabort = () => reject(transaction.error);
      });
      for (const record of prefix.records.slice(0, 240)) transaction.objectStore("intents").put(record);
      transaction.objectStore("payload_chains").put(prefix.payloadChains[0]!);
      transaction.objectStore("submission_groups").put(prefix.groups[0]!);
      await completed;
    }
    async function refusesFullWorkingSet() {
      const before = await readJournalSnapshot(ready);
      await expect(appendAuthorEdit(ready, { from: body.length, to: body.length,
        text: "+", resultingBody: `${body}+`, inputOrigin: "typing" })).rejects.toThrow();
      expect(await readJournalSnapshot(ready)).toEqual(before);
    }
    if (!firstUncollected) throw new Error("the first retained payload is unavailable");
    await replacePrefix(firstUncollected);
    await refusesFullWorkingSet();
    await replacePrefix(saved);
    const chain = saved.payloadChains[0]!;
    await replacePrefix({ ...saved, payloadChains: [{ ...chain,
      payload_collection: saved.payloadChains[1]!.payload_collection!,
    }] });
    await refusesFullWorkingSet();
    await replacePrefix(saved);
    const firstFence = saved.fences[0] as Record<string, unknown>;
    const { partition_disposition: _disposition, ...incompleteFence } = firstFence;
    const invalidFenceWrite = ready.database.transaction("metadata", "readwrite");
    await requestResult(invalidFenceWrite.objectStore("metadata").put({
      key: `collection_fences:${partitionId}`, value: [incompleteFence, ...saved.fences.slice(1)],
    }));
    await refusesFullWorkingSet();
    await requestResult(ready.database.transaction("metadata", "readwrite").objectStore("metadata")
      .put({ key: `collection_fences:${partitionId}`, value: saved.fences }));
    const put = IDBObjectStore.prototype.put;
    const interrupted = vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
      this: IDBObjectStore, value: unknown, key?: IDBValidKey,
    ) {
      if (this.name === "metadata" && value !== null && typeof value === "object"
        && Reflect.get(value, "key") === `working_boundary:${partitionId}`) {
        this.transaction.abort();
      }
      return key === undefined ? put.call(this, value) : put.call(this, value, key);
    });
    try {
      await expect(appendAuthorEdit(workspace, { from: body.length, to: body.length,
        text: "+", resultingBody: `${body}+`, inputOrigin: "typing" })).rejects.toThrow();
    } finally { interrupted.mockRestore(); }
    expect(await readJournalSnapshot(workspace)).toEqual(saved);
    await appendAuthorEdit(workspace, { from: body.length, to: body.length,
      text: "+", resultingBody: `${body}+`, inputOrigin: "typing" });
    const continued = await readJournalSnapshot(workspace);
    expect(continued.records).toHaveLength(1);
    expect(continued.workingBoundary?.last_sequence).toBe(2400);
    const retained = workspace.database.transaction(["intents", "submission_groups", "metadata"]);
    const [history, groups, retainedFences] = await Promise.all([
      requestResult(retained.objectStore("intents").index("partition").getAll(workspace.partition.journal_partition_id)),
      requestResult(retained.objectStore("submission_groups").index("partition").getAll(workspace.partition.journal_partition_id)),
      Promise.all(saved.fences.map((fence) => requestResult(retained.objectStore("metadata")
        .get(`retained_collection_fence:${Reflect.get(fence as object, "collection_fence_id")}`)))),
    ]);
    const withoutIndex = ({ working_set_partition_id: _index, ...value }: Record<string, unknown>) => value;
    expect(history.slice(0, 2400)).toEqual(saved.records.map(withoutIndex));
    expect(groups).toEqual(saved.groups.map(withoutIndex));
    expect(retainedFences.map((entry) => entry.value)).toEqual(saved.fences);
  } finally {
    if (workspace.kind === "editor-ready") workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 180000);

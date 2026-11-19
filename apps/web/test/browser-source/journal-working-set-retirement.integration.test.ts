import { expect, it } from "vitest";
import { openEditorWorkspace, appendAuthorEdit, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import { createJournalUuid, readJournalSnapshot, rebuildPendingProjection }
  from "../../src/local-edit-journal.ts";
import { appliedAuthorEditTransport } from "./journal-working-set-transport.ts";
import {
  OWNER, PROJECT, createBrowserScenario, deleteJournal, requireEditorReady, requestResult,
} from "./scenario.ts";

it("retires settled single-input groups up to the first uncollected group", async () => {
  const scenario = createBrowserScenario();
  let body = "Base";
  const { fetchImpl } = appliedAuthorEditTransport(scenario, () => body);
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({ baseUrl: location.origin,
    project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  // Each append revalidates all groups, so 120 single-input groups and 10 batches keep the run short.
  const sizes = [...Array.from({ length: 120 }, () => 1), ...Array.from({ length: 9 }, () => 240), 120];
  const uncollected = 60;
  try {
    for (const [group, size] of sizes.entries()) {
      const undoGroupId = createJournalUuid();
      for (let offset = 0; offset < size; offset++) {
        const from = body.length;
        body += "a";
        await appendAuthorEdit(workspace, { from, to: from, text: "a", resultingBody: body,
          inputOrigin: "typing", undoGroupId,
          createdAt: new Date(Date.UTC(2026, 7, 15, 8) + group * 10000 + offset).toISOString(),
        });
      }
      await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl,
        cryptoImpl: crypto });
      if (group !== uncollected) await collectEligibleJournalPayload(workspace);
    }
    const saved = await readJournalSnapshot(workspace);
    expect(saved.groups.map((group) => [group.ordered_coverage.length,
      group.payload_collection?.kind === "collected"]))
      .toEqual(sizes.map((size, index) => [size, index !== uncollected]));
    expect(saved.records).toHaveLength(2400);

    const projection = await appendAuthorEdit(workspace, { from: body.length, to: body.length,
      text: "+", resultingBody: `${body}+`, inputOrigin: "typing" });
    expect(projection).toMatchObject({ body: `${body}+`, unsettled_intent_count: 1 });
    expect(await rebuildPendingProjection(workspace)).toEqual(projection);
    const continued = await readJournalSnapshot(workspace);
    const retiredChains = new Set(saved.records.slice(0, uncollected)
      .map((record) => record.payload_chain_ref));
    expect(continued.workingBoundary?.last_sequence)
      .toBe(saved.records[uncollected - 1]!.local_intent_sequence);
    expect(continued.records.slice(0, -1)).toEqual(saved.records.slice(uncollected));
    expect(continued.payloadChains.slice(0, -1)).toEqual(saved.payloadChains
      .filter((chain) => !retiredChains.has(chain.payload_chain_id)));
    expect(continued.groups).toEqual(saved.groups.slice(uncollected));
    expect(continued.fences).toEqual(saved.fences.slice(uncollected));
    const retained = workspace.database.transaction(
      ["intents", "payload_chains", "submission_groups", "metadata"]);
    const partitionId = workspace.partition.journal_partition_id;
    const history = async (store: string) => (await requestResult(retained.objectStore(store)
      .index("partition").getAll(partitionId)) as Record<string, unknown>[])
      .filter((row) => row.working_set_partition_id === undefined);
    const [records, chains, groups, retainedFences] = await Promise.all([
      history("intents"), history("payload_chains"), history("submission_groups"),
      Promise.all(saved.fences.slice(0, uncollected).map((fence) => requestResult(retained
        .objectStore("metadata").get(`retained_collection_fence:${
          Reflect.get(fence as object, "collection_fence_id")}`)))),
    ]);
    const withoutIndex = ({ working_set_partition_id: _index, ...value }: Record<string, unknown>) => value;
    const byKey = (key: string) => (rows: Record<string, unknown>[]) =>
      [...rows].sort((left, right) => String(left[key]).localeCompare(String(right[key])));
    expect(records).toEqual(saved.records.slice(0, uncollected).map(withoutIndex));
    expect(byKey("payload_chain_id")(chains)).toEqual(byKey("payload_chain_id")(saved.payloadChains
      .filter((chain) => retiredChains.has(chain.payload_chain_id)).map(withoutIndex)));
    expect(byKey("journal_submission_group_id")(groups)).toEqual(
      byKey("journal_submission_group_id")(saved.groups.slice(0, uncollected).map(withoutIndex)));
    expect(retainedFences.map((entry) => entry.value)).toEqual(saved.fences.slice(0, uncollected));
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 180000);

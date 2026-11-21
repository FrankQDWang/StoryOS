import { expect, it, vi } from "vitest";
import type { DigestValue, GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openEditorWorkspace, appendAuthorEdit, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import type { EditorState, EditorWorkspace, JournalSnapshot } from "../../src/editor-types.ts";
import { uuidV7 as createJournalUuid } from "../../src/client-identity.ts";
import { readJournalSnapshot, rebuildPendingProjection }
  from "../../src/local-edit-journal.ts";
import {
  OWNER, PROJECT, SESSION, chapterRevision, createAppliedAuthorEditResponse,
  createBrowserScenario, deleteJournal, jsonResponse, requestHeaders,
  requireDigestValue, requireEditorReady, requireRequestBody, requestResult,
} from "./scenario.ts";

function appliedAuthorEditTransport(scenario: ReturnType<typeof createBrowserScenario>,
  currentBody: () => string) {
  let position = 0;
  let commandDigest: DigestValue | undefined;
  let session: GetEditorSessionResponse = {
    ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      commandDigest = requireDigestValue(
        requireRequestBody(init).canonical_command_digest, "command digest",
      );
      return jsonResponse({ nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(session);
    if (path.endsWith("/manuscript/author-edits")) {
      const body = currentBody();
      const request = requireRequestBody(init);
      const idempotencyKey = requestHeaders(init).get("idempotency-key");
      if (!commandDigest || !idempotencyKey) throw new Error("missing admitted identity");
      const revisionId = createJournalUuid();
      const response = createAppliedAuthorEditResponse({ request, commandDigest,
        idempotencyKey, body, projectActivityPosition: String(++position),
        priorRevisionId: session.base_snapshot.authoritative_head_revision_id,
        authoritativeRevisionId: revisionId, commandId: createJournalUuid(),
        authorCommandAdmissionId: createJournalUuid(), receiptId: createJournalUuid(),
        authoritativeCommitId: createJournalUuid(),
      });
      const digest = new Uint8Array(await crypto.subtle.digest(
        "SHA-256", new TextEncoder().encode(body),
      ));
      session = { ...session, base_snapshot: { ...session.base_snapshot,
        snapshot_id: createJournalUuid(), project_activity_position: String(position),
        authoritative_head_revision_id: revisionId,
        materialized_revision: chapterRevision(revisionId, body),
        materialized_payload_digest: { algorithm: "sha256",
          profile: "storyos.canonical-payload.sha256.v1",
          value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
        },
      } };
      return jsonResponse(response);
    }
    throw new Error(`unexpected request ${path}`);
  };
  return { fetchImpl, session: () => session };
}

type Row = Record<string, unknown>;
const withoutIndex = ({ working_set_partition_id: _index, ...value }: Row) => value;
const byKey = (key: string) => (rows: readonly Row[]) =>
  [...rows].sort((left, right) => String(left[key]).localeCompare(String(right[key])));
const fenceId = (fence: unknown) => String(Reflect.get(fence as object, "collection_fence_id"));

/** Keeps the latest content of each row of `snapshot`, so a test can compare a row after it leaves the working set. */
function remember(seen: ReturnType<typeof emptyRows>, snapshot: JournalSnapshot) {
  for (const record of snapshot.records) seen.records.set(String(record.local_intent_sequence), record);
  for (const chain of snapshot.payloadChains) seen.chains.set(chain.payload_chain_id, chain);
  for (const group of snapshot.groups) seen.groups.set(group.journal_submission_group_id, group);
  for (const fence of snapshot.fences) seen.fences.set(fenceId(fence), fence);
}

function emptyRows() {
  return { records: new Map<string, unknown>(), chains: new Map<string, unknown>(),
    groups: new Map<string, unknown>(), fences: new Map<string, unknown>() };
}

/** The rows of the partition that left the working set, and each retained collection fence. */
async function retainedHistory(workspace: EditorWorkspace) {
  const transaction = workspace.database.transaction(
    ["intents", "payload_chains", "submission_groups", "metadata"]);
  const partitionId = workspace.partition.journal_partition_id;
  const history = async (store: string) => (await requestResult(transaction.objectStore(store)
    .index("partition").getAll(partitionId)) as Row[]).filter((row) => row.working_set_partition_id === undefined);
  const [records, chains, groups, fences] = await Promise.all([
    history("intents"), history("payload_chains"), history("submission_groups"),
    requestResult(transaction.objectStore("metadata").getAll(IDBKeyRange.bound(
      "retained_collection_fence:", "retained_collection_fence:￿"))) as Promise<{ value: unknown }[]>,
  ]);
  return { records, chains: byKey("payload_chain_id")(chains),
    groups: byKey("journal_submission_group_id")(groups), fences: fences.map((entry) => entry.value) };
}

/** The retained history that the remembered rows of `ids` give. */
function expectedHistory(seen: ReturnType<typeof emptyRows>, ids: {
  records: readonly string[]; chains: readonly string[]; groups: readonly string[]; fences: readonly string[];
}) {
  const pick = (rows: Map<string, unknown>, keys: readonly string[]) => keys.map((key) => rows.get(key) as Row);
  return { records: pick(seen.records, ids.records).map(withoutIndex),
    chains: byKey("payload_chain_id")(pick(seen.chains, ids.chains).map(withoutIndex)),
    groups: byKey("journal_submission_group_id")(pick(seen.groups, ids.groups).map(withoutIndex)),
    fences: [...ids.fences].sort().map((id) => seen.fences.get(id)) };
}

async function openWorkspace(scenario: ReturnType<typeof createBrowserScenario>, fetchImpl: typeof fetch) {
  const workspace = await openEditorWorkspace({ baseUrl: location.origin,
    project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  return workspace;
}

/** Types `count` inputs of one undo group, and gives the working set after the first input. */
async function typeRun(workspace: EditorWorkspace, text: { body: string }, run: number, count: number) {
  const undoGroupId = createJournalUuid();
  let afterFirst: JournalSnapshot | undefined;
  for (let offset = 0; offset < count; offset++) {
    const from = text.body.length;
    text.body += "a";
    await appendAuthorEdit(workspace, { from, to: from, text: "a",
      resultingBody: text.body, inputOrigin: "typing", undoGroupId,
      createdAt: new Date(Date.UTC(2026, 7, 15, 8) + run * 10000 + offset).toISOString(),
    });
    if (offset === 0) afterFirst = await readJournalSnapshot(workspace);
  }
  return afterFirst!;
}

it("retires each saved and collected group before the next input", async () => {
  const scenario = createBrowserScenario();
  const text = { body: "Base" };
  const { fetchImpl } = appliedAuthorEditTransport(scenario, () => text.body);
  await deleteJournal(scenario.journalName);
  const workspace = await openWorkspace(scenario, fetchImpl);
  const seen = emptyRows();
  const afterFirstInputs: [number, number | undefined][] = [];
  try {
    for (let batch = 0; batch < 10; batch++) {
      const afterFirst = await typeRun(workspace, text, batch, 240);
      if (batch > 0) afterFirstInputs.push([afterFirst.records.length, afterFirst.workingBoundary?.last_sequence]);
      expect(await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
        fetchImpl, cryptoImpl: crypto })).toMatchObject({ body: text.body, save_state: "saved",
        unsettled_intent_count: 0 });
      await collectEligibleJournalPayload(workspace);
      remember(seen, await readJournalSnapshot(workspace));
    }
    expect(afterFirstInputs).toEqual(Array.from({ length: 9 }, (_value, index) => [1, 240 * (index + 1)]));
    const collected = await readJournalSnapshot(workspace);
    expect([collected.records.length, collected.workingBoundary?.last_sequence]).toEqual([240, 2160]);

    await appendAuthorEdit(workspace, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" });
    const continued = await readJournalSnapshot(workspace);
    expect([continued.records.length, continued.workingBoundary?.last_sequence]).toEqual([1, 2400]);
    expect(seen.records.size).toBe(2400);
    expect(await retainedHistory(workspace)).toEqual(expectedHistory(seen, {
      records: [...seen.records.keys()], chains: [...seen.chains.keys()],
      groups: [...seen.groups.keys()], fences: [...seen.fences.keys()],
    }));
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 360000);

/** Replaces the rows of the first 240 records of `ready` with the rows of `prefix`. */
async function replacePrefix(ready: EditorWorkspace, prefix: JournalSnapshot) {
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

it("refuses input when a full working set cannot retire its prefix", async () => {
  const scenario = createBrowserScenario();
  const text = { body: "Base" };
  let firstUncollected: JournalSnapshot | undefined;
  let firstCollected: JournalSnapshot | undefined;
  const transport = appliedAuthorEditTransport(scenario, () => text.body);
  const { fetchImpl } = transport;
  await deleteJournal(scenario.journalName);
  let workspace: EditorState = await openWorkspace(scenario, fetchImpl);
  /** Refuses the next input on `target`, and keeps its working set. */
  async function refusesInput(target: EditorWorkspace) {
    const before = await readJournalSnapshot(target);
    expect(before.records).toHaveLength(2400);
    await expect(appendAuthorEdit(target, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" })).rejects.toThrow();
    expect(await readJournalSnapshot(target)).toEqual(before);
  }
  try {
    for (let batch = 0; batch < 10; batch++) {
      await typeRun(workspace, text, batch, 240);
      expect(await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin,
        fetchImpl, cryptoImpl: crypto })).toMatchObject({ body: text.body, save_state: "saved",
        unsettled_intent_count: 0 });
      if (batch === 0) firstUncollected = await readJournalSnapshot(workspace);
      await collectEligibleJournalPayload(workspace);
      if (batch === 0) {
        firstCollected = await readJournalSnapshot(workspace);
        // A first group that is not collected stops the prefix, so the working set fills to its limit.
        await replacePrefix(workspace, firstUncollected!);
      }
    }
    await refusesInput(workspace);
    await replacePrefix(workspace, firstCollected!);
    const saved = await readJournalSnapshot(workspace);
    expect(saved.records).toHaveLength(2400);
    expect(saved.records.every((record) => record.author_edit_unit === undefined)).toBe(true);
    expect(saved.groups.every((group) => group.payload_collection?.kind === "collected")).toBe(true);
    workspace.database.close();
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
    const reopened = workspace;
    const chain = saved.payloadChains[0]!;
    await replacePrefix(reopened, { ...saved, payloadChains: [{ ...chain,
      payload_collection: saved.payloadChains[1]!.payload_collection!,
    }] });
    await refusesInput(reopened);
    await replacePrefix(reopened, saved);
    const firstFence = saved.fences[0] as Record<string, unknown>;
    const { partition_disposition: _disposition, ...incompleteFence } = firstFence;
    const invalidFenceWrite = reopened.database.transaction("metadata", "readwrite");
    await requestResult(invalidFenceWrite.objectStore("metadata").put({
      key: `collection_fences:${partitionId}`, value: [incompleteFence, ...saved.fences.slice(1)],
    }));
    await refusesInput(reopened);
    await requestResult(reopened.database.transaction("metadata", "readwrite").objectStore("metadata")
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
      await expect(appendAuthorEdit(reopened, { from: text.body.length, to: text.body.length,
        text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" })).rejects.toThrow();
    } finally { interrupted.mockRestore(); }
    expect(await readJournalSnapshot(reopened)).toEqual(saved);
    await appendAuthorEdit(reopened, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" });
    const continued = await readJournalSnapshot(reopened);
    expect([continued.records.length, continued.workingBoundary?.last_sequence]).toEqual([1, 2400]);
  } finally {
    if (workspace.kind === "editor-ready") workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 360000);

it("retires settled single-input groups up to the first uncollected group", async () => {
  const scenario = createBrowserScenario();
  const text = { body: "Base" };
  const { fetchImpl } = appliedAuthorEditTransport(scenario, () => text.body);
  await deleteJournal(scenario.journalName);
  const workspace = await openWorkspace(scenario, fetchImpl);
  const sizes = [...Array.from({ length: 120 }, () => 1), ...Array.from({ length: 9 }, () => 240), 120];
  const uncollected = 60;
  const seen = emptyRows();
  try {
    for (const [group, size] of sizes.entries()) {
      await typeRun(workspace, text, group, size);
      await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl,
        cryptoImpl: crypto });
      if (group !== uncollected) await collectEligibleJournalPayload(workspace);
      if (group < uncollected) remember(seen, await readJournalSnapshot(workspace));
    }
    const saved = await readJournalSnapshot(workspace);
    // The groups after the uncollected group stay, also when they are collected.
    expect(saved.groups.map((group) => [group.ordered_coverage.length,
      group.payload_collection?.kind === "collected"]))
      .toEqual(sizes.slice(uncollected).map((size, index) => [size, index !== 0]));
    expect([saved.records.length, saved.workingBoundary?.last_sequence]).toEqual([2400 - uncollected, uncollected]);

    const projection = await appendAuthorEdit(workspace, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" });
    expect(projection).toMatchObject({ body: `${text.body}+`, unsettled_intent_count: 1 });
    expect(await rebuildPendingProjection(workspace)).toEqual(projection);
    const continued = await readJournalSnapshot(workspace);
    const workingSet = (snapshot: JournalSnapshot) => ({ records: snapshot.records,
      payloadChains: snapshot.payloadChains, groups: snapshot.groups, fences: snapshot.fences,
      workingBoundary: snapshot.workingBoundary });
    expect(workingSet({ ...continued, records: continued.records.slice(0, -1),
      payloadChains: continued.payloadChains.slice(0, -1) })).toEqual(workingSet(saved));
    const retiredGroups = [...seen.groups.keys()];
    expect(retiredGroups).toHaveLength(uncollected);
    expect(await retainedHistory(workspace)).toEqual(expectedHistory(seen, {
      records: [...seen.records.keys()], chains: [...seen.chains.keys()],
      groups: retiredGroups, fences: [...seen.fences.keys()],
    }));
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 360000);

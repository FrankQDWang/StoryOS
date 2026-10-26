import { restoreJournalVersion } from "./journal-version-fixture.ts";
import { expect, it, vi } from "vitest";
import { digestApplyAuthorEdit, type DigestValue, type GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { freezeOneIntentSubmission } from "../../src/author-edit-submission.ts";
import { openEditorWorkspace, appendAuthorEdit, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import type { JournalIntentRecord, JournalSnapshot, JournalSubmissionGroup } from "../../src/editor-types.ts";
import { createJournalUuid, readJournalSnapshot, rebuildPendingProjection }
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

it("retires a stored record and group of a removed Draft path with the collected prefix", async () => {
  const scenario = createBrowserScenario();
  let body = "Base";
  const { fetchImpl } = appliedAuthorEditTransport(scenario, () => body);
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({ baseUrl: location.origin,
    project: scenario.project, chapter: scenario.chapter, profile: scenario.profile,
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  const retrySource = { kind: "draft_retry", source_draft_kind: "refused_edit",
    source_draft_id: "018f0000-0000-7001-8000-000000000090",
    source_current_draft_revision_id: "018f0000-0000-7001-8000-000000000090",
    source_draft_payload_digest: "a".repeat(64), expected_source_draft_closure: "open",
    selected_payload_range: { kind: "exact_structured_range",
      coordinate_profile: "storyos.draft-replacement.block-utf16.v1", from: { block_index: 0, offset: 0 },
      to: { block_index: 0, offset: 1 }, slice_digest: "b".repeat(64) } } as const;
  let createdAt = Date.UTC(2026, 7, 15, 8);
  let undoGroupId = createJournalUuid();
  const append = (text: string) => appendAuthorEdit(workspace, { from: body.length, to: body.length, text,
    resultingBody: body + text, inputOrigin: "typing", undoGroupId, createdAt: new Date(createdAt++).toISOString() });
  // An earlier client stored a Draft retry. The Server never applied its text, so the body stays.
  const storeDraftRetry = async (group?: JournalSubmissionGroup) => {
    const transaction = workspace.database.transaction(["intents", "submission_groups"], "readwrite");
    const records = await requestResult(transaction.objectStore("intents").getAll()) as JournalIntentRecord[];
    transaction.objectStore("intents").put({ ...records.at(-1)!, retry_source: retrySource });
    if (group) {
      const request = { ...group.frozen_request_body,
        retry_source: retrySource } as JournalSubmissionGroup["frozen_request_body"];
      transaction.objectStore("submission_groups").put({ ...group, frozen_request_body: request,
        frozen_request_digest: await digestApplyAuthorEdit(request, crypto) });
    }
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onabort = () => reject(transaction.error);
    });
  };
  try {
    await append("x");
    await storeDraftRetry();
    for (const [index, size] of [238, 0, ...Array.from({ length: 9 }, () => 240)].entries()) {
      undoGroupId = createJournalUuid();
      if (index === 1) {
        await append("x");
        await storeDraftRetry(await freezeOneIntentSubmission(workspace));
        continue;
      }
      for (let offset = 0; offset < size; offset++) {
        await append("a");
        body += "a";
      }
      await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto });
      await collectEligibleJournalPayload(workspace);
    }
    const saved = await readJournalSnapshot(workspace);
    expect(saved.records).toHaveLength(2400);
    expect(saved.groups).toHaveLength(11);

    expect(await append("+")).toMatchObject({ body: `${body}+`, unsettled_intent_count: 1 });
    const continued = await readJournalSnapshot(workspace);
    expect(continued.workingBoundary?.last_sequence).toBe(2400);
    expect(continued.records).toHaveLength(1);
    expect(continued.payloadChains).toHaveLength(1);
    expect(continued.groups).toEqual([]);
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 180000);

import { expect, it, vi } from "vitest";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue, GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openEditorWorkspace, appendAuthorEdit, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import { collectEligibleJournalPayload } from "../../src/journal-payload-collection.ts";
import type { EditorState, EditorWorkspace, JournalSnapshot } from "../../src/editor-types.ts";
import { uuidV7 as createJournalUuid } from "../../src/client-identity.ts";
import { readJournalSnapshot, rebuildPendingProjection }
  from "../../src/local-edit-journal.ts";
import type { CandidateSelectionEdit } from "../../src/local-edit-journal.ts";
import { retainLocalRecovery } from "../../src/local-edit-recovery.ts";
import { recoveryPrefix } from "../../src/local-recovery-record.ts";
import {
  BLOCK, OWNER, PROJECT, SESSION, chapterRevision, createAppliedAuthorEditResponse,
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

/** Writes the collection fences of the partition of `ready`. A fence names only a collected group. */
async function writeFences(ready: EditorWorkspace, fences: readonly unknown[]) {
  await requestResult(ready.database.transaction("metadata", "readwrite").objectStore("metadata")
    .put({ key: `collection_fences:${ready.partition.journal_partition_id}`, value: fences }));
}

/** `prefix` with the settlement of its first group changed by `settle`. */
function withFirstSettlement(prefix: JournalSnapshot,
  settle: (applied: Record<string, unknown>) => Record<string, unknown>): JournalSnapshot {
  const [first, ...rest] = prefix.groups;
  return { ...prefix, groups: [{ ...first!, settlement: settle(first!.settlement) as never }, ...rest] };
}

const reconfirmation = (applied: Record<string, unknown>) => ({ kind: "outcome_query_requires_reconfirmation",
  command_id: applied.command_id, author_command_admission_id: applied.author_command_admission_id,
  reconfirmation_reason: "direct_edit_intent_unrecoverable" });
const refused = (applied: Record<string, unknown>) => {
  const receipt = applied.receipt as Record<string, unknown>;
  return { kind: "zero_authority_receipt_settled", command_id: applied.command_id,
    author_command_admission_id: applied.author_command_admission_id,
    receipt: { ...receipt, result: "refused", resulting_heads: receipt.prior_heads, authoritative_revision_ids: [],
      authoritative_commit_ids: [], author_action_sequence: null },
    effect: { kind: "refused", reason: "target_mismatch" } };
};

it("refuses input when a full working set cannot retire its prefix", async () => {
  const scenario = createBrowserScenario();
  const text = { body: "Base" };
  let firstUncollected: JournalSnapshot | undefined;
  let firstCollected: JournalSnapshot | undefined;
  const transport = appliedAuthorEditTransport(scenario, () => text.body);
  const { fetchImpl } = transport;
  await deleteJournal(scenario.journalName);
  let workspace: EditorState = await openWorkspace(scenario, fetchImpl);
  /** Refuses the next input on `target` with `reason`, and keeps its working set. */
  async function refusesInput(target: EditorWorkspace, reason?: string) {
    const before = await readJournalSnapshot(target);
    expect(before.records).toHaveLength(2400);
    await expect(appendAuthorEdit(target, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" })).rejects.toThrow(reason);
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
        // A first group that is not closed stops the prefix, so the working set fills to its limit (ADR 0036).
        // Behavior of today, owned by #1775: a kept reconfirmation group cannot leave, and input continues behind it.
        await replacePrefix(workspace, withFirstSettlement(firstUncollected!, reconfirmation));
        await writeFences(workspace, []);
        await retainLocalRecovery(workspace, firstUncollected!.groups[0]!.journal_submission_group_id);
      }
    }
    expect(await rebuildPendingProjection(workspace)).toMatchObject({ save_state: "saved" });
    await refusesInput(workspace, "Local Edit Journal schema or partition is incompatible");
    // A refused group also stops the prefix.
    const firstGroupId = firstUncollected!.groups[0]!.journal_submission_group_id;
    await requestResult(workspace.database.transaction("metadata", "readwrite").objectStore("metadata")
      .delete(`${recoveryPrefix}${workspace.partition.journal_partition_id}:${firstGroupId}`));
    await replacePrefix(workspace, withFirstSettlement(firstUncollected!, refused));
    expect(await rebuildPendingProjection(workspace)).toMatchObject({ save_state: "needs_attention" });
    await refusesInput(workspace, "Local Edit Journal limit failed");
    await replacePrefix(workspace, firstCollected!);
    await writeFences(workspace, [firstCollected!.fences[0], ...(await readJournalSnapshot(workspace)).fences]);
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
    // An applied first group that the collection did not take is closed, so it leaves with the groups after it.
    await replacePrefix(reopened, firstUncollected!);
    await writeFences(reopened, saved.fences.slice(1));
    await appendAuthorEdit(reopened, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" });
    const continued = await readJournalSnapshot(reopened);
    expect([continued.records.length, continued.workingBoundary?.last_sequence]).toEqual([1, 2400]);
    expect((await retainedHistory(reopened)).records.filter((record) => record.author_edit_unit !== undefined))
      .toHaveLength(240);
  } finally {
    if (workspace.kind === "editor-ready") workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 360000);

it("retires settled single-input groups and an applied group that the collection did not take", async () => {
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
      remember(seen, await readJournalSnapshot(workspace));
    }
    const saved = await readJournalSnapshot(workspace);
    // The applied group that the collection did not take leaves with the collected group after it (ADR 0036).
    expect(saved.groups.map((group) => [group.ordered_coverage.length,
      group.payload_collection?.kind === "collected"])).toEqual([[120, true]]);
    expect([saved.records.length, saved.workingBoundary?.last_sequence]).toEqual([120, 2280]);

    const projection = await appendAuthorEdit(workspace, { from: text.body.length, to: text.body.length,
      text: "+", resultingBody: `${text.body}+`, inputOrigin: "typing" });
    expect(projection).toMatchObject({ body: `${text.body}+`, unsettled_intent_count: 1 });
    expect(await rebuildPendingProjection(workspace)).toEqual(projection);
    const continued = await readJournalSnapshot(workspace);
    expect([continued.records.length, continued.workingBoundary?.last_sequence]).toEqual([1, 2400]);
    const retained = await retainedHistory(workspace);
    expect([retained.records.length, retained.groups.length, retained.fences.length]).toEqual([2400, 130, 129]);
    // The group that the collection did not take leaves with its payload and gets no retained fence.
    expect(retained.records.filter((record) => record.author_edit_unit !== undefined)).toHaveLength(1);
    expect(retained).toEqual(expectedHistory(seen, {
      records: [...seen.records.keys()], chains: [...seen.chains.keys()],
      groups: [...seen.groups.keys()], fences: [...seen.fences.keys()],
    }));
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
}, 360000);

const PROPOSAL = "018f0000-0000-7001-8000-000000000c01";
const OPERATION = "018f0000-0000-7001-8000-000000000c02";
const REVISIONS = ["018f0000-0000-7001-8000-000000000c03", "018f0000-0000-7001-8000-000000000c04",
  "018f0000-0000-7001-8000-000000000c05"] as const;

function candidateEdit(revision: string, priorText: string, text: string,
  timing: { undoGroupId?: string; createdAt?: string } = {}): CandidateSelectionEdit {
  return { kind: "candidate_selection",
    target: { proposal_id: PROPOSAL, operation_id: OPERATION, revision_id: revision, manuscript_block_id: BLOCK },
    expectedProposalHeads: [revision], priorText, from: priorText.length, to: priorText.length, text,
    resultingBody: `${priorText}${text}`, ...timing };
}

/** The applied transport. The candidate Author Edit number n gives `proposal_revised` with `REVISIONS[n]`. */
function candidateTransport(scenario: ReturnType<typeof createBrowserScenario>, currentBody: () => string) {
  const applied = appliedAuthorEditTransport(scenario, currentBody);
  const targets: string[] = [];
  let sequence = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const request = path.endsWith("/manuscript/author-edits") ? requireRequestBody(init) : undefined;
    const target = request?.proposal_target as { revision_id: string } | undefined;
    if (request === undefined || target === undefined) return applied.fetchImpl(input, init);
    targets.push(target.revision_id);
    sequence += 1;
    const revision = REVISIONS[sequence]!;
    const response = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
      idempotencyKey: requestHeaders(init).get("idempotency-key")!, commandId: createJournalUuid(),
      authorCommandAdmissionId: createJournalUuid(), receiptId: createJournalUuid(),
      priorRevisionId: request.expected_authoritative_revision_id as string });
    const head = request.expected_authoritative_revision_id as string;
    return jsonResponse({ ...response, effect: { kind: "proposal_revised", proposal_revision_id: revision,
      author_action_sequence: String(sequence) }, receipt: { ...response.receipt, result: "proposal_revised",
      prior_heads: [head], resulting_heads: [head], proposal_revision_ids: [revision], authoritative_revision_ids: [],
      authoritative_commit_ids: [], author_action_sequence: String(sequence) } });
  };
  return { fetchImpl, targets };
}

it("retires a group that revised a Proposal with the next collected group", async () => {
  const scenario = createBrowserScenario();
  const text = { body: "Base" };
  const { fetchImpl, targets } = candidateTransport(scenario, () => text.body);
  await deleteJournal(scenario.journalName);
  const workspace = await openWorkspace(scenario, fetchImpl);
  const submit = () => submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto });
  const seen = emptyRows();
  try {
    await appendAuthorEdit(workspace, candidateEdit(REVISIONS[0], "Candidate", "!"));
    expect(await submit()).toMatchObject({ save_state: "saved", unsettled_intent_count: 0 });
    await typeRun(workspace, text, 1, 1);
    expect(await submit()).toMatchObject({ save_state: "saved", unsettled_intent_count: 0 });
    await collectEligibleJournalPayload(workspace);
    const saved = await readJournalSnapshot(workspace);
    remember(seen, saved);
    expect(saved.groups.map((group) => group.payload_collection?.kind)).toEqual([undefined, "collected"]);

    await typeRun(workspace, text, 2, 1);
    const continued = await readJournalSnapshot(workspace);
    expect([continued.records.length, continued.groups.length, continued.workingBoundary?.last_sequence])
      .toEqual([1, 0, 2]);
    expect(targets).toEqual([REVISIONS[0]]);
    // The closed group leaves with its payload and gets no retained collection fence.
    expect(await retainedHistory(workspace)).toEqual(expectedHistory(seen, {
      records: [...seen.records.keys()], chains: [...seen.chains.keys()],
      groups: [...seen.groups.keys()], fences: [...seen.fences.keys()],
    }));
    expect((await retainedHistory(workspace)).records[0]?.author_edit_unit).toBeDefined();
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

it("keeps a group that revised a Proposal while a later record targets the Revision that it revised", async () => {
  const scenario = createBrowserScenario();
  const text = { body: "Base" };
  const { fetchImpl, targets } = candidateTransport(scenario, () => text.body);
  await deleteJournal(scenario.journalName);
  const workspace = await openWorkspace(scenario, fetchImpl);
  const submit = () => submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto });
  try {
    await appendAuthorEdit(workspace, candidateEdit(REVISIONS[0], "Candidate", "!"));
    await submit();
    await typeRun(workspace, text, 1, 1);
    await submit();
    // A later record still targets the Revision that the first group revised.
    const undoGroupId = createJournalUuid();
    await appendAuthorEdit(workspace, candidateEdit(REVISIONS[0], "Candidate!", "?",
      { undoGroupId, createdAt: "2026-08-15T09:00:00.000Z" }));
    await collectEligibleJournalPayload(workspace);
    const before = await readJournalSnapshot(workspace);
    expect(before.groups.map((group) => group.payload_collection?.kind)).toEqual([undefined, "collected"]);
    await appendAuthorEdit(workspace, candidateEdit(REVISIONS[0], "Candidate!?", ".",
      { undoGroupId, createdAt: "2026-08-15T09:00:00.100Z" }));
    const kept = await readJournalSnapshot(workspace);
    expect([kept.records.length, kept.groups.length, kept.workingBoundary]).toEqual([4, 2, undefined]);

    expect(await submit()).toMatchObject({ save_state: "saved", unsettled_intent_count: 0 });
    expect(targets).toEqual([REVISIONS[0], REVISIONS[0]]);
    await typeRun(workspace, text, 3, 1);
    await submit();
    await collectEligibleJournalPayload(workspace);
    // No record after the prefix targets a revised Revision now, so both candidate groups leave.
    await typeRun(workspace, text, 4, 1);
    const continued = await readJournalSnapshot(workspace);
    expect([continued.records.length, continued.groups.length, continued.workingBoundary?.last_sequence])
      .toEqual([1, 0, 5]);
  } finally {
    workspace.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

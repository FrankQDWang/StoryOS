import { closeEditorFlowDraft, createProjectCommandChallenge, digestCloseEditorFlowDraft,
  getEditorSession } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CloseEditorFlowDraftRequest, CloseEditorFlowDraftResponse, DigestValue,
  EditorFlowDraftClosed, ProjectScope, RefusedEditDraftInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { uuidV7 } from "./acceptance-journal.ts";
import type { EditorWorkspace } from "./editor-types.ts";
import { MAX_WORKING_JOURNAL_ITEMS } from "./journal-working-set.ts";
import { digestJournalValue, rebuildPendingProjection } from "./local-edit-journal.ts";

export function canonicalDraftValue(value: unknown): string {
  const keys = new Set<string>();
  JSON.stringify(value, (key, item: unknown) => { keys.add(key); return item; });
  return JSON.stringify(value, [...keys].sort());
}
const same = (left: unknown, right: unknown) => canonicalDraftValue(left) === canonicalDraftValue(right);
function exactKeys(value: object, keys: string[]): boolean {
  return Object.keys(value).sort().join() === keys.sort().join();
}
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const RECORD_SCHEMA = "storyos.local-edit-journal.refused-edit-discard.v1";
export type DiscardRecord = {
  key: string; schema_id: string; explicit_command_record_id: string; local_intent_sequence: number;
  journal_partition_id: string; project_scope: ProjectScope; editor_session_id: string;
  writer_generation: string; exact_semantic_payload_ref: string; semantic_payload_digest: DigestValue;
  exact_target_head_anchor_bindings: { draft_id: string; draft_revision_id: string; payload_digest: string; expected_closure: "open" };
  editor_contract_revision: string; command_kind: "closeEditorFlowDraft"; created_at: string;
  author_visible_decision_ref: { draft_id: string; close_reason: "abandoned" };
  group: { journal_submission_group_id: string; action_class: "explicit_editor_command";
    api_major: 1; method: "POST"; route_template: string; idempotency_key: string;
    ordered_coverage: { local_intent_sequence: number; intent_record_ref: string; payload_digest: DigestValue }[];
    covered_sequence_range: { first: number; last: number }; frozen_payload_coverage_digest: DigestValue;
    frozen_request_body: CloseEditorFlowDraftRequest; frozen_request_digest: DigestValue };
};
export type DiscardObservation = { key: string; record_key: string; schema_id: string } & (
  { kind: "settled_closed"; event: EditorFlowDraftClosed }
  | { kind: "settled"; response: CloseEditorFlowDraftResponse }
  | { kind: "unresolved" });
const result = <T,>(request: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("Discard Journal read failed"));
});
const committed = (transaction: IDBTransaction) => new Promise<void>((resolve, reject) => {
  transaction.oncomplete = () => resolve();
  transaction.onabort = transaction.onerror = () => reject(transaction.error ?? new Error("Discard Journal write failed"));
});

function matchesEvent(record: DiscardRecord, event: EditorFlowDraftClosed): boolean {
  const input = record.group.frozen_request_body.close_editor_flow_draft_input;
  return exactKeys(event, ["schema_id", "event_kind", "event_id", "project_scope", "draft_id", "draft_revision_id",
    "payload_digest", "prior_closure", "closure", "close_reason", "source", "author_action_sequence", "created_at"])
    && exactKeys(event.source, ["command_id", "author_command_admission_id", "receipt_id", "idempotency_key", "command_digest"])
    && event.schema_id === "storyos.event.editor-flow-draft-closed.v1"
    && event.event_kind === "editor_flow_draft_closed" && UUID.test(event.event_id)
    && same(event.project_scope, record.project_scope) && event.draft_id === input.draft_id
    && event.draft_revision_id === input.source_current_draft_revision_id
    && event.payload_digest === input.source_draft_payload_digest
    && event.prior_closure === "open" && event.closure === "closed" && event.close_reason === "abandoned"
    && UUID.test(event.source.command_id) && UUID.test(event.source.author_command_admission_id)
    && UUID.test(event.source.receipt_id) && event.source.idempotency_key === record.group.idempotency_key
    && same(event.source.command_digest, record.group.frozen_request_digest)
    && /^[1-9][0-9]*$/.test(event.author_action_sequence) && Number.isFinite(Date.parse(event.created_at));
}
function matchesResponse(record: DiscardRecord, response: CloseEditorFlowDraftResponse): boolean {
  const receipt = response.receipt;
  const effect = response.effect;
  return exactKeys(response, ["schema_id", "correlation_id", "project_scope", "command_id", "author_command_admission_id", "receipt", "effect"])
    && response.schema_id === "storyos.command.close-editor-flow-draft.response.v1"
    && response.correlation_id === record.group.frozen_request_body.close_editor_flow_draft_input.correlation_id
    && same(response.project_scope, record.project_scope) && UUID.test(response.command_id)
    && UUID.test(response.author_command_admission_id) && UUID.test(receipt.receipt_id)
    && same(receipt.project_scope, record.project_scope) && receipt.command_kind === "closeEditorFlowDraft"
    && receipt.producer_cause === "author_command_admission"
    && receipt.author_command_admission_id === response.author_command_admission_id
    && receipt.idempotency_key === record.group.idempotency_key
    && same(receipt.command_digest, record.group.frozen_request_digest) && receipt.result === effect.kind
    && same(receipt.expected_heads, []) && same(receipt.prior_heads, []) && same(receipt.resulting_heads, [])
    && same(receipt.authoritative_revision_ids, []) && same(receipt.proposal_revision_ids, [])
    && same(receipt.authoritative_commit_ids, []) && same(receipt.condition_refs, [])
    && (effect.kind === "draft_closure_changed" ? matchesEvent(record, effect.event)
      && same(effect.event.source, { command_id: response.command_id,
        author_command_admission_id: response.author_command_admission_id, receipt_id: receipt.receipt_id,
        idempotency_key: record.group.idempotency_key, command_digest: record.group.frozen_request_digest })
      && same(receipt.draft_artifact_refs, [effect.event.draft_id])
      && same(receipt.artifact_lifecycle_event_refs, [effect.event.event_id])
      && receipt.author_action_sequence === effect.event.author_action_sequence && receipt.created_at === effect.event.created_at
      : ((effect.kind === "conflicted" && UUID.test(effect.current_revision_id) && /^[0-9a-f]{64}$/.test(effect.current_digest))
        || (effect.kind === "refused" && ["source_draft_not_open", "source_unavailable"].includes(effect.reason)))
        && ["open", "closed"].includes(effect.current_closure) && receipt.author_action_sequence === null
        && same(receipt.draft_artifact_refs, [record.author_visible_decision_ref.draft_id]) && same(receipt.artifact_lifecycle_event_refs, []));
}

export async function readDiscardJournal(workspace: EditorWorkspace, snapshotTransaction?: IDBTransaction):
  Promise<{ record: DiscardRecord; observation?: DiscardObservation | undefined }[]> {
  const transaction = snapshotTransaction ?? workspace.database.transaction("metadata", "readonly");
  const metadata = transaction.objectStore("metadata");
  const [records, observations, schema] = await Promise.all([
    result(metadata.getAll(IDBKeyRange.bound("discard:", "discard:\uffff"), MAX_WORKING_JOURNAL_ITEMS + 1)) as Promise<DiscardRecord[]>,
    result(metadata.getAll(IDBKeyRange.bound("discard-observation:", "discard-observation:\uffff"), MAX_WORKING_JOURNAL_ITEMS + 1)) as Promise<DiscardObservation[]>,
    result(metadata.get("schema")) as Promise<{ version: number }>,
  ]);
  if (schema?.version !== 4 || records.length > MAX_WORKING_JOURNAL_ITEMS || observations.length > MAX_WORKING_JOURNAL_ITEMS) throw new Error("Discard Journal unavailable");
  for (const record of records) {
    const input = record.group?.frozen_request_body?.close_editor_flow_draft_input;
    const partition = workspace.partition;
    const coverage = { ordered_coverage: record.group?.ordered_coverage, covered_sequence_range: record.group?.covered_sequence_range };
    const coverageDigest = { ...await digestJournalValue(coverage, workspace.cryptoImpl),
      profile: "storyos.local-edit-journal.submission-coverage.sha256.v1" };
    if (record.group === undefined || input === undefined || record.schema_id !== RECORD_SCHEMA || record.command_kind !== "closeEditorFlowDraft"
      || record.key !== `discard:${input?.draft_id}` || !UUID.test(record.explicit_command_record_id)
      || !Number.isSafeInteger(record.local_intent_sequence) || record.local_intent_sequence <= 0
      || !same(record.project_scope, workspace.partition.project_scope)
      || !record.journal_partition_id.startsWith(`${record.project_scope.owner_user_id}:${record.project_scope.project_id}:${record.editor_session_id}:${record.writer_generation}:`)
      || record.exact_semantic_payload_ref !== record.group.journal_submission_group_id
      || !same(record.semantic_payload_digest, record.group.frozen_request_digest)
      || record.editor_contract_revision !== "storyos.editor-contract.release-1.v3"
      || !same(record.exact_target_head_anchor_bindings, { draft_id: input?.draft_id,
        draft_revision_id: input?.source_current_draft_revision_id, payload_digest: input?.source_draft_payload_digest, expected_closure: "open" })
      || !same(record.group.ordered_coverage, [{ local_intent_sequence: record.local_intent_sequence,
        intent_record_ref: record.explicit_command_record_id, payload_digest: record.group.frozen_request_digest }])
      || !same(record.group.covered_sequence_range, { first: record.local_intent_sequence, last: record.local_intent_sequence })
      || !same(record.group.frozen_payload_coverage_digest, coverageDigest)
      || !same(record.author_visible_decision_ref, { draft_id: input?.draft_id, close_reason: "abandoned" })
      || !exactKeys(record.group, ["journal_submission_group_id", "action_class", "api_major", "method", "route_template",
        "idempotency_key", "frozen_request_body", "frozen_request_digest", "ordered_coverage", "covered_sequence_range", "frozen_payload_coverage_digest"])
      || !exactKeys(record.group.frozen_request_body, ["command_schema", "close_editor_flow_draft_input"])
      || record.group.action_class !== "explicit_editor_command" || record.group.api_major !== 1
      || record.group.method !== "POST" || record.group.route_template !== "/api/v1/projects/{project_id}/drafts/{draft_id}/closures"
      || !UUID.test(record.group.journal_submission_group_id) || !UUID.test(record.group.idempotency_key)
      || record.group.frozen_request_body.command_schema !== "storyos.command.close-editor-flow-draft.request.v1"
      || !same(input, { draft_id: input?.draft_id, draft_kind: "refused_edit",
        source_current_draft_revision_id: input?.source_current_draft_revision_id,
        source_draft_payload_digest: input?.source_draft_payload_digest, expected_closure: "open", close_reason: "abandoned",
        editor_session_id: record.editor_session_id, writer_generation: record.writer_generation,
        client_contract_revision: partition.client_contract_revision, security_policy_revision: partition.security_policy_revision, correlation_id: input?.correlation_id })
      || !UUID.test(input.draft_id) || !UUID.test(input.source_current_draft_revision_id) || !UUID.test(input.correlation_id)
      || !/^[0-9a-f]{64}$/.test(input.source_draft_payload_digest)
      || !same(record.group.frozen_request_digest, await digestCloseEditorFlowDraft(record.group.frozen_request_body, workspace.cryptoImpl))
      || !Number.isFinite(Date.parse(record.created_at))
      || Object.keys(record).sort().join() !== ["key", "schema_id", "explicit_command_record_id", "local_intent_sequence",
        "journal_partition_id", "project_scope", "editor_session_id", "writer_generation", "command_kind", "created_at",
        "author_visible_decision_ref", "group", "exact_semantic_payload_ref", "semantic_payload_digest",
        "exact_target_head_anchor_bindings", "editor_contract_revision"].sort().join()) throw new Error("Discard Journal unavailable");
  }
  for (const observation of observations) {
    const record = records.find((item) => item.key === observation.record_key);
    if (record === undefined || observation.schema_id !== `${RECORD_SCHEMA}.observation`
      || !exactKeys(observation, ["key", "record_key", "schema_id", "kind",
        ...(observation.kind === "settled" ? ["response"] : observation.kind === "settled_closed" ? ["event"] : [])])
      || !observation.key.startsWith(`discard-observation:${record.explicit_command_record_id}:`)
      || (observation.kind === "settled_closed" ? !matchesEvent(record, observation.event)
        : observation.kind === "settled" ? !matchesResponse(record, observation.response)
          : observation.kind !== "unresolved")) throw new Error("Discard Journal unavailable");
  }
  return records.map((record) => ({ record, observation: observations.filter((item) => item.record_key === record.key)
    .sort((left, right) => left.key.localeCompare(right.key)).at(-1) }));
}

export async function observeDiscard(workspace: EditorWorkspace, record: DiscardRecord,
  value: Omit<Extract<DiscardObservation, { kind: "settled_closed" }>, "key" | "record_key" | "schema_id">
    | Omit<Extract<DiscardObservation, { kind: "settled" }>, "key" | "record_key" | "schema_id">
    | { kind: "unresolved" }): Promise<void> {
  if ((value.kind === "settled_closed" && !matchesEvent(record, value.event))
    || (value.kind === "settled" && !matchesResponse(record, value.response))) throw new Error("Discard outcome unresolved");
  const transaction = workspace.database.transaction("metadata", "readwrite", { durability: "strict" });
  const done = committed(transaction);
  const metadata = transaction.objectStore("metadata");
  if (!same(await result(metadata.get(record.key)), record)) {
    transaction.abort(); await done; return;
  }
  metadata.add({ key: `discard-observation:${record.explicit_command_record_id}:${uuidV7(workspace.cryptoImpl)}`,
    record_key: record.key, schema_id: `${RECORD_SCHEMA}.observation`, ...value });
  await done;
}

export async function reconcileDiscard(workspace: EditorWorkspace, draft: RefusedEditDraftInspect): Promise<DiscardObservation | undefined> {
  const found = (await readDiscardJournal(workspace)).find(({ record }) => record.key === `discard:${draft.draft_id}`);
  if (found === undefined) return undefined;
  if (draft.closure === "closed" && draft.closure_event && matchesEvent(found.record, draft.closure_event)) {
    if (found.observation?.kind !== "settled_closed") {
      await observeDiscard(workspace, found.record, { kind: "settled_closed", event: draft.closure_event });
    }
    return { key: "", record_key: found.record.key, schema_id: `${RECORD_SCHEMA}.observation`,
      kind: "settled_closed", event: draft.closure_event };
  }
  return found.observation ?? { key: "", record_key: found.record.key,
    schema_id: `${RECORD_SCHEMA}.observation`, kind: "unresolved" };
}

export async function discardRefusedEdit({ workspace, draft, baseUrl, fetchImpl, isCurrent = () => true }: {
  workspace: EditorWorkspace; draft: RefusedEditDraftInspect; baseUrl: string; fetchImpl: typeof fetch;
  isCurrent?: () => boolean;
}): Promise<void> {
  await navigator.locks.request(`storyos-discard:${workspace.partition.project_scope.project_id}`, async () => {
    const journal = await readDiscardJournal(workspace);
    const pending = await rebuildPendingProjection(workspace);
    const scope = workspace.partition.project_scope;
    const canonical = await getEditorSession({ baseUrl, projectId: scope.project_id,
      editorSessionId: workspace.partition.editor_session_id, fetchImpl });
    if (!isCurrent() || journal.length >= MAX_WORKING_JOURNAL_ITEMS || workspace.partition.disposition !== "current_writer_open" || pending.save_state !== "saved"
      || pending.unsettled_intent_count !== 0 || journal.some(({ observation }) => observation === undefined || observation.kind === "unresolved")
      || journal.some(({ record }) => record.key === `discard:${draft.draft_id}`)
      || !same(canonical.project_scope, scope) || canonical.writer.kind !== "current_writer"
      || canonical.writer.writer_generation !== workspace.partition.writer_generation
      || canonical.editor_session.editor_session_id !== workspace.partition.editor_session_id
      || canonical.editor_session.client_session_binding_ref !== workspace.partition.client_session_binding_ref
      || canonical.editor_session.client_session_generation !== workspace.partition.client_session_generation
      || canonical.editor_session.disposition !== "open" || draft.closure !== "open" || draft.retention_state !== "retained") {
      throw new Error("Discard requires a settled current writer and open retained Draft");
    }
    const request: CloseEditorFlowDraftRequest = { command_schema: "storyos.command.close-editor-flow-draft.request.v1",
      close_editor_flow_draft_input: { draft_id: draft.draft_id, draft_kind: "refused_edit",
        source_current_draft_revision_id: draft.draft_revision_id, source_draft_payload_digest: draft.payload_digest,
        expected_closure: "open", close_reason: "abandoned", editor_session_id: workspace.partition.editor_session_id,
        writer_generation: workspace.partition.writer_generation, client_contract_revision: workspace.partition.client_contract_revision,
        security_policy_revision: workspace.partition.security_policy_revision, correlation_id: uuidV7(workspace.cryptoImpl) } };
    const digest = await digestCloseEditorFlowDraft(request, workspace.cryptoImpl);
    const previousSequence = await result(workspace.database.transaction("metadata").objectStore("metadata")
      .get("local_intent_sequence")) as { value: number } | undefined;
    const sequence = (previousSequence?.value ?? 0) + 1;
    const recordId = uuidV7(workspace.cryptoImpl);
    const groupId = uuidV7(workspace.cryptoImpl);
    const coverage = { ordered_coverage: [{ local_intent_sequence: sequence,
      intent_record_ref: recordId, payload_digest: digest }], covered_sequence_range: { first: sequence, last: sequence } };
    const coverageDigest = { ...await digestJournalValue(coverage, workspace.cryptoImpl),
      profile: "storyos.local-edit-journal.submission-coverage.sha256.v1" };
    const transaction = workspace.database.transaction(["metadata", "partitions"], "readwrite", { durability: "strict" });
    const done = committed(transaction);
    const metadata = transaction.objectStore("metadata");
    const [previous, partition, existing] = await Promise.all([result(metadata.get("local_intent_sequence")) as Promise<{ value: number } | undefined>,
      result(transaction.objectStore("partitions").get(workspace.partition.journal_partition_id)), result(metadata.get(`discard:${draft.draft_id}`))]);
    if (!same(partition, workspace.partition) || existing !== undefined || (previous?.value ?? 0) !== (previousSequence?.value ?? 0) || !Number.isSafeInteger(sequence)) {
      transaction.abort(); await done; return;
    }
    const record: DiscardRecord = { key: `discard:${draft.draft_id}`, schema_id: RECORD_SCHEMA,
      explicit_command_record_id: recordId, local_intent_sequence: sequence,
      journal_partition_id: workspace.partition.journal_partition_id, project_scope: scope,
      editor_session_id: workspace.partition.editor_session_id, writer_generation: workspace.partition.writer_generation,
      exact_semantic_payload_ref: groupId, semantic_payload_digest: digest,
      exact_target_head_anchor_bindings: { draft_id: draft.draft_id, draft_revision_id: draft.draft_revision_id,
        payload_digest: draft.payload_digest, expected_closure: "open" }, editor_contract_revision: "storyos.editor-contract.release-1.v3",
      command_kind: "closeEditorFlowDraft", created_at: new Date().toISOString(),
      author_visible_decision_ref: { draft_id: draft.draft_id, close_reason: "abandoned" },
      group: { journal_submission_group_id: groupId, ...coverage, frozen_payload_coverage_digest: coverageDigest, action_class: "explicit_editor_command",
        api_major: 1, method: "POST", route_template: "/api/v1/projects/{project_id}/drafts/{draft_id}/closures",
        idempotency_key: uuidV7(workspace.cryptoImpl), frozen_request_body: request, frozen_request_digest: digest } };
    metadata.add(record);
    metadata.put({ key: "local_intent_sequence", value: sequence });
    await done;
    await readDiscardJournal(workspace);
    const guardedFetch: typeof fetch = (input, init) => {
      if (!isCurrent()) throw new Error("Discard view changed");
      return fetchImpl(input, init);
    };
    try {
      const challenge = await createProjectCommandChallenge({ baseUrl, projectId: scope.project_id, fetchImpl: guardedFetch,
        request: { method: record.group.method, route_template: record.group.route_template,
          command_schema: request.command_schema, canonical_command_digest: digest, idempotency_key: record.group.idempotency_key } });
      const response = await closeEditorFlowDraft({ baseUrl, projectId: scope.project_id, draftId: draft.draft_id,
        fetchImpl: guardedFetch, request, idempotencyKey: record.group.idempotency_key, antiForgery: challenge.nonce });
      await observeDiscard(workspace, record, { kind: "settled", response });
    } catch (error) {
      await observeDiscard(workspace, record, { kind: "unresolved" });
      throw error;
    }
  });
}

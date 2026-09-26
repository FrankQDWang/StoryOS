import { digestUndoLatestAuthorAction } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue, EditorFlowDraftClosed, EditorFlowDraftReopened, ProjectScope,
  RefusedEditDraftInspect, UndoLatestAuthorActionRequest, UndoLatestAuthorActionResponse }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace } from "./editor-types.ts";
import { canonicalDraftValue, MAX_DISCARD_RECORDS } from "./refused-edit-discard.ts";
import { uuidV7 } from "./acceptance-journal.ts";

const keys = (value: unknown, required: string[], optional: string[] = []) => value !== null && typeof value === "object"
  && required.every((key) => Object.hasOwn(value, key)) && Object.keys(value).every((key) => [...required, ...optional].includes(key));
const positive = (value: unknown): value is string => typeof value === "string" && /^[1-9][0-9]{0,19}$/.test(value)
  && BigInt(value) <= 18446744073709551615n;
const same = (a: unknown, b: unknown) => canonicalDraftValue(a) === canonicalDraftValue(b);
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
export type DraftUndoRecord = {
  key: string; schema_id: string; project_scope: ProjectScope; journal_partition_id: string;
  explicit_command_record_id: string; local_intent_sequence: number; created_at: string;
  source_close: EditorFlowDraftClosed; request: UndoLatestAuthorActionRequest;
  idempotency_key: string; digest: DigestValue;
};
type Observation = { key: string; record_key: string; event?: EditorFlowDraftReopened;
  response?: UndoLatestAuthorActionResponse };
const read = <T,>(request: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("Undo Journal read failed"));
});
const committed = (transaction: IDBTransaction) => new Promise<void>((resolve, reject) => {
  transaction.oncomplete = () => resolve();
  transaction.onabort = transaction.onerror = () => reject(transaction.error ?? new Error("Undo Journal write failed"));
});

export function validDraftReopen(draft: Pick<RefusedEditDraftInspect, "draft_id" | "draft_revision_id" | "payload_digest" | "closure" | "closure_event" | "reopen_event">, scope: ProjectScope): boolean {
  const event = draft.reopen_event, closed = draft.closure_event;
  if (event == null) return draft.closure === "closed" || closed == null;
  const receipt = event.handler_receipt;
  return keys(event, ["schema_id", "event_kind", "event_id", "project_scope", "draft_id", "draft_revision_id", "payload_digest",
    "source_close_event_id", "prior_closure", "closure", "source", "handler_receipt", "source_author_action_sequence", "author_action_sequence", "created_at"])
    && keys(receipt, ["schema_id", "receipt_id", "project_scope", "author_undo_receipt_id", "source_close_event_id", "event_id", "result", "created_at"])
    && validSource(event.source, "storyos.command.undoLatestAuthorAction.jcs.v1")
    && draft.closure === "open" && closed != null && same(event.project_scope, scope)
    && event.schema_id === "storyos.event.editor-flow-draft-reopened.v1" && event.event_kind === "editor_flow_draft_reopened"
    && event.draft_id === draft.draft_id && event.draft_revision_id === draft.draft_revision_id
    && event.payload_digest === draft.payload_digest && event.source_close_event_id === closed.event_id
    && event.source_author_action_sequence === closed.author_action_sequence
    && event.prior_closure === "closed" && event.closure === "open" && UUID.test(event.event_id)
    && positive(event.author_action_sequence) && positive(closed.author_action_sequence)
    && BigInt(event.author_action_sequence) > BigInt(closed.author_action_sequence)
    && receipt.schema_id === "storyos.receipt.draft-reopen.v1" && receipt.result === "draft_reopened"
    && UUID.test(receipt.receipt_id) && same(receipt.project_scope, scope)
    && receipt.event_id === event.event_id && receipt.source_close_event_id === closed.event_id
    && receipt.author_undo_receipt_id === event.source.receipt_id && receipt.created_at === event.created_at
    && Number.isFinite(Date.parse(event.created_at));
}
function matches(record: DraftUndoRecord, event: EditorFlowDraftReopened): boolean {
  return validDraftReopen({ draft_id: record.source_close.draft_id,
    draft_revision_id: record.source_close.draft_revision_id, payload_digest: record.source_close.payload_digest,
    closure: "open", closure_event: record.source_close, reopen_event: event }, record.project_scope)
    && event.source.idempotency_key === record.idempotency_key && same(event.source.command_digest, record.digest);
}

function validSource(source: EditorFlowDraftClosed["source"], profile: string): boolean {
  return keys(source, ["command_id", "author_command_admission_id", "receipt_id", "idempotency_key", "command_digest"])
    && [source.command_id, source.author_command_admission_id, source.receipt_id, source.idempotency_key].every((id) => UUID.test(id))
    && keys(source.command_digest, ["algorithm", "profile", "value_hex_lowercase"])
    && source.command_digest.algorithm === "sha256" && source.command_digest.profile === profile
    && /^[0-9a-f]{64}$/.test(source.command_digest.value_hex_lowercase);
}
function validObservation(record: DraftUndoRecord, observation: Observation): boolean {
  const response = observation.response, event = observation.event;
  if (!keys(observation, ["key", "record_key", event === undefined ? "response" : "event"])
    || observation.record_key !== record.key || observation.key !== `draft-undo-observation:${record.source_close.event_id}`) return false;
  if (event !== undefined) return matches(record, event);
  if (!response || !keys(response, ["schema_id", "correlation_id", "project_scope", "command_id", "author_command_admission_id", "receipt", "project", "effect"])) return false;
  const receipt = response.receipt, effect = response.effect;
  const success = effect.kind === "draft_compensated";
  return keys(receipt, ["receipt_id", "project_scope", "command_kind", "command_digest", "idempotency_key", "producer_cause",
    "author_command_admission_id", "expected_heads", "prior_heads", "resulting_heads", "authoritative_revision_ids", "proposal_revision_ids",
    "authoritative_commit_ids", "draft_artifact_refs", "artifact_lifecycle_event_refs", "condition_refs", "result", "created_at"], ["author_action_sequence"])
    && response.schema_id === "storyos.command.undo-latest-author-action.response.v1"
    && same(response.project_scope, record.project_scope) && same(receipt.project_scope, record.project_scope)
    && response.correlation_id === record.request.undo_latest_author_action_input.correlation_id
    && UUID.test(response.command_id) && UUID.test(response.author_command_admission_id) && UUID.test(receipt.receipt_id)
    && receipt.author_command_admission_id === response.author_command_admission_id && receipt.command_kind === "undoLatestAuthorAction"
    && receipt.producer_cause === "author_command_admission" && receipt.idempotency_key === record.idempotency_key
    && same(receipt.command_digest, record.digest) && Number.isFinite(Date.parse(receipt.created_at))
    && [receipt.expected_heads, receipt.prior_heads, receipt.resulting_heads].every((heads) => same(heads, [record.request.undo_latest_author_action_input.expected_authoritative_revision_id]))
    && [receipt.authoritative_revision_ids, receipt.proposal_revision_ids, receipt.authoritative_commit_ids, receipt.condition_refs].every((refs) => same(refs, []))
    && keys(response.project, ["project_id", "title", "open"]) && response.project.project_id === record.project_scope.project_id
    && typeof response.project.title === "string" && keys(response.project.open, response.project.open.kind === "empty" ? ["kind"] : ["kind", "current_chapter_id"])
    && (response.project.open.kind === "empty" || (response.project.open.kind === "current_chapter" && UUID.test(response.project.open.current_chapter_id)))
    && (effect.kind !== "conflicted" || effect.current_author_undo_frontier_sequence === undefined || positive(effect.current_author_undo_frontier_sequence))
    && (success ? keys(effect, ["kind", "event", "author_undo_frontier_sequence"]) && matches(record, effect.event)
      && (effect.author_undo_frontier_sequence === null || positive(effect.author_undo_frontier_sequence))
      && same(effect.event.source, { command_id: response.command_id, author_command_admission_id: response.author_command_admission_id,
        receipt_id: receipt.receipt_id, idempotency_key: record.idempotency_key, command_digest: record.digest })
      && receipt.result === "draft_closure_changed" && receipt.created_at === effect.event.created_at
      && receipt.author_action_sequence === effect.event.author_action_sequence
      && same(receipt.draft_artifact_refs, [record.source_close.draft_id]) && same(receipt.artifact_lifecycle_event_refs, [effect.event.event_id])
      : keys(effect, ["kind", "reason"], effect.kind === "conflicted" ? ["current_author_undo_frontier_sequence"] : [])
        && ((effect.kind === "conflicted" && ["frontier_mismatch", "wrong_target_head", "source_binding_changed"].includes(effect.reason) && receipt.result === "conflicted")
          || (effect.kind === "unavailable" && ["no_frontier", "barrier", "source_unavailable"].includes(effect.reason) && receipt.result === "refused"))
        && receipt.author_action_sequence == null && same(receipt.draft_artifact_refs, []) && same(receipt.artifact_lifecycle_event_refs, []));
}

export async function readDraftUndoJournal(workspace: EditorWorkspace) {
  const metadata = workspace.database.transaction("metadata").objectStore("metadata");
  const [records, observations] = await Promise.all([
    read(metadata.getAll(IDBKeyRange.bound("draft-undo:", "draft-undo:\uffff"), MAX_DISCARD_RECORDS + 1)) as Promise<DraftUndoRecord[]>,
    read(metadata.getAll(IDBKeyRange.bound("draft-undo-observation:", "draft-undo-observation:\uffff"), MAX_DISCARD_RECORDS + 1)) as Promise<Observation[]>,
  ]);
  if (records.length > MAX_DISCARD_RECORDS || observations.length > MAX_DISCARD_RECORDS) throw new Error("Undo Journal limit");
  for (const record of records) {
    const partition = await read(workspace.database.transaction("partitions").objectStore("partitions").get(record.journal_partition_id)) as EditorWorkspace["partition"] | undefined;
    const input = record.request?.undo_latest_author_action_input;
    if (!keys(record, ["key", "schema_id", "project_scope", "journal_partition_id", "explicit_command_record_id", "local_intent_sequence", "created_at", "source_close", "request", "idempotency_key", "digest"])
      || !keys(record.request, ["command_schema", "undo_latest_author_action_input"])
      || !keys(input, ["expected_author_undo_frontier_sequence", "expected_authoritative_revision_id", "editor_session_id", "client_contract_revision", "security_policy_revision", "correlation_id"])
      || partition === undefined || !same(partition.project_scope, record.project_scope)
      || input.editor_session_id !== partition.editor_session_id || input.client_contract_revision !== partition.client_contract_revision
      || input.security_policy_revision !== partition.security_policy_revision
      || record.request.command_schema !== "storyos.command.undo-latest-author-action.request.v1"
      || ![input.editor_session_id, input.correlation_id, input.expected_authoritative_revision_id].every((id) => UUID.test(id))
      || !keys(record.source_close, ["schema_id", "event_kind", "event_id", "project_scope", "draft_id", "draft_revision_id", "payload_digest", "prior_closure", "closure", "close_reason", "source", "author_action_sequence", "created_at"])
      || !validSource(record.source_close.source, "storyos.command.closeEditorFlowDraft.jcs.v1")
      || record.source_close.schema_id !== "storyos.event.editor-flow-draft-closed.v1" || record.source_close.event_kind !== "editor_flow_draft_closed"
      || record.source_close.prior_closure !== "open" || record.source_close.closure !== "closed" || record.source_close.close_reason !== "abandoned"
      || ![record.source_close.event_id, record.source_close.draft_id, record.source_close.draft_revision_id].every((id) => UUID.test(id))
      || !/^[0-9a-f]{64}$/.test(record.source_close.payload_digest) || !positive(record.source_close.author_action_sequence)
      || !Number.isFinite(Date.parse(record.source_close.created_at))
      || record.schema_id !== "storyos.local-edit-journal.draft-undo.v1" || input === undefined
      || record.key !== `draft-undo:${record.source_close?.event_id}` || !UUID.test(record.idempotency_key)
      || !UUID.test(record.explicit_command_record_id) || !same(record.project_scope, workspace.partition.project_scope)
      || !same(record.source_close.project_scope, record.project_scope)
      || input.expected_author_undo_frontier_sequence !== record.source_close.author_action_sequence
      || !record.journal_partition_id.startsWith(`${record.project_scope.owner_user_id}:${record.project_scope.project_id}:${input.editor_session_id}:`)
      || !Number.isSafeInteger(record.local_intent_sequence) || record.local_intent_sequence <= 0
      || !Number.isFinite(Date.parse(record.created_at))
      || !same(record.digest, await digestUndoLatestAuthorAction(record.request, workspace.cryptoImpl))) throw new Error("Undo Journal unavailable");
  }
  for (const observation of observations) {
    const record = records.find((record) => record.key === observation.record_key);
    if (!record || !validObservation(record, observation)) throw new Error("Undo evidence unavailable");
  }
  return records.map((record) => ({ record, observation: observations.find((row) => row.record_key === record.key) }));
}

export async function freezeDraftUndo(workspace: EditorWorkspace, source_close: EditorFlowDraftClosed,
  request: UndoLatestAuthorActionRequest, idempotency_key: string, isCurrent: () => boolean): Promise<DraftUndoRecord> {
  const records = await readDraftUndoJournal(workspace);
  const existing = records.find(({ record }) => record.key === `draft-undo:${source_close.event_id}`);
  if (existing) return existing.record;
  if (records.length >= MAX_DISCARD_RECORDS || records.some(({ observation }) => observation === undefined)) throw new Error("Undo unresolved");
  const digest = await digestUndoLatestAuthorAction(request, workspace.cryptoImpl);
  const transaction = workspace.database.transaction(["metadata", "partitions"], "readwrite", { durability: "strict" });
  const done = committed(transaction), metadata = transaction.objectStore("metadata");
  const sequence = await read(metadata.get("local_intent_sequence")) as { value: number } | undefined;
  const partition = await read(transaction.objectStore("partitions").get(workspace.partition.journal_partition_id));
  const retained = await read(metadata.getAll(IDBKeyRange.bound("draft-undo:", "draft-undo:\uffff"), MAX_DISCARD_RECORDS + 1));
  const local_intent_sequence = (sequence?.value ?? 0) + 1;
  if (!same(partition, workspace.partition) || !isCurrent() || retained.length >= MAX_DISCARD_RECORDS || !Number.isSafeInteger(local_intent_sequence)) {
    transaction.abort(); await done; throw new Error("Undo partition changed");
  }
  const record: DraftUndoRecord = { key: `draft-undo:${source_close.event_id}`, schema_id: "storyos.local-edit-journal.draft-undo.v1",
    project_scope: workspace.partition.project_scope, journal_partition_id: workspace.partition.journal_partition_id,
    explicit_command_record_id: uuidV7(workspace.cryptoImpl), local_intent_sequence, created_at: new Date().toISOString(),
    source_close, request, idempotency_key, digest };
  metadata.add(record); metadata.put({ key: "local_intent_sequence", value: local_intent_sequence });
  await done; return record;
}
export async function observeDraftUndo(workspace: EditorWorkspace, record: DraftUndoRecord,
  value: Pick<Observation, "event" | "response">, isCurrent: () => boolean = () => true): Promise<void> {
  const observation = { key: `draft-undo-observation:${record.source_close.event_id}`, record_key: record.key, ...value };
  if (!validObservation(record, observation)) throw new Error("Undo evidence unavailable");
  const transaction = workspace.database.transaction("metadata", "readwrite", { durability: "strict" });
  const done = committed(transaction), metadata = transaction.objectStore("metadata");
  const key = `draft-undo-observation:${record.source_close.event_id}`;
  if (!same(await read(metadata.get(record.key)), record)) { transaction.abort(); await done; throw new Error("Undo identity changed"); }
  const previous = await read(metadata.get(key)) as Observation | undefined;
  if (!isCurrent()) { transaction.abort(); await done; throw new Error("Undo view changed"); }
  if (previous === undefined) metadata.add(observation);
  else if (value.event && !same(previous.event ?? (previous.response?.effect.kind === "draft_compensated" ? previous.response.effect.event : undefined), value.event)) {
    transaction.abort(); await done; throw new Error("Undo settlement changed");
  }
  await done;
  await readDraftUndoJournal(workspace);
}
export async function reconcileDraftUndo(workspace: EditorWorkspace, draft: RefusedEditDraftInspect, isCurrent: () => boolean = () => true) {
  const records = await readDraftUndoJournal(workspace);
  const event = draft.reopen_event;
  if (event == null) return undefined;
  const found = records.find(({ record }) => matches(record, event));
  if (found && found.observation === undefined) await observeDraftUndo(workspace, found.record, { event }, isCurrent);
  return found ? event : undefined;
}

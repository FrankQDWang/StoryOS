import { digestUndoLatestAuthorAction } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue, EditorFlowDraftClosed, EditorFlowDraftReopened, ProjectScope,
  RefusedEditDraftInspect, UndoLatestAuthorActionRequest, UndoLatestAuthorActionResponse }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace } from "./editor-types.ts";
import { canonicalDraftValue, MAX_DISCARD_RECORDS } from "./refused-edit-discard.ts";
import { uuidV7 } from "./acceptance-journal.ts";

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
  return draft.closure === "open" && closed != null && same(event.project_scope, scope)
    && event.schema_id === "storyos.event.editor-flow-draft-reopened.v1" && event.event_kind === "editor_flow_draft_reopened"
    && event.draft_id === draft.draft_id && event.draft_revision_id === draft.draft_revision_id
    && event.payload_digest === draft.payload_digest && event.source_close_event_id === closed.event_id
    && event.source_author_action_sequence === closed.author_action_sequence
    && event.prior_closure === "closed" && event.closure === "open" && UUID.test(event.event_id)
    && /^[1-9][0-9]*$/.test(event.author_action_sequence)
    && BigInt(event.author_action_sequence) > BigInt(closed.author_action_sequence)
    && receipt.schema_id === "storyos.receipt.draft-reopen.v1" && receipt.result === "draft_reopened"
    && UUID.test(receipt.receipt_id) && same(receipt.project_scope, scope)
    && receipt.event_id === event.event_id && receipt.source_close_event_id === closed.event_id
    && receipt.author_undo_receipt_id === event.source.receipt_id && receipt.created_at === event.created_at;
}
function matches(record: DraftUndoRecord, event: EditorFlowDraftReopened): boolean {
  return validDraftReopen({ draft_id: record.source_close.draft_id,
    draft_revision_id: record.source_close.draft_revision_id, payload_digest: record.source_close.payload_digest,
    closure: "open", closure_event: record.source_close, reopen_event: event }, record.project_scope)
    && event.source.idempotency_key === record.idempotency_key && same(event.source.command_digest, record.digest);
}

export async function readDraftUndoJournal(workspace: EditorWorkspace) {
  const metadata = workspace.database.transaction("metadata").objectStore("metadata");
  const [records, observations] = await Promise.all([
    read(metadata.getAll(IDBKeyRange.bound("draft-undo:", "draft-undo:\uffff"), MAX_DISCARD_RECORDS + 1)) as Promise<DraftUndoRecord[]>,
    read(metadata.getAll(IDBKeyRange.bound("draft-undo-observation:", "draft-undo-observation:\uffff"), MAX_DISCARD_RECORDS + 1)) as Promise<Observation[]>,
  ]);
  if (records.length > MAX_DISCARD_RECORDS || observations.length > MAX_DISCARD_RECORDS) throw new Error("Undo Journal limit");
  for (const record of records) {
    const input = record.request?.undo_latest_author_action_input;
    if (record.schema_id !== "storyos.local-edit-journal.draft-undo.v1" || input === undefined
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
    const response = observation.response, event = observation.event;
    if (!record || observation.key !== `draft-undo-observation:${record.source_close.event_id}`
      || (event === undefined && response === undefined)
      || (event !== undefined && !matches(record, event))
      || (response !== undefined && (!same(response.project_scope, record.project_scope)
        || response.correlation_id !== record.request.undo_latest_author_action_input.correlation_id
        || response.receipt.idempotency_key !== record.idempotency_key || !same(response.receipt.command_digest, record.digest)
        || !["draft_compensated", "conflicted", "unavailable"].includes(response.effect.kind)
        || (response.effect.kind === "draft_compensated" && !matches(record, response.effect.event))))) throw new Error("Undo evidence unavailable");
  }
  return records.map((record) => ({ record, observation: observations.find((row) => row.record_key === record.key) }));
}

export async function freezeDraftUndo(workspace: EditorWorkspace, source_close: EditorFlowDraftClosed,
  request: UndoLatestAuthorActionRequest, idempotency_key: string): Promise<DraftUndoRecord> {
  const records = await readDraftUndoJournal(workspace);
  const existing = records.find(({ record }) => record.key === `draft-undo:${source_close.event_id}`);
  if (existing) return existing.record;
  if (records.length >= MAX_DISCARD_RECORDS || records.some(({ observation }) => observation === undefined)) throw new Error("Undo unresolved");
  const digest = await digestUndoLatestAuthorAction(request, workspace.cryptoImpl);
  const transaction = workspace.database.transaction(["metadata", "partitions"], "readwrite", { durability: "strict" });
  const done = committed(transaction), metadata = transaction.objectStore("metadata");
  const sequence = await read(metadata.get("local_intent_sequence")) as { value: number } | undefined;
  const partition = await read(transaction.objectStore("partitions").get(workspace.partition.journal_partition_id));
  const local_intent_sequence = (sequence?.value ?? 0) + 1;
  if (!same(partition, workspace.partition) || !Number.isSafeInteger(local_intent_sequence)) {
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
  value: Pick<Observation, "event" | "response">): Promise<void> {
  const transaction = workspace.database.transaction("metadata", "readwrite", { durability: "strict" });
  const done = committed(transaction), metadata = transaction.objectStore("metadata");
  const key = `draft-undo-observation:${record.source_close.event_id}`;
  if (!same(await read(metadata.get(record.key)), record)) { transaction.abort(); await done; throw new Error("Undo identity changed"); }
  const previous = await read(metadata.get(key)) as Observation | undefined;
  if (previous === undefined) metadata.add({ key, record_key: record.key, ...value });
  else if (value.event && !same(previous.event ?? (previous.response?.effect.kind === "draft_compensated" ? previous.response.effect.event : undefined), value.event)) {
    transaction.abort(); await done; throw new Error("Undo settlement changed");
  }
  await done;
  await readDraftUndoJournal(workspace);
}
export async function reconcileDraftUndo(workspace: EditorWorkspace, draft: RefusedEditDraftInspect) {
  const records = await readDraftUndoJournal(workspace);
  const event = draft.reopen_event;
  if (event == null) return undefined;
  const found = records.find(({ record }) => matches(record, event));
  if (found && found.observation === undefined) await observeDraftUndo(workspace, found.record, { event });
  return found ? event : undefined;
}

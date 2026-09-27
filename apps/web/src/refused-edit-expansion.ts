import { createProjectCommandChallenge, digestExpandRefusedEditDraft, expandRefusedEditDraftToProposal,
  getEditorSession } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ExpandRefusedEditDraftRequest, ExpandRefusedEditDraftResponse, RefusedEditDraftInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace } from "./editor-types.ts";
import { canonicalDraftValue as canonical, MAX_DISCARD_RECORDS, type DiscardRecord } from "./refused-edit-discard.ts";
import { digestJournalValue, rebuildPendingProjection, JOURNAL_DATABASE_VERSION } from "./local-edit-journal.ts";
import { uuidV7 } from "./acceptance-journal.ts";
import type { RetryTargetRead } from "./refused-edit-retry.ts";

type Request = ExpandRefusedEditDraftRequest;
type ExpansionRecord = Omit<DiscardRecord, "command_kind" | "exact_target_head_anchor_bindings" | "author_visible_decision_ref" | "group"> & {
  command_kind: "expandRefusedEditDraftToProposal";
  exact_target_head_anchor_bindings: Request["expand_refused_edit_draft_to_proposal_input"];
  author_visible_decision_ref: { draft_id: string; selected_payload_range: { kind: "whole_draft_payload" } };
  group: Omit<DiscardRecord["group"], "command_kind" | "frozen_request_body"> & {
    command_kind: "expandRefusedEditDraftToProposal"; frozen_request_body: Request };
};
type Observation = { key: string; record_key: string; response: ExpandRefusedEditDraftResponse };
const SCHEMA = "storyos.local-edit-journal.refused-edit-expansion.v1";
const ROUTE = "/api/v1/projects/{project_id}/drafts/{draft_id}/proposal-expansions";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const same = (a: unknown, b: unknown) => canonical(a) === canonical(b);
const keys = (value: unknown, required: string[], optional: string[] = []) => value !== null && typeof value === "object"
  && required.every((key) => Object.hasOwn(value, key)) && Object.keys(value).every((key) => [...required, ...optional].includes(key));
const read = <T,>(request: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
  request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error);
});
const committed = (transaction: IDBTransaction) => new Promise<void>((resolve, reject) => {
  transaction.oncomplete = () => resolve(); transaction.onabort = transaction.onerror = () => reject(transaction.error);
});

async function recordFor(partition: EditorWorkspace["partition"], request: Request,
  identity: { sequence: number; recordId: string; groupId: string; key: string; createdAt: string }, crypto: Crypto): Promise<ExpansionRecord> {
  const { sequence, recordId, groupId, key, createdAt } = identity;
  const input = request.expand_refused_edit_draft_to_proposal_input, digest = await digestExpandRefusedEditDraft(request, crypto);
  const coverage = { ordered_coverage: [{ local_intent_sequence: sequence, intent_record_ref: recordId, payload_digest: digest }],
    covered_sequence_range: { first: sequence, last: sequence } };
  return { key: `expansion:${input.draft_id}:${input.source_reopen_event_id ?? "initial"}`, schema_id: SCHEMA,
    project_scope: partition.project_scope, journal_partition_id: partition.journal_partition_id,
    editor_session_id: partition.editor_session_id, writer_generation: partition.writer_generation,
    explicit_command_record_id: recordId, local_intent_sequence: sequence, created_at: createdAt,
    command_kind: "expandRefusedEditDraftToProposal", editor_contract_revision: "storyos.editor-contract.release-1.v3",
    exact_semantic_payload_ref: groupId, semantic_payload_digest: digest, exact_target_head_anchor_bindings: input,
    author_visible_decision_ref: { draft_id: input.draft_id, selected_payload_range: { kind: "whole_draft_payload" } },
    group: { journal_submission_group_id: groupId, action_class: "explicit_editor_command", api_major: 1, method: "POST",
      route_template: ROUTE, command_kind: "expandRefusedEditDraftToProposal", command_schema: request.command_schema,
      idempotency_key: key, frozen_request_body: request, frozen_request_digest: digest, digest_profile: digest.profile,
      frozen_request_body_ref: groupId, frozen_request_digest_input_ref: groupId, frozen_at: createdAt, ...coverage,
      frozen_payload_coverage_digest: { ...await digestJournalValue(coverage, crypto), profile: "storyos.local-edit-journal.submission-coverage.sha256.v1" } } };
}

function validResponse(record: ExpansionRecord, response: ExpandRefusedEditDraftResponse): boolean {
  const input = record.group.frozen_request_body.expand_refused_edit_draft_to_proposal_input;
  const receipt = response.receipt, effect = response.effect;
  if (!keys(response, ["schema_id", "correlation_id", "project_scope", "command_id", "author_command_admission_id", "receipt", "effect"])
    || !keys(receipt, ["receipt_id", "project_scope", "command_kind", "command_digest", "idempotency_key", "producer_cause",
      "author_command_admission_id", "expected_heads", "prior_heads", "resulting_heads", "authoritative_revision_ids", "proposal_revision_ids",
      "authoritative_commit_ids", "draft_artifact_refs", "artifact_lifecycle_event_refs", "condition_refs", "result", "created_at"], ["author_action_sequence"])
    || response.schema_id !== "storyos.command.expand-refused-edit-draft-to-proposal.response.v1"
    || response.correlation_id !== input.correlation_id || !same(response.project_scope, record.project_scope)
    || ![response.command_id, response.author_command_admission_id, receipt.receipt_id].every((id) => UUID.test(id))
    || !same(receipt.project_scope, record.project_scope) || receipt.command_kind !== record.command_kind
    || receipt.author_command_admission_id !== response.author_command_admission_id || receipt.idempotency_key !== record.group.idempotency_key
    || receipt.producer_cause !== "author_command_admission" || !same(receipt.command_digest, record.semantic_payload_digest)
    || !same(receipt.expected_heads, input.expected_target_revisions) || !same(receipt.resulting_heads, receipt.prior_heads)
    || !same(receipt.draft_artifact_refs, [input.draft_id]) || !Number.isFinite(Date.parse(receipt.created_at))
    || ![receipt.authoritative_revision_ids, receipt.authoritative_commit_ids, receipt.condition_refs].every((refs) => same(refs, []))) return false;
  if (effect.kind !== "proposal_created_from_draft") return receipt.result === effect.kind && receipt.author_action_sequence == null
    && [receipt.proposal_revision_ids, receipt.artifact_lifecycle_event_refs].every((refs) => same(refs, []))
    && (effect.kind === "conflicted" ? UUID.test(effect.current_revision_id) && /^[0-9a-f]{64}$/.test(effect.current_digest)
      && ["open", "closed"].includes(effect.current_closure) : ["source_draft_not_open", "source_unavailable", "unsupported_payload", "target_unavailable"].includes(effect.reason)
        && ["open", "closed"].includes(effect.current_closure));
  const event = effect.event;
  return receipt.result === effect.kind && [effect.proposal_id, effect.proposal_revision_id, event.event_id].every((id) => UUID.test(id))
    && same(receipt.prior_heads, input.expected_target_revisions) && same(receipt.proposal_revision_ids, [effect.proposal_revision_id])
    && same(receipt.artifact_lifecycle_event_refs, [event.event_id]) && /^[1-9][0-9]{0,19}$/.test(receipt.author_action_sequence ?? "")
    && event.schema_id === "storyos.event.editor-flow-draft-closed.v1" && event.event_kind === "editor_flow_draft_closed"
    && same(event.project_scope, record.project_scope) && event.draft_id === input.draft_id
    && event.draft_revision_id === input.source_current_draft_revision_id && event.payload_digest === input.source_draft_payload_digest
    && event.prior_closure === "open" && event.closure === "closed" && event.close_reason === "superseded"
    && event.created_at === receipt.created_at && event.author_action_sequence === receipt.author_action_sequence
    && same(event.source, { command_id: response.command_id, author_command_admission_id: response.author_command_admission_id,
      receipt_id: receipt.receipt_id, idempotency_key: record.group.idempotency_key, command_digest: record.semantic_payload_digest });
}

export async function readExpansionJournal(workspace: EditorWorkspace) {
  const metadata = workspace.database.transaction("metadata").objectStore("metadata");
  const [records, schema] = await Promise.all([
    read(metadata.getAll(IDBKeyRange.bound("expansion:", "expansion:\uffff"), MAX_DISCARD_RECORDS + 1)) as Promise<ExpansionRecord[]>,
    read(metadata.get("schema")) as Promise<{ version: number } | undefined>]);
  const observations = await read(workspace.database.transaction("metadata").objectStore("metadata")
    .getAll(IDBKeyRange.bound("expansion-observation:", "expansion-observation:\uffff"), MAX_DISCARD_RECORDS + 1)) as Observation[];
  if (schema?.version !== JOURNAL_DATABASE_VERSION || records.length > MAX_DISCARD_RECORDS || observations.length > MAX_DISCARD_RECORDS) throw new Error("Expansion Journal limit");
  const result = [];
  for (const record of records) {
    const partition = await read(workspace.database.transaction("partitions").objectStore("partitions").get(record.journal_partition_id)) as EditorWorkspace["partition"];
    const input = record.group.frozen_request_body.expand_refused_edit_draft_to_proposal_input;
    if (!partition || !same(partition.project_scope, workspace.partition.project_scope)
      || record.group.frozen_request_body.command_schema !== "storyos.command.expand-refused-edit-draft-to-proposal.request.v1"
      || input.editor_session_id !== partition.editor_session_id || input.writer_generation !== partition.writer_generation
      || input.client_contract_revision !== partition.client_contract_revision || input.security_policy_revision !== partition.security_policy_revision
      || input.expected_source_draft_closure !== "open" || input.proposal_kind !== "inline_edit" || !same(input.selected_payload_range, { kind: "whole_draft_payload" })
      || input.target_refs.length !== 1 || input.expected_target_revisions.length !== 1 || input.anchors.length !== 1
      || ![record.explicit_command_record_id, record.group.journal_submission_group_id, record.group.idempotency_key,
        input.draft_id, input.source_current_draft_revision_id, input.chapter_id, input.correlation_id, ...input.target_refs, ...input.expected_target_revisions].every((id) => UUID.test(id))
      || !/^[0-9a-f]{64}$/.test(input.source_draft_payload_digest) || !Number.isSafeInteger(record.local_intent_sequence)
      || record.local_intent_sequence < 1 || !Number.isFinite(Date.parse(record.created_at))
      || !same(record, await recordFor(partition, record.group.frozen_request_body, { sequence: record.local_intent_sequence,
        recordId: record.explicit_command_record_id, groupId: record.group.journal_submission_group_id, key: record.group.idempotency_key,
        createdAt: record.created_at }, workspace.cryptoImpl))) throw new Error("Expansion Journal unavailable");
    const observation = observations.find((value) => value.record_key === record.key);
    if (observation && (!same(Object.keys(observation).sort(), ["key", "record_key", "response"])
      || observation.key !== `expansion-observation:${record.key}` || !validResponse(record, observation.response))) throw new Error("Expansion observation unavailable");
    result.push({ record, observation });
  }
  if (observations.some((value) => !records.some((record) => record.key === value.record_key))) throw new Error("Expansion observation source unavailable");
  return result;
}

export async function retryExpansion(workspace: EditorWorkspace, record: ExpansionRecord, baseUrl: string,
  fetchImpl: typeof fetch, isCurrent = () => true) {
  const guarded: typeof fetch = (input, init) => { if (!isCurrent()) throw new Error("Expansion view changed"); return fetchImpl(input, init); };
  const group = record.group, projectId = record.project_scope.project_id;
  const challenge = await createProjectCommandChallenge({ baseUrl, projectId, fetchImpl: guarded, request: {
    method: "POST", route_template: ROUTE, command_schema: group.command_schema,
    canonical_command_digest: group.frozen_request_digest, idempotency_key: group.idempotency_key } });
  const response = await expandRefusedEditDraftToProposal({ baseUrl, projectId, draftId: record.author_visible_decision_ref.draft_id,
    fetchImpl: guarded, request: group.frozen_request_body, idempotencyKey: group.idempotency_key, antiForgery: challenge.nonce });
  if (!validResponse(record, response)) throw new Error("Expansion settlement unavailable");
  const transaction = workspace.database.transaction("metadata", "readwrite", { durability: "strict" }), done = committed(transaction);
  const metadata = transaction.objectStore("metadata"), key = `expansion-observation:${record.key}`;
  const previous = await read(metadata.get(key)) as Observation | undefined;
  if (previous && !same(previous.response, response)) { transaction.abort(); await done; throw new Error("Expansion settlement changed"); }
  metadata.put({ key, record_key: record.key, response });
  await done; return response;
}

export async function expandWholeDraft(workspace: EditorWorkspace, draft: RefusedEditDraftInspect, targetIndex: string,
  from: number, to: number, target: RetryTargetRead, baseUrl: string, fetchImpl: typeof fetch, isCurrent: () => boolean) {
  return navigator.locks.request(`storyos-expansion:${workspace.partition.project_scope.project_id}`, async () => {
    const partition = workspace.partition, scope = partition.project_scope;
    const pending = await rebuildPendingProjection(workspace), journal = await readExpansionJournal(workspace);
    const current = await getEditorSession({ baseUrl, projectId: scope.project_id, editorSessionId: partition.editor_session_id, fetchImpl });
    const source = draft.payload.author_edit_units[0]?.selection_snapshot?.ordered_selection?.sources[Number(targetIndex)];
    const block = source?.owner.kind === "manuscript" ? current.base_snapshot.materialized_revision.blocks
      .find((item) => item.manuscript_block_id === source.owner.manuscript_block_id) : undefined;
    if (!isCurrent() || journal.length >= MAX_DISCARD_RECORDS || pending.save_state !== "saved" || pending.unsettled_intent_count !== 0
      || current.writer.kind !== "current_writer" || current.writer.writer_generation !== partition.writer_generation
      || !same(current.editor_session, workspace.session.editor_session) || !same(current.project_scope, scope)
      || current.base_snapshot.authoritative_head_revision_id !== target.authoritativeHead || !block || block.text !== target.text
      || draft.closure !== "open" || draft.retention_state !== "retained" || !Number.isInteger(from) || !Number.isInteger(to) || from < 0 || from >= to || to > block.text.length) throw new Error("Select a current manuscript range for the whole Draft");
    const anchor = { manuscript_block_id: block.manuscript_block_id, base_authoritative_revision_id: target.authoritativeHead,
      manuscript_schema_version: 1, coordinate_profile: "prosemirror-token-utf16.v1", from, to, boundary_profile: "exclusive-authoritative-edges.v1",
      base_slice_digest: `sha256:${(await digestJournalValue(JSON.parse(canonical({ manuscript_block_id: block.manuscript_block_id, block_kind: block.block_kind,
        manuscript_schema_version: 1, coordinate_profile: "prosemirror-token-utf16.v1", from, to, base_slice: block.text.slice(from, to) })), workspace.cryptoImpl)).value_hex_lowercase}` };
    const request: Request = { command_schema: "storyos.command.expand-refused-edit-draft-to-proposal.request.v1", expand_refused_edit_draft_to_proposal_input: {
      draft_id: draft.draft_id, source_current_draft_revision_id: draft.draft_revision_id, source_draft_payload_digest: draft.payload_digest,
      ...(draft.reopen_event ? { source_reopen_event_id: draft.reopen_event.event_id } : {}), expected_source_draft_closure: "open",
      selected_payload_range: { kind: "whole_draft_payload" }, proposal_kind: "inline_edit", chapter_id: draft.payload.chapter_id,
      target_refs: [block.manuscript_block_id], expected_target_revisions: [target.authoritativeHead], anchors: [anchor],
      editor_session_id: partition.editor_session_id, writer_generation: partition.writer_generation,
      client_contract_revision: partition.client_contract_revision, security_policy_revision: partition.security_policy_revision,
      correlation_id: uuidV7(workspace.cryptoImpl) } };
    const old = await read(workspace.database.transaction("metadata").objectStore("metadata").get("local_intent_sequence")) as { value: number } | undefined;
    const record = await recordFor(partition, request, { sequence: (old?.value ?? 0) + 1, recordId: uuidV7(workspace.cryptoImpl),
      groupId: uuidV7(workspace.cryptoImpl), key: uuidV7(workspace.cryptoImpl), createdAt: new Date().toISOString() }, workspace.cryptoImpl);
    const transaction = workspace.database.transaction(["metadata", "partitions"], "readwrite", { durability: "strict" }), done = committed(transaction);
    const metadata = transaction.objectStore("metadata");
    const latest = await read(metadata.get("local_intent_sequence"));
    const stored = await read(transaction.objectStore("partitions").get(partition.journal_partition_id));
    if (!isCurrent() || !same(latest, old) || !same(stored, partition)) { transaction.abort(); await done; throw new Error("Expansion writer changed"); }
    metadata.add(record); metadata.put({ key: "local_intent_sequence", value: record.local_intent_sequence });
    await done; await readExpansionJournal(workspace);
    return retryExpansion(workspace, record, baseUrl, fetchImpl, isCurrent);
  });
}

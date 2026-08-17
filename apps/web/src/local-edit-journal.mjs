import { digestApplyAuthorEdit }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export const JOURNAL_DATABASE_VERSION = 2;
export const EDITOR_CONTRACT_REVISION = "storyos.editor-contract.release-1.v2";
export const AUTHOR_EDIT_BATCH_POLICY_REVISION =
  "storyos.author-edit-batch.release-1.preview.v1";
export const AUTHOR_EDIT_BATCH_IDLE_MS = 250;
export const AUTHOR_EDIT_MAX_UNITS = 240;
export const AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES = 240;
export const AUTHOR_EDIT_MAX_WIRE_BODY_BYTES = 1024 * 1024;

const MAX_RETAINED_JOURNAL_ITEMS = 2400;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const INPUT_ORIGINS = new Set([
  "typing",
  "deletion",
  "selection_replacement",
  "paste",
  "cut",
  "composition_confirmation",
]);
const HARD_BOUNDARY_INPUT_ORIGINS = new Set([
  "paste",
  "cut",
  "composition_confirmation",
]);

export function mayAppendJournalSubmissionRecord(first, prior, next) {
  const elapsed = Date.parse(next.created_at) - Date.parse(prior.created_at);
  return !HARD_BOUNDARY_INPUT_ORIGINS.has(first.input_origin)
    && !HARD_BOUNDARY_INPUT_ORIGINS.has(next.input_origin)
    && elapsed >= 0
    && elapsed <= AUTHOR_EDIT_BATCH_IDLE_MS
    && first.journal_partition_id === next.journal_partition_id
    && first.editor_session_id === next.editor_session_id
    && first.writer_generation === next.writer_generation
    && first.chapter_object_id === next.chapter_object_id
    && first.base_snapshot_id === next.base_snapshot_id
    && first.base_activity_position === next.base_activity_position
    && first.limit_profile_revision === next.limit_profile_revision
    && first.batch_policy_revision === next.batch_policy_revision
    && first.editor_contract_revision === next.editor_contract_revision
    && first.observed_ownership_partition === next.observed_ownership_partition
    && first.undo_group_binding.undo_group_id === next.undo_group_binding.undo_group_id
    && JSON.stringify(first.project_scope) === JSON.stringify(next.project_scope)
    && JSON.stringify(first.target_refs) === JSON.stringify(next.target_refs)
    && JSON.stringify(first.expected_authoritative_heads)
      === JSON.stringify(next.expected_authoritative_heads)
    && JSON.stringify(first.expected_proposal_heads)
      === JSON.stringify(next.expected_proposal_heads)
    && JSON.stringify(first.proposal_anchors) === JSON.stringify(next.proposal_anchors)
    && JSON.stringify(first.retry_source) === JSON.stringify(next.retry_source);
}

const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction) => new Promise((resolve, reject) => {
  transaction.oncomplete = resolve;
  transaction.onabort = () => reject(
    transaction.error ?? new Error("IndexedDB transaction aborted"),
  );
  transaction.onerror = () => reject(
    transaction.error ?? new Error("IndexedDB transaction failed"),
  );
});

export function createJournalUuid(cryptoImpl = globalThis.crypto, now = Date.now()) {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6] & 0x0f) | 0x70;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export async function digestJournalValue(value, cryptoImpl) {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  return {
    algorithm: "sha256",
    profile: "storyos.local-edit-journal.payload.sha256.v1",
    value_hex_lowercase: [...digest]
      .map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

async function digestSubmissionCoverage(coverage, cryptoImpl) {
  const bytes = new TextEncoder().encode(JSON.stringify(coverage));
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  return {
    algorithm: "sha256",
    profile: "storyos.local-edit-journal.submission-coverage.sha256.v1",
    value_hex_lowercase: [...digest]
      .map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

function replaceSelection(body, primitive) {
  if (primitive?.kind !== "replace_selection"
    || !Number.isSafeInteger(primitive.from)
    || !Number.isSafeInteger(primitive.to)
    || primitive.from < 0
    || primitive.to < primitive.from
    || primitive.to > body.length
    || typeof primitive.text !== "string") {
    throw new Error("Local Edit Journal reconstruction failed");
  }
  return `${body.slice(0, primitive.from)}${primitive.text}${body.slice(primitive.to)}`;
}

function isAppliedSettlement(group) {
  return group?.settlement?.kind === "applied_receipt_settled"
    || group?.settlement?.kind === "receipt_settled";
}

function isZeroAuthoritySettlement(group) {
  return group?.settlement?.kind === "zero_authority_receipt_settled";
}

export async function readJournalSnapshot(workspace) {
  const transaction = workspace.database.transaction(
    ["metadata", "payload_chains", "intents", "submission_groups"],
    "readonly",
  );
  const partitionId = workspace.partition.journal_partition_id;
  const [schema, watermark, activeBase, records, payloadChains, groups] = await Promise.all([
    requestResult(transaction.objectStore("metadata").get("schema")),
    requestResult(transaction.objectStore("metadata")
      .get(`durable_high_watermark:${partitionId}`)),
    requestResult(transaction.objectStore("metadata").get(`active_base:${partitionId}`)),
    requestResult(transaction.objectStore("intents").index("partition")
      .getAll(partitionId, MAX_RETAINED_JOURNAL_ITEMS + 1)),
    requestResult(transaction.objectStore("payload_chains").index("partition")
      .getAll(partitionId, MAX_RETAINED_JOURNAL_ITEMS + 1)),
    requestResult(transaction.objectStore("submission_groups").index("partition")
      .getAll(partitionId, MAX_RETAINED_JOURNAL_ITEMS + 1)),
  ]);
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || records.length > MAX_RETAINED_JOURNAL_ITEMS
    || payloadChains.length > MAX_RETAINED_JOURNAL_ITEMS
    || groups.length > MAX_RETAINED_JOURNAL_ITEMS) {
    throw new Error("Local Edit Journal exceeds this Editor Session scope");
  }
  records.sort((left, right) => left.local_intent_sequence - right.local_intent_sequence);
  groups.sort((left, right) => left.covered_sequence_range.first
    - right.covered_sequence_range.first);
  return { watermark, activeBase: activeBase?.value, records, payloadChains, groups };
}

async function validatePayloadChains(workspace, records, payloadChains) {
  const recordsByChain = new Map();
  for (const record of records) {
    const chainRecords = recordsByChain.get(record.payload_chain_ref) ?? [];
    chainRecords.push(record);
    recordsByChain.set(record.payload_chain_ref, chainRecords);
  }
  const bodyBySequence = new Map();
  for (const chain of payloadChains) {
    const chainRecords = recordsByChain.get(chain.payload_chain_id) ?? [];
    const patches = chain.ordered_patch_refs ?? [];
    if (chain.journal_partition_id !== workspace.partition.journal_partition_id
      || chainRecords.length === 0
      || patches.length !== chainRecords.length
      || patches.length > AUTHOR_EDIT_MAX_UNITS) {
      throw new Error("Local Edit Journal is corrupt");
    }
    let body = chain.checkpoint_ref?.materialized_payload;
    if (typeof body !== "string"
      || new TextEncoder().encode(body).byteLength > workspace.maxJsonStringUtf8Bytes) {
      throw new Error("Local Edit Journal is corrupt");
    }
    for (const [index, record] of chainRecords.entries()) {
      const patchRef = patches[index];
      const [primitive] = patchRef?.normalized_primitives ?? [];
      const expectedDigest = await digestJournalValue(record.author_edit_unit, workspace.cryptoImpl);
      if (record.base_snapshot_id !== chain.checkpoint_ref.source_snapshot_id
        || record.chapter_object_id !== chain.checkpoint_ref.chapter_object_id
        || JSON.stringify(record.expected_authoritative_heads)
          !== JSON.stringify(chain.checkpoint_ref.source_heads)
        || patchRef?.completed_intent_record_id !== record.completed_intent_record_id
        || patchRef?.local_intent_sequence !== record.local_intent_sequence
        || JSON.stringify(patchRef?.normalized_primitives)
          !== JSON.stringify(record.author_edit_unit.normalized_primitives)
        || JSON.stringify(record.payload_digest) !== JSON.stringify(expectedDigest)
        || record.author_edit_unit.normalized_primitives.length !== 1) {
        throw new Error("Local Edit Journal is corrupt");
      }
      body = replaceSelection(body, primitive);
      if (new TextEncoder().encode(body).byteLength > workspace.maxJsonStringUtf8Bytes
        || JSON.stringify(await digestJournalValue(body, workspace.cryptoImpl))
          !== JSON.stringify(patchRef.resulting_payload_digest)) {
        throw new Error("Local Edit Journal is corrupt");
      }
      bodyBySequence.set(record.local_intent_sequence, body);
    }
    recordsByChain.delete(chain.payload_chain_id);
  }
  if (recordsByChain.size !== 0) throw new Error("Local Edit Journal is corrupt");
  return bodyBySequence;
}

async function validateCoverage(workspace, records, groups) {
  const recordsBySequence = new Map(records.map((record) => [record.local_intent_sequence, record]));
  const covered = new Set();
  let nextCoveredSequence = 1;
  for (const group of groups) {
    const first = group.covered_sequence_range?.first;
    const last = group.covered_sequence_range?.last;
    const coverage = group.ordered_coverage ?? [];
    if (group.journal_partition_id !== workspace.partition.journal_partition_id
      || group.batch_policy_revision !== AUTHOR_EDIT_BATCH_POLICY_REVISION
      || !["unsettled", "applied_receipt_settled", "zero_authority_receipt_settled"]
        .includes(group.settlement?.kind)
      || !Number.isSafeInteger(first)
      || !Number.isSafeInteger(last)
      || first !== nextCoveredSequence
      || first > last
      || coverage.length !== last - first + 1
      || coverage.length > AUTHOR_EDIT_MAX_UNITS) {
      throw new Error("Journal Submission Group is corrupt");
    }
    const firstRecord = recordsBySequence.get(first);
    const request = group.frozen_request_body;
    const coveredRecords = coverage.map((_, index) => recordsBySequence.get(first + index));
    const [requestDigest, coverageDigest] = await Promise.all([
      digestApplyAuthorEdit(request, workspace.cryptoImpl),
      digestSubmissionCoverage({ ordered_coverage: coverage,
        covered_sequence_range: group.covered_sequence_range }, workspace.cryptoImpl),
    ]);
    if (!UUID.test(group.journal_submission_group_id ?? "")
      || JSON.stringify(group.project_scope) !== JSON.stringify(workspace.partition.project_scope)
      || group.editor_session_id !== workspace.partition.editor_session_id
      || group.writer_generation !== workspace.partition.writer_generation
      || group.action_class !== "direct_editor_action"
      || group.api_major !== 1
      || group.method !== "POST"
      || group.route_template !== "/api/v1/projects/{project_id}/manuscript/author-edits"
      || group.command_schema !== "storyos.command.apply-author-edit.request.v1"
      || group.command_kind !== "applyAuthorEdit"
      || group.digest_profile !== "storyos.command.applyAuthorEdit.jcs.v1"
      || !UUID.test(group.idempotency_key ?? "")
      || request?.command_schema !== group.command_schema
      || request?.client_contract_revision !== workspace.partition.client_contract_revision
      || request?.security_policy_revision !== workspace.partition.security_policy_revision
      || request?.editor_session_id !== workspace.partition.editor_session_id
      || request?.writer_generation !== workspace.partition.writer_generation
      || request?.chapter_id !== firstRecord?.chapter_object_id
      || request?.expected_authoritative_revision_id
        !== firstRecord?.expected_authoritative_heads[0]
      || JSON.stringify(request?.expected_proposal_head_revision_ids)
        !== JSON.stringify(firstRecord?.expected_proposal_heads)
      || JSON.stringify(request?.target_refs) !== JSON.stringify(firstRecord?.target_refs)
      || request?.observed_ownership_partition !== firstRecord?.observed_ownership_partition
      || request?.undo_group_id !== firstRecord?.undo_group_binding.undo_group_id
      || request?.editor_contract_revision !== EDITOR_CONTRACT_REVISION
      || request?.completed_intent_record_id !== firstRecord?.completed_intent_record_id
      || request?.local_intent_sequence !== String(first)
      || JSON.stringify(request?.author_edit_units)
        !== JSON.stringify(coveredRecords.map((record) => record?.author_edit_unit))
      || JSON.stringify(group.frozen_request_digest) !== JSON.stringify(requestDigest)
      || JSON.stringify(group.frozen_payload_coverage_digest) !== JSON.stringify(coverageDigest)
      || new TextEncoder().encode(JSON.stringify(request)).byteLength
        > AUTHOR_EDIT_MAX_WIRE_BODY_BYTES) {
      throw new Error("Journal Submission Group is corrupt");
    }
    for (const [index, item] of coverage.entries()) {
      const sequence = first + index;
      const record = recordsBySequence.get(sequence);
      if (covered.has(sequence)
        || item.local_intent_sequence !== sequence
        || item.intent_record_ref !== record?.completed_intent_record_id
        || JSON.stringify(item.payload_digest) !== JSON.stringify(record?.payload_digest)
        || (index > 0 && !mayAppendJournalSubmissionRecord(
          firstRecord, coveredRecords[index - 1], record,
        ))) {
        throw new Error("Journal Submission Group is corrupt");
      }
      covered.add(sequence);
    }
    nextCoveredSequence = last + 1;
  }
  return covered;
}

export async function validateJournalSnapshot(workspace, snapshot) {
  const { records, payloadChains, groups, watermark, activeBase } = snapshot;
  let priorSequence = 0;
  for (const record of records) {
    if (record.journal_partition_id !== workspace.partition.journal_partition_id
      || record.editor_session_id !== workspace.partition.editor_session_id
      || record.writer_generation !== workspace.partition.writer_generation
      || record.limit_profile_revision !== workspace.partition.limit_profile_revision
      || JSON.stringify(record.project_scope) !== JSON.stringify(workspace.partition.project_scope)
      || record.local_intent_sequence !== priorSequence + 1
      || record.projection_dependency?.prior_sequence !== record.local_intent_sequence - 1
      || record.projection_dependency?.snapshot_id !== record.base_snapshot_id
      || record.retry_source?.kind !== "fresh_editor_intent"
      || record.editor_contract_revision !== EDITOR_CONTRACT_REVISION
      || record.batch_policy_revision !== AUTHOR_EDIT_BATCH_POLICY_REVISION
      || record.undo_group_binding?.kind !== "direct_author_input"
      || !UUID.test(record.undo_group_binding?.undo_group_id ?? "")
      || !INPUT_ORIGINS.has(record.input_origin)
      || typeof record.created_at !== "string"
      || Number.isNaN(Date.parse(record.created_at))
      || new Date(Date.parse(record.created_at)).toISOString() !== record.created_at
      || JSON.stringify(record.proposal_anchors) !== JSON.stringify([])) {
      throw new Error("Local Edit Journal is corrupt");
    }
    priorSequence = record.local_intent_sequence;
  }
  if ((records.length === 0 && watermark !== undefined)
    || (records.length > 0 && watermark?.value !== records.at(-1).local_intent_sequence)) {
    throw new Error("Local Edit Journal is corrupt");
  }
  if (activeBase !== undefined
    && JSON.stringify(activeBase) !== JSON.stringify(workspace.session.base_snapshot)) {
    throw new Error("Local Edit Journal active base is corrupt");
  }
  const bodyBySequence = await validatePayloadChains(workspace, records, payloadChains);
  const covered = await validateCoverage(workspace, records, groups);
  const pendingRecords = records.filter((record) => !covered.has(record.local_intent_sequence));
  if (pendingRecords.some((record) =>
    record.base_snapshot_id !== workspace.session.base_snapshot.snapshot_id)
    || pendingRecords.some((record, index) => index > 0
      && !mayAppendJournalSubmissionRecord(
        pendingRecords[0], pendingRecords[index - 1], record,
      ))) {
    throw new Error("Local Edit Journal is corrupt");
  }
  return { ...snapshot, bodyBySequence, covered };
}

export async function rebuildPendingProjection(workspace) {
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const appliedSequences = new Set();
  let hasAppliedSettlement = false;
  let hasZeroAuthoritySettlement = false;
  for (const group of snapshot.groups) {
    if (isAppliedSettlement(group)) {
      hasAppliedSettlement = true;
      for (const item of group.ordered_coverage) appliedSequences.add(item.local_intent_sequence);
    } else if (isZeroAuthoritySettlement(group)) {
      hasZeroAuthoritySettlement = true;
    }
  }
  const base = workspace.session.base_snapshot;
  const activeRecords = snapshot.records.filter((record) =>
    record.base_snapshot_id === base.snapshot_id && !appliedSequences.has(record.local_intent_sequence));
  const body = activeRecords.length === 0
    ? base.materialized_revision.body
    : snapshot.bodyBySequence.get(activeRecords.at(-1).local_intent_sequence);
  return {
    body,
    save_state: hasZeroAuthoritySettlement
      ? "needs_attention"
      : activeRecords.length
        ? "saving"
        : hasAppliedSettlement ? "saved" : "clean",
    unsettled_intent_count: activeRecords.length,
    authoritative_revision_id: base.authoritative_head_revision_id,
  };
}

export async function persistReplaceSelection(
  workspace,
  edit,
  cryptoImpl = globalThis.crypto,
) {
  if (workspace.partition.disposition !== "current_writer_open") {
    throw new Error("Editor Session is read only");
  }
  const createdAt = edit.createdAt ?? new Date().toISOString();
  const inputOrigin = edit.inputOrigin ?? "typing";
  const undoGroupId = edit.undoGroupId ?? createJournalUuid(cryptoImpl);
  const projection = await rebuildPendingProjection(workspace);
  const expectedBody = replaceSelection(projection.body, {
    kind: "replace_selection", from: edit.from, to: edit.to, text: edit.text,
  });
  if (!INPUT_ORIGINS.has(inputOrigin)
    || !UUID.test(undoGroupId)
    || typeof createdAt !== "string"
    || Number.isNaN(Date.parse(createdAt))
    || new Date(Date.parse(createdAt)).toISOString() !== createdAt
    || typeof edit.resultingBody !== "string"
    || edit.resultingBody !== expectedBody
    || new TextEncoder().encode(edit.text).byteLength > workspace.maxJsonStringUtf8Bytes
    || new TextEncoder().encode(edit.resultingBody).byteLength
      > workspace.maxJsonStringUtf8Bytes
    || projection.save_state === "needs_attention") {
    throw new Error("Local Edit Journal limit failed");
  }
  const base = workspace.session.base_snapshot;
  const completedIntentRecordId = createJournalUuid(cryptoImpl);
  const patchId = createJournalUuid(cryptoImpl);
  const authorEditUnit = {
    normalized_primitives: [{
      kind: "replace_selection", from: edit.from, to: edit.to, text: edit.text,
    }],
    selection_snapshot: {
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: edit.from, to: edit.to,
    },
  };
  const [payloadDigest, resultingPayloadDigest] = await Promise.all([
    digestJournalValue(authorEditUnit, cryptoImpl),
    digestJournalValue(edit.resultingBody, cryptoImpl),
  ]);
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "payload_chains", "intents", "submission_groups"],
    "readwrite",
  );
  const metadata = transaction.objectStore("metadata");
  const intents = transaction.objectStore("intents");
  const partitionId = workspace.partition.journal_partition_id;
  const [schema, partition, current, activeBase, durableRecords, chains, groups] =
    await Promise.all([
      requestResult(metadata.get("schema")),
      requestResult(transaction.objectStore("partitions").get(partitionId)),
      requestResult(metadata.get("local_intent_sequence")),
      requestResult(metadata.get(`active_base:${partitionId}`)),
      requestResult(intents.index("partition")
        .getAll(partitionId, MAX_RETAINED_JOURNAL_ITEMS + 1)),
      requestResult(transaction.objectStore("payload_chains").index("partition")
        .getAll(partitionId, MAX_RETAINED_JOURNAL_ITEMS + 1)),
      requestResult(transaction.objectStore("submission_groups").index("partition")
        .getAll(partitionId, MAX_RETAINED_JOURNAL_ITEMS + 1)),
    ]);
  const hasUnsettledGroup = groups.some((group) => group.settlement?.kind === "unsettled");
  const coveredSequences = new Set(groups.flatMap((group) =>
    (group.ordered_coverage ?? []).map((coverage) => coverage.local_intent_sequence)));
  const priorRecord = durableRecords.filter((record) =>
    record.base_snapshot_id === base.snapshot_id
      && !coveredSequences.has(record.local_intent_sequence))
    .sort((left, right) => left.local_intent_sequence - right.local_intent_sequence).at(-1);
  const adjacentElapsed = priorRecord
    ? Date.parse(createdAt) - Date.parse(priorRecord.created_at)
    : 0;
  if (priorRecord && (adjacentElapsed < 0
    || adjacentElapsed > AUTHOR_EDIT_BATCH_IDLE_MS
    || HARD_BOUNDARY_INPUT_ORIGINS.has(priorRecord.input_origin)
    || HARD_BOUNDARY_INPUT_ORIGINS.has(inputOrigin)
    || priorRecord.undo_group_binding.undo_group_id !== undoGroupId)) {
    transaction.abort();
    throw new Error("Local Edit Journal requires prior group settlement");
  }
  const sequence = (current?.value ?? 0) + 1;
  if (schema?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || JSON.stringify(activeBase?.value) !== JSON.stringify(base)
    || durableRecords.length >= MAX_RETAINED_JOURNAL_ITEMS
    || chains.length >= MAX_RETAINED_JOURNAL_ITEMS
    || groups.length > MAX_RETAINED_JOURNAL_ITEMS
    || hasUnsettledGroup
    || !Number.isSafeInteger(sequence)) {
    transaction.abort();
    throw new Error("Local Edit Journal schema or partition is incompatible");
  }
  let payloadChain = chains.find((chain) =>
    chain.checkpoint_ref?.source_snapshot_id === base.snapshot_id);
  if (chains.filter((chain) => chain.checkpoint_ref?.source_snapshot_id === base.snapshot_id).length > 1
    || (payloadChain?.ordered_patch_refs.length ?? 0) >= AUTHOR_EDIT_MAX_UNITS) {
    transaction.abort();
    throw new Error("Local Edit Journal limit failed");
  }
  const isNewChain = !payloadChain;
  if (isNewChain) {
    payloadChain = {
      payload_chain_id: createJournalUuid(cryptoImpl),
      journal_partition_id: partitionId,
      checkpoint_ref: {
        chapter_object_id: base.chapter_id,
        materialized_payload: base.materialized_revision.body,
        materialized_payload_digest: base.materialized_payload_digest,
        source_snapshot_id: base.snapshot_id,
        source_heads: [base.authoritative_head_revision_id],
      },
      ordered_patch_refs: [],
    };
  }
  payloadChain.ordered_patch_refs.push({
    patch_id: patchId,
    completed_intent_record_id: completedIntentRecordId,
    local_intent_sequence: sequence,
    normalized_primitives: authorEditUnit.normalized_primitives,
    resulting_payload_digest: resultingPayloadDigest,
  });
  const record = {
    completed_intent_record_id: completedIntentRecordId,
    local_intent_sequence: sequence,
    journal_partition_id: partitionId,
    project_scope: workspace.partition.project_scope,
    editor_session_id: workspace.partition.editor_session_id,
    writer_generation: workspace.partition.writer_generation,
    limit_profile_revision: workspace.partition.limit_profile_revision,
    batch_policy_revision: AUTHOR_EDIT_BATCH_POLICY_REVISION,
    input_origin: inputOrigin,
    chapter_object_id: base.chapter_id,
    base_snapshot_id: base.snapshot_id,
    base_activity_position: base.project_activity_position,
    target_refs: base.target_refs,
    expected_authoritative_heads: [base.authoritative_head_revision_id],
    expected_proposal_heads: base.proposal_head_revision_ids,
    proposal_anchors: [],
    observed_ownership_partition: base.observed_ownership_partition,
    author_edit_unit: authorEditUnit,
    retry_source: { kind: "fresh_editor_intent" },
    editor_contract_revision: EDITOR_CONTRACT_REVISION,
    undo_group_binding: { kind: "direct_author_input", undo_group_id: undoGroupId },
    payload_chain_ref: payloadChain.payload_chain_id,
    payload_digest: payloadDigest,
    projection_dependency: { snapshot_id: base.snapshot_id, prior_sequence: sequence - 1 },
    created_at: createdAt,
  };
  metadata.put({ key: "local_intent_sequence", value: sequence });
  metadata.put({ key: `durable_high_watermark:${partitionId}`, value: sequence });
  if (isNewChain) transaction.objectStore("payload_chains").add(payloadChain);
  else transaction.objectStore("payload_chains").put(payloadChain);
  intents.add(record);
  await transactionResult(transaction);
  return rebuildPendingProjection(workspace);
}

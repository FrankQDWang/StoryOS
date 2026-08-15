import {
  applyAuthorEdit,
  createProjectCommandChallenge,
  digestApplyAuthorEdit,
  getChapter,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { rebuildPendingProjection } from "./editor-session.mjs";

const DATABASE_VERSION = 2;
const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const boundedU64 = (value) => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;
const positiveU64 = (value) => boundedU64(value) && BigInt(value) > 0n;

const requestResult = (request) => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction) => new Promise((resolve, reject) => {
  transaction.oncomplete = resolve;
  transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
  transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"));
});

function uuidV7(cryptoImpl = globalThis.crypto, now = Date.now()) {
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

async function validateFrozenGroup(group, workspace, record, cryptoImpl) {
  const request = group.frozen_request_body;
  const [requestDigest, coverageDigest] = await Promise.all([
    digestApplyAuthorEdit(group.frozen_request_body, cryptoImpl),
    digestSubmissionCoverage({ ordered_coverage: group.ordered_coverage,
      covered_sequence_range: group.covered_sequence_range }, cryptoImpl),
  ]);
  if (group.journal_partition_id !== workspace.partition.journal_partition_id
    || !UUID.test(group.journal_submission_group_id ?? "")
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
    || group.settlement?.kind !== "unsettled"
    || group.ordered_coverage?.length !== 1
    || group.ordered_coverage[0].intent_record_ref !== record.completed_intent_record_id
    || group.covered_sequence_range?.first !== record.local_intent_sequence
    || group.covered_sequence_range?.last !== record.local_intent_sequence
    || JSON.stringify(group.ordered_coverage[0].payload_digest)
      !== JSON.stringify(record.payload_digest)
    || request?.command_schema !== group.command_schema
    || request?.editor_session_id !== workspace.partition.editor_session_id
    || request?.writer_generation !== workspace.partition.writer_generation
    || request?.chapter_id !== record.chapter_object_id
    || request?.expected_authoritative_revision_id !== record.expected_authoritative_heads[0]
    || JSON.stringify(request?.expected_proposal_head_revision_ids)
      !== JSON.stringify(record.expected_proposal_heads)
    || JSON.stringify(request?.target_refs) !== JSON.stringify(record.target_refs)
    || request?.observed_ownership_partition !== record.observed_ownership_partition
    || request?.undo_group_id !== record.undo_group_binding.undo_group_id
    || request?.completed_intent_record_id !== record.completed_intent_record_id
    || request?.local_intent_sequence !== String(record.local_intent_sequence)
    || JSON.stringify(request?.author_edit_units) !== JSON.stringify([record.author_edit_unit])
    || JSON.stringify(group.frozen_request_digest) !== JSON.stringify(requestDigest)
    || JSON.stringify(group.frozen_payload_coverage_digest) !== JSON.stringify(coverageDigest)) {
    throw new Error("Journal Submission Group is corrupt");
  }
}

export async function freezeOneIntentSubmission(workspace, cryptoImpl = globalThis.crypto) {
  if (workspace.partition.disposition !== "current_writer_open") {
    throw new Error("Editor Session is read only");
  }
  await rebuildPendingProjection(workspace);
  const snapshot = workspace.database
    .transaction(["intents", "payload_chains", "submission_groups"], "readonly");
  const [records, payloadChains, existingGroups] = await Promise.all([
    requestResult(snapshot.objectStore("intents").index("partition")
      .getAll(workspace.partition.journal_partition_id, 2)),
    requestResult(snapshot.objectStore("payload_chains").index("partition")
      .getAll(workspace.partition.journal_partition_id, 2)),
    requestResult(snapshot.objectStore("submission_groups").index("partition")
      .getAll(workspace.partition.journal_partition_id, 2)),
  ]);
  if (records.length !== 1 || payloadChains.length !== 1 || existingGroups.length > 1) {
    throw new Error("One pending Author Edit is required");
  }
  const [record] = records;
  const [payloadChain] = payloadChains;
  if (payloadChain.payload_chain_id !== record.payload_chain_ref) {
    throw new Error("Local Edit Journal is corrupt");
  }
  if (existingGroups.length === 1) {
    const [existing] = existingGroups;
    await validateFrozenGroup(existing, workspace, record, cryptoImpl);
    return existing;
  }
  const request = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    client_contract_revision: workspace.partition.client_contract_revision,
    security_policy_revision: workspace.partition.security_policy_revision,
    correlation_id: uuidV7(cryptoImpl),
    editor_session_id: workspace.partition.editor_session_id,
    writer_generation: workspace.partition.writer_generation,
    chapter_id: record.chapter_object_id,
    expected_authoritative_revision_id: record.expected_authoritative_heads[0],
    expected_proposal_head_revision_ids: record.expected_proposal_heads,
    target_refs: record.target_refs,
    observed_ownership_partition: record.observed_ownership_partition,
    editor_contract_revision: record.editor_contract_revision,
    undo_group_id: record.undo_group_binding.undo_group_id,
    completed_intent_record_id: record.completed_intent_record_id,
    local_intent_sequence: String(record.local_intent_sequence),
    author_edit_units: [record.author_edit_unit],
  };
  const orderedCoverage = [{ local_intent_sequence: record.local_intent_sequence,
    intent_record_ref: record.completed_intent_record_id, payload_digest: record.payload_digest }];
  const coveredSequenceRange = {
    first: record.local_intent_sequence, last: record.local_intent_sequence,
  };
  const [frozenRequestDigest, frozenPayloadCoverageDigest] = await Promise.all([
    digestApplyAuthorEdit(request, cryptoImpl),
    digestSubmissionCoverage({ ordered_coverage: orderedCoverage,
      covered_sequence_range: coveredSequenceRange }, cryptoImpl),
  ]);
  const group = {
    journal_submission_group_id: uuidV7(cryptoImpl),
    journal_partition_id: workspace.partition.journal_partition_id,
    project_scope: workspace.partition.project_scope,
    editor_session_id: workspace.partition.editor_session_id,
    writer_generation: workspace.partition.writer_generation,
    ordered_coverage: orderedCoverage,
    covered_sequence_range: coveredSequenceRange,
    action_class: "direct_editor_action",
    api_major: 1,
    method: "POST",
    route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
    command_schema: request.command_schema,
    command_kind: "applyAuthorEdit",
    digest_profile: "storyos.command.applyAuthorEdit.jcs.v1",
    idempotency_key: uuidV7(cryptoImpl),
    frozen_request_body: request,
    frozen_request_digest: frozenRequestDigest,
    frozen_payload_coverage_digest: frozenPayloadCoverageDigest,
    settlement: { kind: "unsettled" },
    frozen_at: new Date().toISOString(),
  };
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "payload_chains", "intents", "submission_groups"], "readwrite",
  );
  const durableGroups = transaction.objectStore("submission_groups");
  const [schema, partition, durableRecords, durablePayloadChain, durableExistingGroups] =
    await Promise.all([
      requestResult(transaction.objectStore("metadata").get("schema")),
      requestResult(transaction.objectStore("partitions")
        .get(workspace.partition.journal_partition_id)),
      requestResult(transaction.objectStore("intents").index("partition")
        .getAll(workspace.partition.journal_partition_id, 2)),
      requestResult(transaction.objectStore("payload_chains").get(record.payload_chain_ref)),
      requestResult(durableGroups.index("partition")
        .getAll(workspace.partition.journal_partition_id, 2)),
    ]);
  if (schema?.version !== DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || JSON.stringify(durableRecords) !== JSON.stringify(records)
    || JSON.stringify(durablePayloadChain) !== JSON.stringify(payloadChain)
    || durableExistingGroups.length !== 0) {
    transaction.abort();
    throw new Error("Local Edit Journal changed before submission freeze");
  }
  durableGroups.add(group);
  await transactionResult(transaction);
  return group;
}

export async function submitOnePendingAuthorEdit({
  workspace, baseUrl, fetchImpl = globalThis.fetch, cryptoImpl = globalThis.crypto,
}) {
  const group = await freezeOneIntentSubmission(workspace, cryptoImpl);
  const challenge = await createProjectCommandChallenge({
    baseUrl,
    projectId: workspace.partition.project_scope.project_id,
    request: {
      method: group.method,
      route_template: group.route_template,
      command_schema: group.command_schema,
      canonical_command_digest: group.frozen_request_digest,
      idempotency_key: group.idempotency_key,
    },
    fetchImpl,
  });
  const response = await applyAuthorEdit({
    baseUrl,
    projectId: workspace.partition.project_scope.project_id,
    request: group.frozen_request_body,
    idempotencyKey: group.idempotency_key,
    antiForgery: challenge.nonce,
    fetchImpl,
  });
  const pending = await rebuildPendingProjection(workspace);
  const effect = response.effect;
  const receipt = response.receipt;
  if (response.schema_id !== "storyos.command.apply-author-edit.response.v1"
    || response.correlation_id !== group.frozen_request_body.correlation_id
    || !UUID.test(response.command_id ?? "")
    || !UUID.test(response.author_command_admission_id ?? "")
    || JSON.stringify(response.project_scope) !== JSON.stringify(group.project_scope)
    || response.completed_intent_record_id !== group.ordered_coverage[0].intent_record_ref
    || response.local_intent_sequence !== String(group.covered_sequence_range.first)
    || !UUID.test(receipt?.receipt_id ?? "")
    || JSON.stringify(receipt?.project_scope) !== JSON.stringify(group.project_scope)
    || receipt?.command_kind !== "applyAuthorEdit"
    || JSON.stringify(receipt?.command_digest) !== JSON.stringify(group.frozen_request_digest)
    || receipt?.idempotency_key !== group.idempotency_key
    || receipt?.producer_cause !== "author_command_admission"
    || receipt?.author_command_admission_id !== response.author_command_admission_id
    || JSON.stringify(receipt?.expected_heads)
      !== JSON.stringify([group.frozen_request_body.expected_authoritative_revision_id])
    || JSON.stringify(receipt?.prior_heads)
      !== JSON.stringify([group.frozen_request_body.expected_authoritative_revision_id])
    || receipt?.result !== "authoritative_applied"
    || effect?.kind !== "authoritative_applied"
    || effect.authoritative_revision?.body !== pending.body
    || JSON.stringify(receipt.resulting_heads)
      !== JSON.stringify([effect.authoritative_revision?.revision_id])
    || JSON.stringify(receipt.authoritative_revision_ids)
      !== JSON.stringify([effect.authoritative_revision?.revision_id])
    || JSON.stringify(receipt.proposal_revision_ids) !== JSON.stringify([])
    || JSON.stringify(receipt.authoritative_commit_ids)
      !== JSON.stringify([effect.authoritative_commit_id])
    || receipt.author_action_sequence !== effect.author_action_sequence
    || JSON.stringify(receipt.draft_artifact_refs) !== JSON.stringify([])
    || JSON.stringify(receipt.artifact_lifecycle_event_refs) !== JSON.stringify([])
    || JSON.stringify(receipt.condition_refs) !== JSON.stringify([])
    || typeof receipt.created_at !== "string"
    || Number.isNaN(Date.parse(receipt.created_at))
    || !UUID.test(effect.authoritative_revision?.revision_id ?? "")
    || !UUID.test(effect.authoritative_commit_id ?? "")
    || !positiveU64(effect.author_action_sequence)
    || !positiveU64(effect.project_activity_position)) {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  const canonical = await getChapter({
    baseUrl,
    projectId: workspace.partition.project_scope.project_id,
    chapterId: group.frozen_request_body.chapter_id,
    fetchImpl,
  });
  if (canonical.schema_id !== "storyos.query.chapter.response.v1"
    || !UUID.test(canonical.correlation_id ?? "")
    || JSON.stringify(canonical.project_scope) !== JSON.stringify(group.project_scope)
    || !boundedU64(canonical.project_activity_position)
    || BigInt(canonical.project_activity_position) < BigInt(effect.project_activity_position)
    || canonical.chapter?.chapter_id !== group.frozen_request_body.chapter_id
    || canonical.chapter?.current_revision?.revision_id
      !== effect.authoritative_revision.revision_id
    || canonical.chapter?.current_revision?.body !== effect.authoritative_revision.body) {
    throw new Error("Authoritative Chapter did not converge");
  }
  const transaction = workspace.database.transaction("submission_groups", "readwrite");
  const groups = transaction.objectStore("submission_groups");
  const durable = await requestResult(groups.get(group.journal_submission_group_id));
  if (JSON.stringify(durable) !== JSON.stringify(group)) {
    transaction.abort();
    throw new Error("Journal Submission Group changed after challenge issuance");
  }
  durable.settlement = {
    kind: "receipt_settled",
    command_id: response.command_id,
    author_command_admission_id: response.author_command_admission_id,
    receipt,
    authoritative_revision: effect.authoritative_revision,
    authoritative_commit_id: effect.authoritative_commit_id,
    author_action_sequence: effect.author_action_sequence,
    project_activity_position: effect.project_activity_position,
  };
  groups.put(durable);
  await transactionResult(transaction);
  return rebuildPendingProjection(workspace);
}

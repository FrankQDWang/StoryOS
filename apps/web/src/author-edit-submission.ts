import {
  applyAuthorEdit,
  createProjectCommandChallenge,
  digestApplyAuthorEdit,
  getEditorSession,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  ApplyAuthorEditResponse,
  DigestValue,
  EditorWriterProjection,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  commitOutcomeQueryWithGroup,
  commitStrongerGroup,
  markOutcomeQueryUnresolved,
  reconcileLostAcknowledgement,
} from "./author-edit-outcome-reconciliation.ts";
import type { DurableOutcomeQuery }
  from "./author-edit-outcome-reconciliation.ts";
import type {
  EditorWorkspace,
  InputOrigin,
  JournalCoverage,
  JournalIntentRecord,
  JournalSubmissionGroup,
  PendingEditProjection,
} from "./editor-types.ts";
import {
  AUTHOR_EDIT_BATCH_POLICY_REVISION,
  AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES,
  AUTHOR_EDIT_MAX_UNITS,
  AUTHOR_EDIT_MAX_WIRE_BODY_BYTES,
  JOURNAL_DATABASE_VERSION,
  mayAppendJournalSubmissionRecord,
  rebuildPendingProjection,
  readJournalSnapshot,
  validateJournalSnapshot,
} from "./local-edit-journal.ts";
import {
  commitProtectedTransportSend,
  readAvailableCommandProof,
} from "./protected-transport-capsule.ts";

const U64 = /^(?:0|[1-9][0-9]{0,19})$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const boundedU64 = (value: unknown): value is string => typeof value === "string" && U64.test(value)
  && BigInt(value) <= 18446744073709551615n;
const positiveU64 = (value: unknown): value is string => boundedU64(value) && BigInt(value) > 0n;

async function canonicalBodyDigest(body: string, cryptoImpl: Crypto): Promise<string> {
  const digest = new Uint8Array(await cryptoImpl.subtle.digest(
    "SHA-256", new TextEncoder().encode(body),
  ));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function writerProjectionAllowsLateDurableSettlement(
  canonicalWriter: EditorWriterProjection,
  sessionWriter: EditorWriterProjection,
  partitionGeneration: string,
): boolean {
  if (JSON.stringify(canonicalWriter) === JSON.stringify(sessionWriter)) return true;
  // A fenced tab may observe generation N+1 while its journal partition stays
  // bound to generation N. Do not treat that writer delta as Snapshot mismatch.
  return sessionWriter?.kind === "current_writer"
    && canonicalWriter?.kind === "read_only"
    && canonicalWriter.reason === "superseded_by_takeover"
    && !("writer_generation" in canonicalWriter)
    && positiveU64(canonicalWriter.observed_writer_generation)
    && sessionWriter.writer_generation === partitionGeneration
    && BigInt(canonicalWriter.observed_writer_generation) > BigInt(sessionWriter.writer_generation);
}

const requestResult = (request: IDBRequest): Promise<unknown> => new Promise((resolve, reject) => {
  request.onsuccess = () => resolve(request.result);
  request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
});
const transactionResult = (transaction: IDBTransaction): Promise<void> => new Promise(
  (resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () => reject(
      transaction.error ?? new Error("IndexedDB transaction aborted"),
    );
    transaction.onerror = () => reject(
      transaction.error ?? new Error("IndexedDB transaction failed"),
    );
  },
);

function uuidV7(cryptoImpl: Crypto = globalThis.crypto, now = Date.now()): string {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

async function digestSubmissionCoverage(
  coverage: {
    ordered_coverage: JournalCoverage[];
    covered_sequence_range: { first: number; last: number };
  },
  cryptoImpl: Crypto,
): Promise<DigestValue> {
  const bytes = new TextEncoder().encode(JSON.stringify(coverage));
  const digest = new Uint8Array(await cryptoImpl.subtle.digest("SHA-256", bytes));
  return {
    algorithm: "sha256",
    profile: "storyos.local-edit-journal.submission-coverage.sha256.v1",
    value_hex_lowercase: [...digest]
      .map((byte) => byte.toString(16).padStart(2, "0")).join(""),
  };
}

const HARD_FLUSH_ORIGINS = new Set<InputOrigin>([
  "composition_confirmation", "paste", "cut",
]);

async function validateFrozenGroup(
  group: JournalSubmissionGroup,
  workspace: EditorWorkspace,
  records: JournalIntentRecord[],
  cryptoImpl: Crypto,
): Promise<void> {
  const request = group.frozen_request_body;
  const firstRecord = records[0]!;
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
    || group.batch_policy_revision !== AUTHOR_EDIT_BATCH_POLICY_REVISION
    || group.action_class !== "direct_editor_action"
    || group.api_major !== 1
    || group.method !== "POST"
    || group.route_template !== "/api/v1/projects/{project_id}/manuscript/author-edits"
    || group.command_schema !== "storyos.command.apply-author-edit.request.v1"
    || group.command_kind !== "applyAuthorEdit"
    || group.digest_profile !== "storyos.command.applyAuthorEdit.jcs.v1"
    || !UUID.test(group.idempotency_key ?? "")
    || group.settlement?.kind !== "unsettled"
    || records.length === 0
    || records.length > AUTHOR_EDIT_MAX_UNITS
    || group.ordered_coverage?.length !== records.length
    || group.covered_sequence_range?.first !== firstRecord.local_intent_sequence
    || group.covered_sequence_range?.last !== records.at(-1)!.local_intent_sequence
    || request?.command_schema !== group.command_schema
    || request?.editor_session_id !== workspace.partition.editor_session_id
    || request?.writer_generation !== workspace.partition.writer_generation
    || request?.chapter_id !== firstRecord.chapter_object_id
    || request?.expected_authoritative_revision_id !== firstRecord.expected_authoritative_heads[0]
    || JSON.stringify(request?.expected_proposal_head_revision_ids)
      !== JSON.stringify(firstRecord.expected_proposal_heads)
    || JSON.stringify(request?.target_refs) !== JSON.stringify(firstRecord.target_refs)
    || request?.observed_ownership_partition !== firstRecord.observed_ownership_partition
    || request?.undo_group_id !== firstRecord.undo_group_binding.undo_group_id
    || request?.completed_intent_record_id !== firstRecord.completed_intent_record_id
    || request?.local_intent_sequence !== String(firstRecord.local_intent_sequence)
    || JSON.stringify(request?.author_edit_units)
      !== JSON.stringify(records.map((record) => record.author_edit_unit))
    || records.reduce((count, record) =>
      count + record.author_edit_unit!.normalized_primitives.length, 0)
      > AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES
    || new TextEncoder().encode(JSON.stringify(request)).byteLength
      > AUTHOR_EDIT_MAX_WIRE_BODY_BYTES
    || JSON.stringify(group.frozen_request_digest) !== JSON.stringify(requestDigest)
    || JSON.stringify(group.frozen_payload_coverage_digest) !== JSON.stringify(coverageDigest)) {
    throw new Error("Journal Submission Group is corrupt");
  }
  for (const [index, record] of records.entries()) {
    const coverage = group.ordered_coverage[index];
    if (record.local_intent_sequence !== firstRecord.local_intent_sequence + index
      || coverage!.local_intent_sequence !== record.local_intent_sequence
      || coverage!.intent_record_ref !== record.completed_intent_record_id
      || JSON.stringify(coverage!.payload_digest) !== JSON.stringify(record.payload_digest)
      || (index > 0 && !mayAppendJournalSubmissionRecord(
        firstRecord, records[index - 1]!, record,
      ))) {
      throw new Error("Journal Submission Group is corrupt");
    }
  }
}

export async function freezeOneIntentSubmission(
  workspace: EditorWorkspace,
  cryptoImpl: Crypto = globalThis.crypto,
): Promise<JournalSubmissionGroup> {
  if (workspace.partition.disposition !== "current_writer_open") {
    throw new Error("Editor Session is read only");
  }
  const snapshot = await validateJournalSnapshot(workspace, await readJournalSnapshot(workspace));
  const existing = snapshot.groups.find((group) => group.settlement?.kind === "unsettled");
  if (existing) {
    if (existing.reconciliation?.kind === "outcome_query_unresolved") return existing;
    const existingRecords = existing.ordered_coverage.map((coverage) =>
      snapshot.records.find((record) =>
        record.local_intent_sequence === coverage.local_intent_sequence));
    await validateFrozenGroup(
      existing, workspace, existingRecords as JournalIntentRecord[], cryptoImpl,
    );
    return existing;
  }
  const coveredSequences = new Set(snapshot.groups.flatMap((group) =>
    group.ordered_coverage.map((coverage) => coverage.local_intent_sequence)));
  const firstUncovered = snapshot.records.find((record) =>
    !coveredSequences.has(record.local_intent_sequence));
  if (firstUncovered
    && firstUncovered.base_snapshot_id !== workspace.session.base_snapshot.snapshot_id) {
    throw new Error("Local Edit Journal is corrupt");
  }
  const candidates = snapshot.records.filter((record) =>
    !coveredSequences.has(record.local_intent_sequence)
    && record.base_snapshot_id === workspace.session.base_snapshot.snapshot_id);
  if (candidates.length === 0) {
    throw new Error("One pending Author Edit is required");
  }
  const firstCandidate = candidates[0]!;
  if (firstCandidate.local_intent_sequence !== firstUncovered?.local_intent_sequence) {
    throw new Error("Local Edit Journal is corrupt");
  }
  const records: JournalIntentRecord[] = [firstCandidate];
  if (!HARD_FLUSH_ORIGINS.has(firstCandidate.input_origin)) {
    for (const candidate of candidates.slice(1)) {
      if (records.length === AUTHOR_EDIT_MAX_UNITS
        || !mayAppendJournalSubmissionRecord(records[0]!, records.at(-1)!, candidate)) break;
      records.push(candidate);
    }
  }
  const firstRecord = records[0]!;
  const request: ApplyAuthorEditRequest = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    client_contract_revision: workspace.partition.client_contract_revision,
    security_policy_revision: workspace.partition.security_policy_revision,
    correlation_id: uuidV7(cryptoImpl),
    editor_session_id: workspace.partition.editor_session_id,
    writer_generation: workspace.partition.writer_generation,
    chapter_id: firstRecord.chapter_object_id,
    expected_authoritative_revision_id: firstRecord.expected_authoritative_heads[0]!,
    expected_proposal_head_revision_ids: firstRecord.expected_proposal_heads,
    target_refs: firstRecord.target_refs,
    observed_ownership_partition: firstRecord.observed_ownership_partition,
    editor_contract_revision: firstRecord.editor_contract_revision,
    undo_group_id: firstRecord.undo_group_binding.undo_group_id,
    completed_intent_record_id: firstRecord.completed_intent_record_id,
    local_intent_sequence: String(firstRecord.local_intent_sequence),
    author_edit_units: records.map((record) => record.author_edit_unit!),
  };
  const orderedCoverage: JournalCoverage[] = records.map((record) => ({
    local_intent_sequence: record.local_intent_sequence,
    intent_record_ref: record.completed_intent_record_id,
    payload_digest: record.payload_digest,
  }));
  const coveredSequenceRange = {
    first: firstRecord.local_intent_sequence, last: records.at(-1)!.local_intent_sequence,
  };
  const [frozenRequestDigest, frozenPayloadCoverageDigest] = await Promise.all([
    digestApplyAuthorEdit(request, cryptoImpl),
    digestSubmissionCoverage({ ordered_coverage: orderedCoverage,
      covered_sequence_range: coveredSequenceRange }, cryptoImpl),
  ]);
  const group: JournalSubmissionGroup = {
    journal_submission_group_id: uuidV7(cryptoImpl),
    journal_partition_id: workspace.partition.journal_partition_id,
    project_scope: workspace.partition.project_scope,
    editor_session_id: workspace.partition.editor_session_id,
    writer_generation: workspace.partition.writer_generation,
    batch_policy_revision: AUTHOR_EDIT_BATCH_POLICY_REVISION,
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
  await validateFrozenGroup(group, workspace, records, cryptoImpl);
  const transaction = workspace.database.transaction(
    ["metadata", "partitions", "payload_chains", "intents", "submission_groups"], "readwrite",
  );
  const durableGroups = transaction.objectStore("submission_groups");
  const [schema, partition, durableRecords, durablePayloadChains, durableExistingGroups] =
    await Promise.all([
      requestResult(transaction.objectStore("metadata").get("schema")),
      requestResult(transaction.objectStore("partitions")
        .get(workspace.partition.journal_partition_id)),
      requestResult(transaction.objectStore("intents").index("partition")
        .getAll(workspace.partition.journal_partition_id, 2401)),
      requestResult(transaction.objectStore("payload_chains").index("partition")
        .getAll(workspace.partition.journal_partition_id, 2401)),
      requestResult(durableGroups.index("partition")
        .getAll(workspace.partition.journal_partition_id, 2401)),
    ]);
  if ((schema as { version?: unknown } | undefined)?.version !== JOURNAL_DATABASE_VERSION
    || JSON.stringify(partition) !== JSON.stringify(workspace.partition)
    || JSON.stringify(durableRecords) !== JSON.stringify(snapshot.records)
    || JSON.stringify(durablePayloadChains) !== JSON.stringify(snapshot.payloadChains)
    || JSON.stringify(durableExistingGroups) !== JSON.stringify(snapshot.groups)) {
    transaction.abort();
    throw new Error("Local Edit Journal changed before submission freeze");
  }
  durableGroups.add(group);
  await transactionResult(transaction);
  return group;
}

async function settleAuthorEditResponse({
  workspace, group, response, baseUrl, fetchImpl, cryptoImpl, durableQuery,
}: {
  workspace: EditorWorkspace;
  group: JournalSubmissionGroup;
  response: unknown;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  durableQuery?: DurableOutcomeQuery;
}): Promise<void> {
  const pending = await rebuildPendingProjection(workspace);
  const settledResponse = response as ApplyAuthorEditResponse;
  const effect = settledResponse.effect;
  const receipt = settledResponse.receipt;
  if (settledResponse.schema_id !== "storyos.command.apply-author-edit.response.v2"
    || settledResponse.correlation_id !== group.frozen_request_body.correlation_id
    || !UUID.test(settledResponse.command_id ?? "")
    || !UUID.test(settledResponse.author_command_admission_id ?? "")
    || JSON.stringify(settledResponse.project_scope) !== JSON.stringify(group.project_scope)
    || settledResponse.completed_intent_record_id !== group.ordered_coverage[0]!.intent_record_ref
    || settledResponse.local_intent_sequence !== String(group.covered_sequence_range.first)
    || !UUID.test(receipt?.receipt_id ?? "")
    || JSON.stringify(receipt?.project_scope) !== JSON.stringify(group.project_scope)
    || receipt?.command_kind !== "applyAuthorEdit"
    || JSON.stringify(receipt?.command_digest) !== JSON.stringify(group.frozen_request_digest)
    || receipt?.idempotency_key !== group.idempotency_key
    || receipt?.producer_cause !== "author_command_admission"
    || receipt?.author_command_admission_id !== settledResponse.author_command_admission_id
    || JSON.stringify(receipt?.expected_heads)
      !== JSON.stringify([group.frozen_request_body.expected_authoritative_revision_id])
    || JSON.stringify(receipt.proposal_revision_ids) !== JSON.stringify([])
    || JSON.stringify(receipt.draft_artifact_refs) !== JSON.stringify([])
    || JSON.stringify(receipt.artifact_lifecycle_event_refs) !== JSON.stringify([])
    || JSON.stringify(receipt.condition_refs) !== JSON.stringify([])
    || typeof receipt.created_at !== "string"
    || Number.isNaN(Date.parse(receipt.created_at))) {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  if (effect?.kind !== "authoritative_applied") {
    const currentHead = effect?.kind === "conflicted"
      ? effect.current_authoritative_revision_id
      : group.frozen_request_body.expected_authoritative_revision_id;
    const validEffect = effect?.kind === "no_effect"
      ? effect.reason === "content_unchanged" && receipt.result === "no_effect"
      : effect?.kind === "conflicted"
        ? ["stale_authoritative_head", "proposal_head_present", "ownership_changed"]
          .includes(effect.reason)
          && UUID.test(effect.current_authoritative_revision_id ?? "")
          && receipt.result === "conflicted"
        : effect?.kind === "refused"
          && ["unsupported_intent_shape", "invalid_selection", "target_mismatch"]
            .includes(effect.reason)
          && receipt.result === "refused";
    if (!validEffect
      || JSON.stringify(receipt.prior_heads) !== JSON.stringify([currentHead])
      || JSON.stringify(receipt.resulting_heads) !== JSON.stringify([currentHead])
      || JSON.stringify(receipt.authoritative_revision_ids) !== JSON.stringify([])
      || JSON.stringify(receipt.authoritative_commit_ids) !== JSON.stringify([])
      || receipt.author_action_sequence !== null) {
      throw new Error("Author Edit acknowledgement does not converge");
    }
    const rest = { ...group };
    delete rest.reconciliation;
    const next: JournalSubmissionGroup = {
      ...rest,
      settlement: {
        kind: "zero_authority_receipt_settled",
        command_id: settledResponse.command_id,
        author_command_admission_id: settledResponse.author_command_admission_id,
        receipt,
        effect,
      },
    };
    if (durableQuery) await commitOutcomeQueryWithGroup(workspace, { ...durableQuery, next });
    else await commitStrongerGroup(workspace, next);
    return;
  }
  if (receipt.result !== "authoritative_applied"
    || effect.authoritative_revision?.body !== pending.body
    || JSON.stringify(receipt.prior_heads)
      !== JSON.stringify([group.frozen_request_body.expected_authoritative_revision_id])
    || JSON.stringify(receipt.resulting_heads)
      !== JSON.stringify([effect.authoritative_revision?.revision_id])
    || JSON.stringify(receipt.authoritative_revision_ids)
      !== JSON.stringify([effect.authoritative_revision?.revision_id])
    || JSON.stringify(receipt.authoritative_commit_ids)
      !== JSON.stringify([effect.authoritative_commit_id])
    || receipt.author_action_sequence !== effect.author_action_sequence
    || !UUID.test(effect.authoritative_revision?.revision_id ?? "")
    || !UUID.test(effect.authoritative_commit_id ?? "")
    || !positiveU64(effect.author_action_sequence)
    || !positiveU64(effect.project_activity_position)) {
    throw new Error("Author Edit acknowledgement does not converge");
  }
  const canonical = await getEditorSession({
    baseUrl,
    projectId: workspace.partition.project_scope.project_id,
    editorSessionId: workspace.partition.editor_session_id,
    fetchImpl,
  });
  const freshBase = canonical.base_snapshot;
  if (canonical.schema_id !== "storyos.query.editor-session.response.v1"
    || !UUID.test(canonical.correlation_id ?? "")
    || JSON.stringify(canonical.project_scope) !== JSON.stringify(group.project_scope)
    || JSON.stringify(canonical.editor_session) !== JSON.stringify(workspace.session.editor_session)
    || !writerProjectionAllowsLateDurableSettlement(
      canonical.writer, workspace.session.writer, workspace.partition.writer_generation,
    )
    || !UUID.test(freshBase?.snapshot_id ?? "")
    || freshBase.snapshot_id === workspace.session.base_snapshot.snapshot_id
    || freshBase.chapter_id !== group.frozen_request_body.chapter_id
    || freshBase.project_activity_position !== effect.project_activity_position
    || freshBase.authoritative_head_revision_id !== effect.authoritative_revision.revision_id
    || JSON.stringify(freshBase.proposal_head_revision_ids)
      !== JSON.stringify(group.frozen_request_body.expected_proposal_head_revision_ids)
    || JSON.stringify(freshBase.target_refs)
      !== JSON.stringify(group.frozen_request_body.target_refs)
    || freshBase.observed_ownership_partition
      !== group.frozen_request_body.observed_ownership_partition
    || JSON.stringify(freshBase.materialized_revision)
      !== JSON.stringify(effect.authoritative_revision)
    || freshBase.materialized_payload_digest?.algorithm !== "sha256"
    || freshBase.materialized_payload_digest?.profile
      !== "storyos.canonical-payload.sha256.v1"
    || freshBase.materialized_payload_digest?.value_hex_lowercase
      !== await canonicalBodyDigest(effect.authoritative_revision.body, cryptoImpl)
    || typeof freshBase.created_at !== "string"
    || Number.isNaN(Date.parse(freshBase.created_at))) {
    throw new Error("Editor Base Snapshot did not converge");
  }
  const previousPartition = workspace.partition;
  if (writerProjectionAllowsLateDurableSettlement(
    canonical.writer, workspace.session.writer, workspace.partition.writer_generation,
  ) && canonical.writer?.kind === "read_only") {
    workspace.partition = { ...previousPartition, disposition: "read_only_observer" };
  }
  const rest = { ...group };
  delete rest.reconciliation;
  const next: JournalSubmissionGroup = {
    ...rest,
    settlement: {
      kind: "applied_receipt_settled",
      command_id: settledResponse.command_id,
      author_command_admission_id: settledResponse.author_command_admission_id,
      receipt,
      authoritative_revision: effect.authoritative_revision,
      authoritative_commit_id: effect.authoritative_commit_id,
      author_action_sequence: effect.author_action_sequence,
      project_activity_position: effect.project_activity_position,
      installed_base_snapshot: freshBase,
    },
  };
  try {
    if (durableQuery) {
      await commitOutcomeQueryWithGroup(workspace, { ...durableQuery, next, activeBase: freshBase });
    } else {
      await commitStrongerGroup(workspace, next, freshBase);
    }
  } catch (error) {
    workspace.partition = previousPartition;
    throw error;
  }
  workspace.session = canonical;
}

export async function submitOnePendingAuthorEdit({
  workspace, baseUrl, fetchImpl = globalThis.fetch, cryptoImpl = globalThis.crypto,
}: {
  workspace: EditorWorkspace;
  baseUrl: string;
  fetchImpl?: typeof fetch;
  cryptoImpl?: Crypto;
}): Promise<PendingEditProjection> {
  const run = async (): Promise<PendingEditProjection> => {
    const group = await freezeOneIntentSubmission(workspace, cryptoImpl);
    const alreadyUnresolved = group.reconciliation?.kind === "outcome_query_unresolved";
    const proof = alreadyUnresolved ? null : await readAvailableCommandProof(workspace, group);
    if (alreadyUnresolved || proof) {
      const current = alreadyUnresolved
        ? group
        : await markOutcomeQueryUnresolved(workspace, group);
      await reconcileLostAcknowledgement({
        workspace, group: current, baseUrl, fetchImpl, cryptoImpl,
        settleFromCommandResponse: settleAuthorEditResponse,
      });
      return rebuildPendingProjection(workspace);
    }
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
    await commitProtectedTransportSend(workspace, group, challenge);
    let response: unknown;
    try {
      response = await applyAuthorEdit({
        baseUrl,
        projectId: workspace.partition.project_scope.project_id,
        request: group.frozen_request_body,
        idempotencyKey: group.idempotency_key,
        antiForgery: challenge.nonce,
        fetchImpl,
      });
    } catch {
      const unresolved = await markOutcomeQueryUnresolved(workspace, group);
      await reconcileLostAcknowledgement({
        workspace, group: unresolved, baseUrl, fetchImpl, cryptoImpl,
        settleFromCommandResponse: settleAuthorEditResponse,
      });
      return rebuildPendingProjection(workspace);
    }
    await settleAuthorEditResponse({
      workspace, group, response, baseUrl, fetchImpl, cryptoImpl,
    });
    return rebuildPendingProjection(workspace);
  };
  const queued = (workspace.submitQueue ?? Promise.resolve()).then(run, run);
  workspace.submitQueue = queued.catch(() => {});
  return queued;
}

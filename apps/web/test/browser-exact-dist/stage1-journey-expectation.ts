export const USER_A = "018f0000-0000-7001-8000-000000000001";
export const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
export const CHAPTER = "018f0000-0000-7001-8000-000000000003";
export const INITIAL_REVISION = "018f0000-0000-7001-8000-000000000005";
export const OPEN_BODY = "Authoritative A";
export const AFTER_TYPE = "Authoritative A Hello";
export const AFTER_IME = "Authoritative A Hello中文";
export const AFTER_PASTE = "Authoritative A Hello中文 EN";
export const AFTER_UNSETTLED = "Authoritative A Hello中文 EN!";

export type JourneyRow = Readonly<Record<string, unknown>>;

export interface JourneyJournal {
  readonly fences: JourneyRow[];
  readonly groups: JourneyRow[];
  readonly intents: JourneyRow[];
}

export interface ObservedStage1Journey {
  readonly afterIme: unknown;
  readonly afterImeInput: unknown;
  readonly afterPasteInput: unknown;
  readonly afterType: unknown;
  readonly authority: {
    readonly activities: JourneyRow[];
    readonly effects: JourneyRow[];
    readonly manuscript: unknown;
    readonly receipts: JourneyRow[];
  };
  readonly id: "S1-JRN-001";
  readonly input: unknown;
  readonly interrupt: { readonly journal: JourneyJournal; readonly pending: unknown };
  readonly open: unknown;
  readonly recover: {
    readonly journal: JourneyJournal;
    readonly pending: unknown;
    readonly saved: unknown;
  };
  readonly settle: { readonly journal: JourneyJournal; readonly pending: unknown };
}

const SCOPE = { owner_user_id: USER_A, project_id: PROJECT_A };
const TARGET = [`manuscript:${CHAPTER}`];
const CLIENT_CONTRACT = "storyos.web-client.release-1.v3";
const SECURITY_POLICY = "storyos.web-security-policy.release-1.v1";
const EDITOR_CONTRACT = "storyos.editor-contract.release-1.v2";
const BATCH_POLICY = "storyos.author-edit-batch.release-1.preview.v1";
const LIMIT_PROFILE = "storyos.foundation.absolute.v1";
const JOURNAL_PAYLOAD = "storyos.local-edit-journal.payload.sha256.v1";
const JOURNAL_COVERAGE = "storyos.local-edit-journal.submission-coverage.sha256.v1";
const CANONICAL_PAYLOAD = "storyos.canonical-payload.sha256.v1";
const COMMAND_DIGEST = "storyos.command.applyAuthorEdit.jcs.v1";
const COMMAND_SCHEMA = "storyos.command.apply-author-edit.request.v1";
const PARTITION = [
  USER_A,
  PROJECT_A,
  "editor-session",
  "1",
  "client-session",
  "1",
  CLIENT_CONTRACT,
  SECURITY_POLICY,
  LIMIT_PROFILE,
].join(":");
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u;
const BODIES = [OPEN_BODY, AFTER_TYPE, AFTER_IME, AFTER_PASTE, AFTER_UNSETTLED] as const;
const TEXTS = [" Hello", "中文", " EN", "!"] as const;
const ORIGINS = ["typing", "typing", "paste", "typing"] as const;
const UNIT_DIGESTS = [
  "8ffb38577db2c7858ade72684cf5100a5cfe9cd305b50e7c94a1fb0167beebbe",
  "b8dde1c51a0b6118f0cf143a6dabacdf81f65a6932f9f2c3d8a1038381a62a8b",
  "ad2b8fdada8755d7381925fc994d4c9b1be0193afcd88bd68eb05647d8b12b2d",
  "6816d9d0141a07449962a67a93603332b26ce69ed5a1f1a6ce9d2cbe3b676b7e",
] as const;
const BODY_DIGESTS = [
  "87857d5ef182d0ec64f1674d9eb1d0b15ce31ede622af3527ba72a8405417109",
  "448ccebb2dc2b34125b421c342932acd70b29eeae73dd04a6895f0ad38feb2a0",
  "3bd56a6c7a9f057bf34938ec43536df1c98c925ddfdd269d8a0c4fa88388fa3c",
  "c7eaeca50cfb69f8d8f29aade21d2b3fe902c1dc4ccce01ec615170f31c87f20",
  "c3d7b3b641831ec4298377f9b541cf79bf4424b26c016acae5a589495a87b815",
] as const;

function requiredAt<Value>(values: readonly Value[], index: number, label: string): Value {
  const value = values[index];
  if (value === undefined) throw new Error(`${label} ${index} is unavailable`);
  return value;
}

function digestValue(profile: string, value: string) {
  return { algorithm: "sha256", profile, value_hex_lowercase: value };
}

function journalDigest(index: number) {
  return digestValue(JOURNAL_PAYLOAD, requiredAt(UNIT_DIGESTS, index, "unit digest"));
}

function bodyDigest(index: number) {
  return digestValue(CANONICAL_PAYLOAD, requiredAt(BODY_DIGESTS, index, "body digest"));
}

function pendingProjection(
  body: string,
  save_state: string,
  unsettled_intent_count: number,
  authoritative_revision_id: string,
) {
  return { body, save_state, unsettled_intent_count, authoritative_revision_id };
}

function priorRevision(index: number): string {
  return index === 0 ? INITIAL_REVISION : `revision-${index}`;
}

function replaceUnit(index: number) {
  const from = requiredAt(BODIES, index, "body").length;
  return {
    normalized_primitives: [{
      kind: "replace_selection",
      from,
      to: from,
      text: requiredAt(TEXTS, index, "edit text"),
    }],
    selection_snapshot: {
      coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      from,
      to: from,
    },
  };
}

function slot(kind: string, sequence: number): string {
  return `${kind}-${sequence}`;
}

function commandDigestValue(sequence: number) {
  return digestValue(COMMAND_DIGEST, slot("digest-command", sequence));
}

function appliedReceipt(index: number) {
  const sequence = index + 1;
  const expected = priorRevision(index);
  const resulting = slot("revision", sequence);
  return {
    receipt_id: slot("receipt", sequence),
    project_scope: SCOPE,
    command_kind: "applyAuthorEdit",
    command_digest: commandDigestValue(sequence),
    idempotency_key: slot("idempotency", sequence),
    producer_cause: "author_command_admission",
    author_command_admission_id: slot("admission", sequence),
    expected_heads: [expected],
    prior_heads: [expected],
    resulting_heads: [resulting],
    authoritative_revision_ids: [resulting],
    proposal_revision_ids: [],
    authoritative_commit_ids: [slot("commit", sequence)],
    author_action_sequence: String(sequence),
    draft_artifact_refs: [],
    artifact_lifecycle_event_refs: [],
    condition_refs: [],
    result: "authoritative_applied",
    created_at: "instant",
  };
}

function appliedEffect(index: number) {
  const sequence = index + 1;
  return {
    kind: "authoritative_applied",
    authoritative_revision: {
      revision_id: slot("revision", sequence),
      body: requiredAt(BODIES, sequence, "body"),
    },
    authoritative_commit_id: slot("commit", sequence),
    author_action_sequence: String(sequence),
    project_activity_position: String(sequence),
  };
}

function appliedActivity(index: number) {
  const sequence = index + 1;
  const payload = {
    chapter_id: CHAPTER,
    authoritative_revision_id: slot("revision", sequence),
    authoritative_commit_id: slot("commit", sequence),
    author_action_sequence: String(sequence),
  };
  return {
    envelope_version: 1,
    activity_profile: "storyos.project-activity.v1",
    event_id: slot("event", sequence),
    event_schema: "storyos.event.authoritative-author-edit-applied.v1",
    event_kind: "authoritative_author_edit_applied",
    project_scope: SCOPE,
    requester_user_id: USER_A,
    actor: { kind: "author", id: USER_A },
    project_sequence: String(sequence),
    stream_sequence: String(sequence),
    agent_run_id: null,
    run_step_id: null,
    run_sequence: null,
    aggregate_ref: { kind: "chapter", id: CHAPTER },
    correlation_id: slot("correlation", sequence),
    causation: { kind: "command", id: slot("command", sequence) },
    command_id: slot("command", sequence),
    receipt_ref: { kind: "domain_receipt", id: slot("receipt", sequence) },
    occurred_at: "instant",
    recorded_at: "instant",
    payload,
    payload_digest: digestValue("storyos.event-payload.jcs.v1", slot("digest-event", sequence)),
    application_wire_record_ref: slot("event", sequence),
    limit_profile_revision: LIMIT_PROFILE,
  };
}

function intentRecord(index: number, retainedUnit = false) {
  const sequence = index + 1;
  const unit = replaceUnit(index);
  const snapshot = slot("snapshot", index);
  const record = {
    completed_intent_record_id: slot("intent", sequence),
    local_intent_sequence: sequence,
    journal_partition_id: PARTITION,
    project_scope: SCOPE,
    editor_session_id: "editor-session",
    writer_generation: "1",
    limit_profile_revision: LIMIT_PROFILE,
    batch_policy_revision: BATCH_POLICY,
    input_origin: requiredAt(ORIGINS, index, "input origin"),
    chapter_object_id: CHAPTER,
    base_snapshot_id: snapshot,
    base_activity_position: String(index),
    target_refs: TARGET,
    expected_authoritative_heads: [priorRevision(index)],
    expected_proposal_heads: [],
    proposal_anchors: [],
    observed_ownership_partition: "authoritative",
    retry_source: { kind: "fresh_editor_intent" },
    editor_contract_revision: EDITOR_CONTRACT,
    undo_group_binding: { kind: "direct_author_input", undo_group_id: slot("undo", sequence) },
    payload_chain_ref: slot("chain", sequence),
    payload_digest: journalDigest(index),
    projection_dependency: { snapshot_id: snapshot, prior_sequence: index },
    created_at: "instant",
  };
  return retainedUnit ? { ...record, author_edit_unit: unit } : record;
}

function installedSnapshot(index: number) {
  const sequence = index + 1;
  const revision = slot("revision", sequence);
  return {
    snapshot_id: slot("snapshot", sequence),
    chapter_id: CHAPTER,
    project_activity_position: String(sequence),
    authoritative_head_revision_id: revision,
    proposal_head_revision_ids: [],
    target_refs: TARGET,
    observed_ownership_partition: "authoritative",
    materialized_revision: {
      revision_id: revision,
      body: requiredAt(BODIES, sequence, "body"),
    },
    materialized_payload_digest: bodyDigest(sequence),
    created_at: "instant",
  };
}

function collectedGroup(index: number) {
  const sequence = index + 1;
  const orderedCoverage = [{
    local_intent_sequence: sequence,
    intent_record_ref: slot("intent", sequence),
    payload_digest: journalDigest(index),
  }];
  return {
    journal_submission_group_id: slot("group", sequence),
    journal_partition_id: PARTITION,
    project_scope: SCOPE,
    editor_session_id: "editor-session",
    writer_generation: "1",
    batch_policy_revision: BATCH_POLICY,
    ordered_coverage: orderedCoverage,
    covered_sequence_range: { first: sequence, last: sequence },
    action_class: "direct_editor_action",
    api_major: 1,
    method: "POST",
    route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
    command_schema: COMMAND_SCHEMA,
    command_kind: "applyAuthorEdit",
    digest_profile: COMMAND_DIGEST,
    idempotency_key: slot("idempotency", sequence),
    frozen_request_body: {
      command_schema: COMMAND_SCHEMA,
      client_contract_revision: CLIENT_CONTRACT,
      security_policy_revision: SECURITY_POLICY,
      correlation_id: slot("correlation", sequence),
      editor_session_id: "editor-session",
      writer_generation: "1",
      chapter_id: CHAPTER,
      expected_authoritative_revision_id: priorRevision(index),
      expected_proposal_head_revision_ids: [],
      target_refs: TARGET,
      observed_ownership_partition: "authoritative",
      editor_contract_revision: EDITOR_CONTRACT,
      undo_group_id: slot("undo", sequence),
      completed_intent_record_id: slot("intent", sequence),
      local_intent_sequence: String(sequence),
      author_edit_units: [],
    },
    frozen_request_digest: commandDigestValue(sequence),
    frozen_payload_coverage_digest: digestValue(
      JOURNAL_COVERAGE,
      slot("digest-coverage", sequence),
    ),
    settlement: {
      kind: "applied_receipt_settled",
      command_id: slot("command", sequence),
      author_command_admission_id: slot("admission", sequence),
      receipt: appliedReceipt(index),
      authoritative_revision: {
        revision_id: slot("revision", sequence),
        body: requiredAt(BODIES, sequence, "body"),
      },
      authoritative_commit_id: slot("commit", sequence),
      author_action_sequence: String(sequence),
      project_activity_position: String(sequence),
      installed_base_snapshot: installedSnapshot(index),
    },
    frozen_at: "instant",
    payload_collection: { kind: "collected", collection_fence_id: slot("fence", sequence) },
  };
}

function collectedFence(index: number) {
  const sequence = index + 1;
  return {
    collection_fence_id: slot("fence", sequence),
    journal_partition_id: PARTITION,
    project_scope: SCOPE,
    writer_generation: "1",
    partition_disposition: "current_writer_open",
    collected_groups: [{
      journal_submission_group_id: slot("group", sequence),
      covered_sequence_range: { first: sequence, last: sequence },
      payload_digests: [journalDigest(index)],
      settlement_kind: "applied_receipt_settled",
      command_id: slot("command", sequence),
      author_command_admission_id: slot("admission", sequence),
      receipt_id: slot("receipt", sequence),
      project_activity_position: String(sequence),
    }],
    successor: {
      kind: "authoritative_revision",
      snapshot_id: slot("snapshot", sequence),
      revision_id: slot("revision", sequence),
      materialized_payload_digest: bodyDigest(sequence),
    },
    collected_intent_sequences: [sequence],
    collected_payload_chain_ids: [slot("chain", sequence)],
    reason: "applied_receipt_converged_with_durable_successor",
  };
}

function journalSnapshot(count: number, extraIntent = false) {
  return {
    groups: Array.from({ length: count }, (_value, index) => collectedGroup(index)),
    intents: [
      ...Array.from({ length: count }, (_value, index) => intentRecord(index)),
      ...(extraIntent ? [intentRecord(count, true)] : []),
    ],
    fences: Array.from({ length: count }, (_value, index) => collectedFence(index)),
  };
}

function isRow(value: unknown): value is JourneyRow {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function row(value: unknown, label: string): JourneyRow {
  if (!isRow(value)) throw new Error(`${label} is not an object`);
  return value;
}

function valueAt(value: unknown, key: string): unknown {
  return Reflect.get(row(value, key), key);
}

function stringAt(value: unknown, key: string): string {
  const result = valueAt(value, key);
  if (typeof result !== "string") throw new Error(`${key} is not a string`);
  return result;
}

function valuesAt(value: unknown, key: string): unknown[] {
  const result = valueAt(value, key);
  if (!Array.isArray(result)) throw new Error(`${key} is not an array`);
  return result;
}

function firstRow(values: readonly JourneyRow[], label: string): JourneyRow {
  const value = values[0];
  if (value === undefined) throw new Error(`${label} is empty`);
  return value;
}

function bind(map: Map<string, string>, from: unknown, to: string): void {
  if (typeof from === "string" && from.length > 0 && !map.has(from)) map.set(from, to);
}

function rewrite(
  value: unknown,
  ids: ReadonlyMap<string, string>,
  digests: ReadonlyMap<string, string>,
): unknown {
  if (Array.isArray(value)) return value.map((item) => rewrite(item, ids, digests));
  if (isRow(value)) {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [
      key,
      rewrite(item, ids, digests),
    ]));
  }
  if (typeof value !== "string") return value;
  if (INSTANT.test(value)) return "instant";
  let next = value;
  for (const [from, to] of ids) {
    if (from !== to) next = next.split(from).join(to);
  }
  for (const [from, to] of digests) next = next.split(from).join(to);
  return next;
}

export function normalizeStage1Journey(observed: ObservedStage1Journey): unknown {
  const ids = new Map<string, string>();
  const digests = new Map<string, string>();
  observed.authority.receipts.forEach((receipt, index) => {
    const sequence = index + 1;
    bind(ids, valueAt(receipt, "receipt_id"), slot("receipt", sequence));
    bind(ids, valueAt(receipt, "author_command_admission_id"), slot("admission", sequence));
    bind(ids, valueAt(receipt, "idempotency_key"), slot("idempotency", sequence));
    bind(ids, valuesAt(receipt, "authoritative_revision_ids")[0], slot("revision", sequence));
    bind(ids, valuesAt(receipt, "authoritative_commit_ids")[0], slot("commit", sequence));
    bind(
      digests,
      valueAt(valueAt(receipt, "command_digest"), "value_hex_lowercase"),
      slot("digest-command", sequence),
    );
  });
  observed.authority.activities.forEach((activity, index) => {
    const sequence = index + 1;
    bind(ids, valueAt(activity, "event_id"), slot("event", sequence));
    bind(ids, valueAt(activity, "command_id"), slot("command", sequence));
    bind(ids, valueAt(activity, "correlation_id"), slot("correlation", sequence));
    bind(
      digests,
      valueAt(valueAt(activity, "payload_digest"), "value_hex_lowercase"),
      slot("digest-event", sequence),
    );
  });
  const journal = observed.recover.journal;
  const partitionParts = stringAt(firstRow(journal.groups, "Journal groups"), "journal_partition_id")
    .split(":");
  bind(ids, partitionParts[2], "editor-session");
  bind(ids, partitionParts[4], "client-session");
  journal.groups.forEach((group, index) => {
    const sequence = index + 1;
    bind(ids, valueAt(group, "journal_submission_group_id"), slot("group", sequence));
    bind(
      ids,
      valueAt(valueAt(group, "payload_collection"), "collection_fence_id"),
      slot("fence", sequence),
    );
    const settlement = valueAt(group, "settlement");
    bind(
      ids,
      valueAt(valueAt(settlement, "installed_base_snapshot"), "snapshot_id"),
      slot("snapshot", sequence),
    );
    const frozenRequest = valueAt(group, "frozen_request_body");
    bind(ids, valueAt(frozenRequest, "undo_group_id"), slot("undo", sequence));
    bind(
      ids,
      valueAt(frozenRequest, "completed_intent_record_id"),
      slot("intent", sequence),
    );
    bind(
      digests,
      valueAt(valueAt(group, "frozen_payload_coverage_digest"), "value_hex_lowercase"),
      slot("digest-coverage", sequence),
    );
  });
  journal.intents.forEach((intent, index) => {
    bind(ids, valueAt(intent, "completed_intent_record_id"), slot("intent", index + 1));
    bind(ids, valueAt(intent, "payload_chain_ref"), slot("chain", index + 1));
    bind(
      ids,
      valueAt(valueAt(intent, "undo_group_binding"), "undo_group_id"),
      slot("undo", index + 1),
    );
    bind(ids, valueAt(intent, "base_snapshot_id"), slot("snapshot", index));
  });
  return rewrite(observed, ids, digests);
}

export function expectedStage1Journey() {
  const page = {
    alert: false,
    bootState: "project-ready",
    chapter: "Chapter A",
    heading: "Project A",
    readOnly: false,
  };
  return {
    id: "S1-JRN-001",
    open: {
      ...page,
      pending: pendingProjection(OPEN_BODY, "clean", 0, INITIAL_REVISION),
    },
    input: pendingProjection(AFTER_TYPE, "saving", 1, INITIAL_REVISION),
    afterType: pendingProjection(AFTER_TYPE, "saved", 0, "revision-1"),
    afterImeInput: pendingProjection(AFTER_IME, "saving", 1, "revision-1"),
    afterIme: pendingProjection(AFTER_IME, "saved", 0, "revision-2"),
    afterPasteInput: pendingProjection(AFTER_PASTE, "saving", 1, "revision-2"),
    settle: {
      pending: pendingProjection(AFTER_PASTE, "saved", 0, "revision-3"),
      journal: journalSnapshot(3),
    },
    interrupt: {
      pending: pendingProjection(AFTER_UNSETTLED, "saving", 1, "revision-3"),
      journal: journalSnapshot(3, true),
    },
    recover: {
      pending: pendingProjection(AFTER_UNSETTLED, "saving", 1, "revision-3"),
      saved: {
        ...page,
        pending: pendingProjection(AFTER_UNSETTLED, "saved", 0, "revision-4"),
      },
      journal: journalSnapshot(4),
    },
    authority: {
      receipts: [0, 1, 2, 3].map(appliedReceipt),
      effects: [0, 1, 2, 3].map(appliedEffect),
      activities: [0, 1, 2, 3].map(appliedActivity),
      manuscript: { revision_id: "revision-4", body: AFTER_UNSETTLED },
    },
  };
}

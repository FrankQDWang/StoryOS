import type {
  ApplyAuthorEditRequest,
  ApplyAuthorEditEffect,
  AuthorEditUnit,
  DigestValue,
  DomainReceipt,
  EditorBaseSnapshot,
  EditorSessionBinding,
  EditorWriterProjection,
  GetEditorSessionResponse,
  GetChapterResponse,
  GetProjectResponse,
  ProjectScope,
  Release1ProtocolProfile,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export type InputOrigin =
  | "typing"
  | "deletion"
  | "selection_replacement"
  | "paste"
  | "cut"
  | "composition_confirmation";

export interface JournalPartition {
  journal_partition_id: string;
  project_scope: ProjectScope;
  editor_session_id: string;
  writer_generation: string;
  client_session_binding_ref: string;
  client_session_generation: string;
  client_contract_revision: string;
  security_policy_revision: string;
  limit_profile_revision: string;
  created_at: string;
  disposition: "current_writer_open" | "read_only_observer";
}

export interface JournalIntentRecord extends Record<string, unknown> {
  completed_intent_record_id: string;
  local_intent_sequence: number;
  journal_partition_id: string;
  project_scope: ProjectScope;
  editor_session_id: string;
  writer_generation: string;
  limit_profile_revision: string;
  batch_policy_revision: string;
  input_origin: InputOrigin;
  chapter_object_id: string;
  base_snapshot_id: string;
  base_activity_position: string;
  target_refs: string[];
  expected_authoritative_heads: string[];
  expected_proposal_heads: string[];
  proposal_anchors: [];
  observed_ownership_partition: string;
  author_edit_unit?: AuthorEditUnit;
  retry_source: { kind: "fresh_editor_intent" };
  editor_contract_revision: string;
  undo_group_binding: { kind: "direct_author_input"; undo_group_id: string };
  payload_chain_ref: string;
  payload_digest: DigestValue;
  projection_dependency: { snapshot_id: string; prior_sequence: number };
  created_at: string;
}

export interface JournalPatchReference {
  patch_id: string;
  completed_intent_record_id: string;
  local_intent_sequence: number;
  normalized_primitives?: AuthorEditUnit["normalized_primitives"];
  resulting_payload_digest: DigestValue;
}

export interface JournalPayloadChain extends Record<string, unknown> {
  payload_chain_id: string;
  journal_partition_id: string;
  checkpoint_ref: {
    chapter_object_id: string;
    materialized_payload?: string;
    materialized_payload_digest: DigestValue;
    source_snapshot_id: string;
    source_heads: string[];
  };
  ordered_patch_refs: JournalPatchReference[];
  payload_collection?: { kind: "collected"; collection_fence_id: string };
}

export interface JournalCoverage {
  local_intent_sequence: number;
  intent_record_ref: string;
  payload_digest: DigestValue;
}

export type ReconciliationEvidence =
  | { kind: "no_outcome_observed" }
  | { kind: "challenge_issued"; expires_at: string }
  | {
      kind: "admission_committed";
      command_id: string;
      author_command_admission_id: string;
      reconciliation_required: true;
    };

export interface OutcomeQueryReconciliation {
  kind: "outcome_query_unresolved";
  strongest: ReconciliationEvidence;
}

export type SubmissionSettlement =
  | (Record<string, unknown> & { kind: "unsettled" })
  | (Record<string, unknown> & {
      kind: "applied_receipt_settled";
      command_id: string;
      author_command_admission_id: string;
      receipt: DomainReceipt;
      authoritative_revision: { revision_id: string; body: string };
      authoritative_commit_id: string;
      author_action_sequence: string;
      project_activity_position: string;
      installed_base_snapshot: EditorBaseSnapshot;
    })
  | (Record<string, unknown> & {
      kind: "zero_authority_receipt_settled";
      command_id: string;
      author_command_admission_id: string;
      receipt: DomainReceipt;
      effect: Exclude<ApplyAuthorEditEffect, { kind: "authoritative_applied" }>;
    })
  | (Record<string, unknown> & {
      kind: "outcome_query_rejected_no_admission";
      reason: "challenge_expired_unconsumed";
    });

export interface JournalSubmissionGroup extends Record<string, unknown> {
  journal_submission_group_id: string;
  journal_partition_id: string;
  project_scope: ProjectScope;
  editor_session_id: string;
  writer_generation: string;
  batch_policy_revision: string;
  ordered_coverage: JournalCoverage[];
  covered_sequence_range: { first: number; last: number };
  action_class: "direct_editor_action";
  api_major: 1;
  method: "POST";
  route_template: string;
  command_schema: string;
  command_kind: "applyAuthorEdit";
  digest_profile: string;
  idempotency_key: string;
  frozen_request_body: ApplyAuthorEditRequest;
  frozen_request_digest: DigestValue;
  frozen_payload_coverage_digest: DigestValue;
  settlement: SubmissionSettlement;
  reconciliation?: OutcomeQueryReconciliation;
  payload_collection?: { kind: "collected"; collection_fence_id: string };
  frozen_at: string;
}

export interface JournalSnapshot {
  watermark: { key: string; value: number } | undefined;
  activeBase: EditorBaseSnapshot | undefined;
  records: JournalIntentRecord[];
  payloadChains: JournalPayloadChain[];
  groups: JournalSubmissionGroup[];
  fences: unknown[];
}

export interface ValidatedJournalSnapshot extends JournalSnapshot {
  bodyBySequence: Map<number, string>;
  covered: Set<number>;
}

export interface PendingEditProjection {
  body: string;
  save_state: "clean" | "saving" | "saved" | "needs_attention";
  unsettled_intent_count: number;
  authoritative_revision_id: string;
}

export interface EditorWorkspace {
  database: IDBDatabase;
  partition: JournalPartition;
  session: GetEditorSessionResponse;
  cryptoImpl: Crypto;
  maxJsonStringUtf8Bytes: number;
  submitQueue?: Promise<PendingEditProjection | void>;
}

export interface EditorReadyState extends EditorWorkspace {
  kind: "editor-ready";
  pending: PendingEditProjection;
}

export interface EditorReadOnlyState {
  kind: "editor-read-only-recovery";
  code: "editor_session_read_only" | "local_journal_unavailable";
  editor_session?: EditorSessionBinding;
  writer?: EditorWriterProjection;
  pending?: PendingEditProjection;
  error?: unknown;
}

export type EditorState = EditorReadyState | EditorReadOnlyState;

export interface ReplaceSelectionEdit {
  from: number;
  to: number;
  text: string;
  resultingBody: string;
  inputOrigin?: InputOrigin;
  undoGroupId?: string;
  createdAt?: string;
}

export interface TransportCapsule extends Record<string, unknown> {
  exact_transport_retry_capsule_id: string;
  journal_submission_group_id: string;
  project_scope: ProjectScope;
  client_session_binding_ref: string;
  client_session_generation: string;
  request_identity: {
    api_major: number;
    method: string;
    route_template: string;
    command_schema: string;
    command_kind: string;
  };
  exact_request_body_ref: string;
  canonical_command_digest: DigestValue;
  digest_profile: string;
  exact_client_controlled_headers: Record<string, string>;
  challenge_expires_at: string;
  committed_before_send_at: string;
  disposition: { kind: "available" };
}

export interface AvailableCommandProof {
  nonce: string;
  expiresAt: string;
  idempotencyKey: string;
  capsule: TransportCapsule;
}

export interface OutcomeQueryAttempt extends Record<string, unknown> {
  outcome_query_attempt_id: string;
  journal_submission_group_id: string;
  exact_transport_retry_capsule_id: string;
  request_identity: {
    method: "GET";
    route_template: string;
    request_schema: string;
    idempotency_key: string;
  };
  started_at: string;
  outcome: { kind: "in_flight" };
}

export interface ProjectActivityEvent {
  envelope_version: 1;
  activity_profile: string;
  event_id: string;
  event_schema: string;
  event_kind: string;
  project_scope: ProjectScope;
  requester_user_id: string;
  actor: { kind: "author"; id: string };
  project_sequence: string;
  stream_sequence: string;
  agent_run_id: null;
  run_step_id: null;
  run_sequence: null;
  aggregate_ref: { kind: "chapter"; id: string };
  correlation_id: string;
  causation: { kind: "command"; id: string };
  command_id: string;
  receipt_ref: { kind: "domain_receipt"; id: string };
  occurred_at: string;
  recorded_at: string;
  payload: {
    chapter_id: string;
    authoritative_revision_id: string;
    authoritative_commit_id: string;
    author_action_sequence: string;
  };
  payload_digest: DigestValue;
  application_wire_record_ref: string;
  limit_profile_revision: string;
}

export interface ProjectActivityFrame {
  id: string;
  event: "storyos.project-activity";
  data: unknown;
}

export interface ProjectActivityIngest {
  replay_generation: string;
  processed_through_stream_sequence: string;
  events: ProjectActivityEvent[];
  held: ProjectActivityEvent[];
}

export interface StateDifference extends Record<string, unknown> {
  path: string;
  expected?: unknown;
  received?: unknown;
}

export interface ProtocolBlockedState {
  kind: "protocol-blocked";
  code:
    | "protocol_identity_missing"
    | "protocol_upgrade_required"
    | "protocol_capabilities_incompatible"
    | "protocol_unavailable";
  heading: string;
  message: string;
  details: StateDifference[];
}

export interface ProtectedReadyState {
  kind: "protected-ready";
  profile: Release1ProtocolProfile;
}

export type ProtocolBootState = ProtocolBlockedState | ProtectedReadyState;

export interface ProjectBlockedState {
  kind: "project-blocked";
  code: "project_unavailable" | "project_url_invalid";
  heading: string;
  message: string;
  details?: StateDifference[];
}

export interface ProjectReadyState {
  kind: "project-ready";
  profile: Release1ProtocolProfile;
  project: GetProjectResponse;
  chapter: GetChapterResponse;
  editor: EditorState;
}

export type ControlledProjectState =
  | ProtocolBlockedState
  | ProjectBlockedState
  | ProjectReadyState;

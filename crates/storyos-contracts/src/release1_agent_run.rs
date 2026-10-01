use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, QueryOperation};
use crate::release1_readable_export::ExportAcknowledgement;

pub const CREATE_AGENT_RUN_REQUEST_SCHEMA_ID: &str = "storyos.command.create-agent-run.request.v2";
pub const CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.create-agent-run.response.v2";
pub const CREATE_AGENT_RUN_DIGEST_PROFILE: &str = "storyos.command.createAgentRun.jcs.v1";
pub const GET_AGENT_RUN_REQUEST_SCHEMA_ID: &str = "storyos.query.agent-run.request.v2";
pub const GET_AGENT_RUN_RESPONSE_SCHEMA_ID: &str = "storyos.query.agent-run.response.v2";

pub(super) const CREATE_AGENT_RUN: QueryOperation = QueryOperation {
    operation_id: "createAgentRun",
    method: "POST",
    path: "/api/v1/projects/{project_id}/agent-runs",
    request_schema: CREATE_AGENT_RUN_REQUEST_SCHEMA_ID,
    response_schema: CREATE_AGENT_RUN_RESPONSE_SCHEMA_ID,
    responses: &[
        (202, "Bounded assistance request accepted"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or conversation conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Assistance request refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.createAgentRun.positive.v1",
        "storyos.golden.createAgentRun.invalid.v1",
        "storyos.golden.createAgentRun.boundary.v1",
    ],
};

pub(super) const GET_AGENT_RUN: QueryOperation = QueryOperation {
    operation_id: "getAgentRun",
    method: "GET",
    path: "/api/v1/projects/{project_id}/agent-runs/{run_id}",
    request_schema: GET_AGENT_RUN_REQUEST_SCHEMA_ID,
    response_schema: GET_AGENT_RUN_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Durable AgentRun status"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Active release conflict"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Request refused"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.getAgentRun.positive.v1",
        "storyos.golden.getAgentRun.invalid.v1",
        "storyos.golden.getAgentRun.boundary.v1",
    ],
};

pub const CREATE_AGENT_RUN_PATH: &str = CREATE_AGENT_RUN.path;
pub const CREATE_AGENT_RUN_METHOD: &str = CREATE_AGENT_RUN.method;
pub const GET_AGENT_RUN_PATH: &str = GET_AGENT_RUN.path;
pub const GET_AGENT_RUN_METHOD: &str = GET_AGENT_RUN.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConversationSelection {
    New,
    Existing { conversation_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AuthorMessage {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistanceWorkingTarget {
    CurrentChapter {
        chapter_id: String,
    },
    ProposalCandidate {
        source_chapter_id: String,
        target: ProposalCandidateTarget,
    },
    PassageCollection {
        source_chapter_id: String,
        #[schemars(length(min = 1, max = 10_001))]
        targets: Vec<PassageTarget>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProposalCandidateTarget {
    pub proposal_id: String,
    pub operation_id: String,
    pub revision_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PassageTarget {
    pub chapter_id: String,
    pub base_authoritative_revision_id: String,
    #[schemars(length(min = 1, max = 10_001))]
    pub manuscript_block_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstructionBinding {
    Absent,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssistanceCause {
    AuthorRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentRunInput {
    pub conversation: ConversationSelection,
    pub author_message: AuthorMessage,
    pub working_target: AssistanceWorkingTarget,
    pub instruction: InstructionBinding,
    pub cause: AssistanceCause,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentRunRequest {
    pub command_schema: String,
    pub create_agent_run_input: CreateAgentRunInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentRunRef {
    AgentRun { run_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CreateAgentRunEffect {
    Admitted {
        project_agent_id: String,
        conversation_id: String,
        memory_settings_revision: String,
        run_id: String,
        project_activity_position: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CreateAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub acknowledgement: ExportAcknowledgement,
    pub operation_ref: Option<AgentRunRef>,
    pub project: ControlledProject,
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub effect: CreateAgentRunEffect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunStatus {
    Queued,
    Claimed,
    Waiting,
    Paused,
    Completed,
    Refused,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContextPurpose {
    CurrentPassageAssistance,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContextSourceClass {
    HostControl,
    AuthorInstruction,
    WorkingTarget,
    InstructionBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMode {
    ExactRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextSufficiency {
    Complete,
    Blocked { reasons: Vec<ContextBlockReason> },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextBlockReason {
    ExactRequiredOverLimit { source_class: ContextSourceClass },
    RequiredInstructionRevisionUnavailable,
    WorkingTargetRevisionUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextRejectionReason {
    OverItemTokenLimit,
    RequiredRevisionUnavailable,
    WorkingTargetRevisionUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DestinationIo {
    None,
    HostFake,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalManifestRef {
    Absent,
    Present { manifest_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceAvailability {
    Current,
    Unavailable,
    Superseded { current_revision_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TokenCountingProfileInspect {
    pub profile_revision: String,
    pub algorithm_revision: String,
    pub item_token_limit: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContextSourceInspect {
    pub source_class: ContextSourceClass,
    pub source_version: String,
    pub token_count: String,
    pub eligible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContextProjectionInspect {
    pub source_class: ContextSourceClass,
    pub source_version: String,
    pub projection_mode: ProjectionMode,
    pub token_count: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContextRejectionInspect {
    pub source_class: ContextSourceClass,
    pub source_version: String,
    pub token_count: String,
    pub reason: ContextRejectionReason,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HostControlInspect {
    pub distinct_from_destination: bool,
    pub destination_visible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CurrentAvailabilityInspect {
    pub working_target: SourceAvailability,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AgentRunContextInspect {
    pub operation_requirement_id: String,
    pub input_snapshot_id: String,
    pub purpose: ContextPurpose,
    pub token_counting_profile: TokenCountingProfileInspect,
    pub sufficiency: ContextSufficiency,
    pub considered: Vec<ContextSourceInspect>,
    pub selected: Vec<ContextProjectionInspect>,
    pub rejected: Vec<ContextRejectionInspect>,
    pub host_control: HostControlInspect,
    pub assembly_manifest_id: String,
    pub destination_context_manifest: OptionalManifestRef,
    pub outbound_disclosure_manifest: OptionalManifestRef,
    pub destination_io: DestinationIo,
    pub current_availability: CurrentAvailabilityInspect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub passage_targets: Option<Vec<PassageTarget>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub candidate_target: Option<ProposalCandidateTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalContinuationInspect {
    Absent,
    Present { continuation_binding_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAvailability {
    Current,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttemptEvidence {
    SentContent {
        attempt_id: String,
        availability: EvidenceAvailability,
        content: String,
    },
    StoredReference {
        attempt_id: String,
        availability: EvidenceAvailability,
        reference_id: String,
    },
    ProviderReport {
        attempt_id: String,
        availability: EvidenceAvailability,
        report: String,
    },
    ProviderOpaque {
        attempt_id: String,
        availability: EvidenceAvailability,
        unknown_facts: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalOpenedProposalInspect {
    Absent,
    Present { proposal_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProseChangeLocationInspect {
    pub chapter_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
    pub candidate_text: String,
    pub explanation: String,
    pub outcome: ProseChangeLocationOutcome,
    pub current: Option<ProseChangeLocationCurrent>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProseChangeLocationOutcome {
    Revised {
        proposal_id: String,
        operation_id: String,
        revision_id: String,
        prior_revision_id: String,
        validation_receipt_id: String,
    },
    Opened {
        proposal_id: String,
        operation_id: String,
        revision_id: String,
        validation_receipt_id: String,
    },
    Refused {
        reason: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProseChangeLocationCurrent {
    pub revision_id: String,
    pub generation: String,
    pub validation: String,
    pub closure: String,
    pub resolution: String,
    pub reservation_state: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalDecisionInspect {
    Absent,
    ExecutionRefused {
        capability: String,
    },
    Advisory {
        decision_id: String,
        selected: bool,
        text: String,
        continuation: OptionalContinuationInspect,
    },
    ProseChange {
        decision_id: String,
        selected: bool,
        text: String,
        producer_input: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        locations: Option<Vec<ProseChangeLocationInspect>>,
        continuation: OptionalContinuationInspect,
        authoritative: bool,
        opened_proposal: OptionalOpenedProposalInspect,
    },
    Clarification {
        decision_id: String,
        selected: bool,
        question: String,
        required_reply: String,
        continuation: OptionalContinuationInspect,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationInputMappingInspect {
    None,
    Incremental,
    Full,
    NewTransport,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContinuationAdmissionInspect {
    pub processing_destination_identity: String,
    pub evidence_revision: String,
    pub model_registration_revision: String,
    pub adapter_mapping: String,
    pub project_model_use_binding_revision: String,
    pub external_compatibility_decision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalModelAttemptInspect {
    Absent,
    Present {
        model_attempt_id: String,
        destination_attempt_id: String,
        outbound_disclosure_event_id: String,
        model_invocation_id: String,
        dispatch_state: String,
        prior_continuation: OptionalContinuationInspect,
        known_prior_continuation: OptionalContinuationInspect,
        input_mapping: ContinuationInputMappingInspect,
        admission: Box<ContinuationAdmissionInspect>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ActiveCompactionInstallState {
    Staged,
    Installed,
    Refused,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ActiveCompactionMappingKind {
    HostManaged,
    Native,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActiveCompactionKnownInput {
    ModelAttempt {
        id: String,
    },
    Manifest {
        id: String,
    },
    Projection {
        source_class: String,
        source_version: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalCompactionInstallInspect {
    Absent,
    Present {
        run_step_id: String,
        model_invocation_id: String,
        model_attempt_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum OptionalActiveCompactionInspect {
    Absent,
    Present {
        compaction_id: String,
        install_state: ActiveCompactionInstallState,
        prior_model_attempt_id: String,
        prior_manifest_id: String,
        prior_run_step_id: String,
        producer_model_attempt_id: String,
        producer_manifest_id: String,
        producer_invocation_id: String,
        producer: String,
        mapping_kind: ActiveCompactionMappingKind,
        mapping_revision: String,
        known_inputs: Vec<ActiveCompactionKnownInput>,
        output_text: String,
        usage: AgentRunUsageInspect,
        loss_facts: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refusal_reason: Option<String>,
        installed: OptionalCompactionInstallInspect,
        preserved_item_ids: Vec<String>,
        admission: ContinuationAdmissionInspect,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceRecoveryDisposition {
    Rebuilt,
    Blocked,
    UnknownCreate,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalReferenceRecoveryInspect {
    Absent,
    Present {
        recovery_id: String,
        disposition: ReferenceRecoveryDisposition,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_reason: Option<String>,
        predecessor_run_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        predecessor_continuation_binding_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_step_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_invocation_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model_attempt_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assembly_manifest_id: Option<String>,
        lossless_provider_reconstruction: bool,
        semantic_erasure: bool,
        opaque_reused: bool,
        covered_content_included: bool,
        predecessor_terminal: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OriginalResultRetrievalDisposition {
    KeptUnknown,
    EvidenceOnly,
    Settled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalOriginalResultRetrievalInspect {
    Absent,
    Present {
        reconciliation_id: String,
        disposition: OriginalResultRetrievalDisposition,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keep_reason: Option<String>,
        original_model_attempt_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        response_reference_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retrieval_attempt_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        assembly_manifest_id: Option<String>,
        repeats_original_create: bool,
        resumes_stream: bool,
        proves_create_idempotency: bool,
        supplies_decision: bool,
        supplies_tool_call: bool,
        advances_continuation: bool,
        reservation_released: bool,
        usage_kind: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum UnknownCreateSuccessorDisposition {
    Fenced,
    Dispatched,
    Paused,
    Prohibited,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalUnknownCreateSuccessorInspect {
    Absent,
    Present {
        recovery_id: String,
        disposition: UnknownCreateSuccessorDisposition,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pause_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lookup_unavailable_reason: Option<String>,
        predecessor_model_attempt_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        successor_model_attempt_id: Option<String>,
        model_invocation_id: String,
        predecessor_fenced: bool,
        allowance_consumed: bool,
        predecessor_usage_kind: String,
        predecessor_reservation_released: bool,
        successor_settles_predecessor: bool,
        supplies_tool_call: bool,
        advances_predecessor_continuation: bool,
        reuses_changed_context: bool,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetAgentRunRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_attempt_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AgentRunStreamItemInspect {
    pub item_id: String,
    pub role: String,
    pub state: String,
    pub phase: String,
    pub text: Option<String>,
    pub summary: Option<String>,
    pub call_id: Option<String>,
    pub arguments: Option<String>,
    pub refusal: Option<String>,
    pub hosted_report: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AgentRunUsageInspect {
    pub kind: String,
}

/// Settings evidence from the exact revision captured by this Run.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapturedMemorySettingsInspect {
    Available {
        memory_settings_revision: String,
        use_enabled: bool,
        contribution_enabled: bool,
    },
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub captured_memory_settings: CapturedMemorySettingsInspect,
    pub run_id: String,
    pub status: AgentRunStatus,
    pub context: AgentRunContextInspect,
    pub decision: OptionalDecisionInspect,
    pub model_attempt: OptionalModelAttemptInspect,
    pub active_compaction: OptionalActiveCompactionInspect,
    pub reference_recovery: OptionalReferenceRecoveryInspect,
    pub original_result_retrieval: OptionalOriginalResultRetrievalInspect,
    pub unknown_create_successor: OptionalUnknownCreateSuccessorInspect,
    pub evidence: Vec<AttemptEvidence>,
    pub items: Vec<AgentRunStreamItemInspect>,
    pub usage: AgentRunUsageInspect,
    pub redaction_profile: String,
}

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
    CurrentChapter { chapter_id: String },
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
    Completed,
    Refused,
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
        continuation: OptionalContinuationInspect,
        authoritative: bool,
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
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalModelAttemptInspect {
    Absent,
    Present {
        model_attempt_id: String,
        destination_attempt_id: String,
        outbound_disclosure_event_id: String,
        model_invocation_id: String,
        dispatch_state: String,
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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GetAgentRunResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub run_id: String,
    pub status: AgentRunStatus,
    pub context: AgentRunContextInspect,
    pub decision: OptionalDecisionInspect,
    pub model_attempt: OptionalModelAttemptInspect,
    pub evidence: Vec<AttemptEvidence>,
    pub items: Vec<AgentRunStreamItemInspect>,
    pub usage: AgentRunUsageInspect,
    pub redaction_profile: String,
}

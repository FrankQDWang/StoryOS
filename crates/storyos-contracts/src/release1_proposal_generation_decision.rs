use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, QueryOperation};

pub const COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID: &str =
    "storyos.command.complete-ready-partial-proposal.request.v1";
pub const COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.complete-ready-partial-proposal.response.v1";
pub const COMPLETE_READY_PARTIAL_PROPOSAL_DIGEST_PROFILE: &str =
    "storyos.command.completeReadyPartialProposal.jcs.v1";
pub const CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID: &str =
    "storyos.command.continue-proposal-generation.request.v1";
pub const CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.continue-proposal-generation.response.v1";
pub const CONTINUE_PROPOSAL_GENERATION_DIGEST_PROFILE: &str =
    "storyos.command.continueProposalGeneration.jcs.v1";

pub(super) const COMPLETE_READY_PARTIAL_PROPOSAL: QueryOperation = QueryOperation {
    operation_id: "completeReadyPartialProposal",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-completions",
    request_schema: COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
    response_schema: COMPLETE_READY_PARTIAL_PROPOSAL_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Generation completion settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Generation completion refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.completeReadyPartialProposal.positive.v1",
        "storyos.golden.completeReadyPartialProposal.invalid.v1",
        "storyos.golden.completeReadyPartialProposal.boundary.v1",
    ],
};

pub(super) const CONTINUE_PROPOSAL_GENERATION: QueryOperation = QueryOperation {
    operation_id: "continueProposalGeneration",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
    request_schema: CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
    response_schema: CONTINUE_PROPOSAL_GENERATION_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Generation continuation settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Generation continuation refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.continueProposalGeneration.positive.v1",
        "storyos.golden.continueProposalGeneration.invalid.v1",
        "storyos.golden.continueProposalGeneration.boundary.v1",
    ],
};

pub const COMPLETE_READY_PARTIAL_PROPOSAL_PATH: &str = COMPLETE_READY_PARTIAL_PROPOSAL.path;
pub const COMPLETE_READY_PARTIAL_PROPOSAL_METHOD: &str = COMPLETE_READY_PARTIAL_PROPOSAL.method;
pub const CONTINUE_PROPOSAL_GENERATION_PATH: &str = CONTINUE_PROPOSAL_GENERATION.path;
pub const CONTINUE_PROPOSAL_GENERATION_METHOD: &str = CONTINUE_PROPOSAL_GENERATION.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CompleteReadyPartialProposalInput {
    pub proposal_revision_id: String,
    pub generation_id: String,
    pub expected_candidate_digest: String,
    pub last_applied_stream_seq: String,
    pub expected_target_revisions: Vec<String>,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CompleteReadyPartialProposalRequest {
    pub command_schema: String,
    pub complete_ready_partial_proposal_input: CompleteReadyPartialProposalInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContinueProposalGenerationInput {
    pub proposal_revision_id: String,
    pub prior_generation_id: String,
    pub expected_generation_state: String,
    pub expected_candidate_digest: String,
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_target_revisions: Vec<String>,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContinueProposalGenerationRequest {
    pub command_schema: String,
    pub continue_proposal_generation_input: ContinueProposalGenerationInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProposalGenerationReceiptResult {
    ProposalGenerationCompleted,
    ProposalGenerationStarted,
    Conflicted,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProposalGenerationReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub author_command_admission_id: String,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub expected_target_revisions: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub result: ProposalGenerationReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CompleteReadyPartialProposalRefusalReason {
    StaleProposalRevision,
    NotEligible,
    NotReadyPartial,
    StaleGeneration,
    StaleCandidate,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContinueProposalGenerationRefusalReason {
    StaleProposalRevision,
    NotEligible,
    NotContinuable,
    StaleGeneration,
    StaleCandidate,
    OperationNotPending,
    DuplicateIdentities,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProposalGenerationConflictReason {
    ChangedHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProposalGenerationUndoDisposition {
    Forward,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum CompleteReadyPartialProposalEffect {
    Completed {
        author_action_sequence: String,
        undo_disposition: ProposalGenerationUndoDisposition,
        generation_id: String,
        prior_generation_state: String,
        resulting_generation_state: String,
        preserved_validation: String,
        preserved_closure: String,
        preserved_operation_resolution: String,
        generation_event_ref: String,
    },
    Conflicted {
        reason: ProposalGenerationConflictReason,
    },
    Refused {
        reason: CompleteReadyPartialProposalRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum ContinueProposalGenerationEffect {
    Started {
        author_action_sequence: String,
        undo_disposition: ProposalGenerationUndoDisposition,
        prior_generation_id: String,
        new_generation_id: String,
        prior_generation_state: String,
        resulting_generation_state: String,
        prior_run_id: String,
        resulting_run_id: String,
        preserved_validation: String,
        preserved_closure: String,
        preserved_operation_resolution: String,
        generation_event_ref: String,
    },
    Conflicted {
        reason: ProposalGenerationConflictReason,
    },
    Refused {
        reason: ContinueProposalGenerationRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CompleteReadyPartialProposalResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: ProposalGenerationReceipt,
    pub project: ControlledProject,
    pub effect: CompleteReadyPartialProposalEffect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ContinueProposalGenerationResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: ProposalGenerationReceipt,
    pub project: ControlledProject,
    pub effect: ContinueProposalGenerationEffect,
}

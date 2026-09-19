use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{
    AuthoritativeChapterRevision, ControlledProject, DigestValue, QueryOperation,
};

pub const ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID: &str = "storyos.command.accept-proposal.request.v1";
pub const ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID: &str = "storyos.command.accept-proposal.response.v1";
pub const ACCEPT_PROPOSAL_DIGEST_PROFILE: &str = "storyos.command.acceptProposal.jcs.v1";

pub(super) const ACCEPT_PROPOSAL: QueryOperation = QueryOperation {
    operation_id: "acceptProposal",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
    request_schema: ACCEPT_PROPOSAL_REQUEST_SCHEMA_ID,
    response_schema: ACCEPT_PROPOSAL_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Acceptance settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Acceptance refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.acceptProposal.positive.v1",
        "storyos.golden.acceptProposal.invalid.v1",
        "storyos.golden.acceptProposal.boundary.v1",
    ],
};

pub const ACCEPT_PROPOSAL_PATH: &str = ACCEPT_PROPOSAL.path;
pub const ACCEPT_PROPOSAL_METHOD: &str = ACCEPT_PROPOSAL.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AcceptProposalInput {
    pub proposal_revision_id: String,
    pub validation_receipt_id: String,
    pub selected_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AcceptProposalRequest {
    pub command_schema: String,
    pub accept_proposal_input: AcceptProposalInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceReceiptResult {
    Applied,
    Invalid,
    Conflicted,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub author_command_admission_id: String,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub validation_receipt_id: String,
    pub selected_operation_ids: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub condition_refs: Vec<String>,
    pub result: AcceptanceReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AcceptProposalRefusalReason {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    OperationNotPending,
    DuplicateIdentities,
    MissingRequiredDependencies,
    IncompleteBundleClosure,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AcceptProposalInvalidReason {
    InvalidValidation,
    AlteredCandidate,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AcceptProposalConflictReason {
    ChangedHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AcceptProposalEffect {
    Applied {
        author_action_sequence: String,
        authoritative_commit_id: String,
        authoritative_revision: AuthoritativeChapterRevision,
        project_activity_position: String,
    },
    Invalid {
        reason: AcceptProposalInvalidReason,
    },
    Conflicted {
        reason: AcceptProposalConflictReason,
    },
    Refused {
        reason: AcceptProposalRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AcceptProposalResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: AcceptanceReceipt,
    pub project: ControlledProject,
    pub effect: AcceptProposalEffect,
}

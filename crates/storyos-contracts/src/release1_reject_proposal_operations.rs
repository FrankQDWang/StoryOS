use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, QueryOperation};

pub const REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID: &str =
    "storyos.command.reject-proposal-operations.request.v1";
pub const REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.reject-proposal-operations.response.v1";
pub const REJECT_PROPOSAL_OPERATIONS_DIGEST_PROFILE: &str =
    "storyos.command.rejectProposalOperations.jcs.v1";

pub(super) const REJECT_PROPOSAL_OPERATIONS: QueryOperation = QueryOperation {
    operation_id: "rejectProposalOperations",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
    request_schema: REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
    response_schema: REJECT_PROPOSAL_OPERATIONS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Rejection settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Rejection refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.rejectProposalOperations.positive.v1",
        "storyos.golden.rejectProposalOperations.invalid.v1",
        "storyos.golden.rejectProposalOperations.boundary.v1",
    ],
};

pub const REJECT_PROPOSAL_OPERATIONS_PATH: &str = REJECT_PROPOSAL_OPERATIONS.path;
pub const REJECT_PROPOSAL_OPERATIONS_METHOD: &str = REJECT_PROPOSAL_OPERATIONS.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BoundedAuthorNote {
    Omitted,
    Present { text: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposalRejectionReason {
    AuthorDeclined { note: BoundedAuthorNote },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RejectProposalOperationsInput {
    pub proposal_revision_id: String,
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_target_revisions: Vec<String>,
    pub rejection_reason: ProposalRejectionReason,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RejectProposalOperationsRequest {
    pub command_schema: String,
    pub reject_proposal_operations_input: RejectProposalOperationsInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReceiptResult {
    Resolved,
    Conflicted,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RejectionReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub author_command_admission_id: String,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_target_revisions: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub result: RejectionReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RejectProposalOperationsRefusalReason {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    OperationNotPending,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RejectProposalOperationsConflictReason {
    ChangedHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AuthorUndoDisposition {
    Forward,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RejectProposalOperationsEffect {
    Resolved {
        author_action_sequence: String,
        undo_disposition: AuthorUndoDisposition,
        operation_ids: Vec<String>,
        prior_resolution: String,
        resulting_resolution: String,
        rejection_reason: ProposalRejectionReason,
        preserved_generation: String,
        preserved_validation: String,
        preserved_closure: String,
        resolution_event_refs: Vec<String>,
    },
    Conflicted {
        reason: RejectProposalOperationsConflictReason,
    },
    Refused {
        reason: RejectProposalOperationsRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RejectProposalOperationsResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: RejectionReceipt,
    pub project: ControlledProject,
    pub effect: RejectProposalOperationsEffect,
}

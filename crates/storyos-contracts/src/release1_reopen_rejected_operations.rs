use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, QueryOperation};
use crate::release1_reject_proposal_operations::AuthorUndoDisposition;

pub const REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID: &str =
    "storyos.command.reopen-rejected-operations.request.v1";
pub const REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.reopen-rejected-operations.response.v1";
pub const REOPEN_REJECTED_OPERATIONS_DIGEST_PROFILE: &str =
    "storyos.command.reopenRejectedOperations.jcs.v1";

pub(super) const REOPEN_REJECTED_OPERATIONS: QueryOperation = QueryOperation {
    operation_id: "reopenRejectedOperations",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
    request_schema: REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID,
    response_schema: REOPEN_REJECTED_OPERATIONS_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Reopen settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Reopen refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.reopenRejectedOperations.positive.v1",
        "storyos.golden.reopenRejectedOperations.invalid.v1",
        "storyos.golden.reopenRejectedOperations.boundary.v1",
    ],
};

pub const REOPEN_REJECTED_OPERATIONS_PATH: &str = REOPEN_REJECTED_OPERATIONS.path;
pub const REOPEN_REJECTED_OPERATIONS_METHOD: &str = REOPEN_REJECTED_OPERATIONS.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenRejectedOperationsInput {
    pub proposal_revision_id: String,
    pub selected_rejected_operation_ids: Vec<String>,
    pub rejection_event_refs: Vec<String>,
    pub expected_target_revisions: Vec<String>,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenRejectedOperationsRequest {
    pub command_schema: String,
    pub reopen_rejected_operations_input: ReopenRejectedOperationsInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenReceiptResult {
    Resolved,
    Conflicted,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub author_command_admission_id: String,
    pub proposal_id: String,
    pub source_proposal_revision_id: String,
    pub resulting_proposal_revision_id: String,
    pub selected_rejected_operation_ids: Vec<String>,
    pub rejection_event_refs: Vec<String>,
    pub expected_target_revisions: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub result: ReopenReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenRejectedOperationsRefusalReason {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    OperationNotRejected,
    UnavailableProof,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenRejectedOperationsConflictReason {
    ChangedHead,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReopenRejectedOperationsEffect {
    Resolved {
        author_action_sequence: String,
        undo_disposition: AuthorUndoDisposition,
        operation_ids: Vec<String>,
        rejection_event_refs: Vec<String>,
        prior_resolution: String,
        resulting_resolution: String,
        resulting_proposal_revision_id: String,
        resulting_validation: String,
        preserved_generation: String,
        preserved_closure: String,
        state_event_refs: Vec<String>,
    },
    Conflicted {
        reason: ReopenRejectedOperationsConflictReason,
    },
    Refused {
        reason: ReopenRejectedOperationsRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenRejectedOperationsResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: ReopenReceipt,
    pub project: ControlledProject,
    pub effect: ReopenRejectedOperationsEffect,
}

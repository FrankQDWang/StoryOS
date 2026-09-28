use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, QueryOperation};
use crate::release1_reject_proposal_operations::AuthorUndoDisposition;

pub const REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID: &str =
    "storyos.command.reopen-withdrawn-proposal.request.v1";
pub const REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.reopen-withdrawn-proposal.response.v1";
pub const REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE: &str =
    "storyos.command.reopenWithdrawnProposal.jcs.v1";

pub(super) const REOPEN_WITHDRAWN_PROPOSAL: QueryOperation = QueryOperation {
    operation_id: "reopenWithdrawnProposal",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
    request_schema: REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID,
    response_schema: REOPEN_WITHDRAWN_PROPOSAL_RESPONSE_SCHEMA_ID,
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
        "storyos.golden.reopenWithdrawnProposal.positive.v1",
        "storyos.golden.reopenWithdrawnProposal.invalid.v1",
        "storyos.golden.reopenWithdrawnProposal.boundary.v1",
    ],
};

pub const REOPEN_WITHDRAWN_PROPOSAL_PATH: &str = REOPEN_WITHDRAWN_PROPOSAL.path;
pub const REOPEN_WITHDRAWN_PROPOSAL_METHOD: &str = REOPEN_WITHDRAWN_PROPOSAL.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenWithdrawnProposalInput {
    pub proposal_revision_id: String,
    pub withdrawal_event_ref: String,
    pub expected_closure: String,
    pub expected_target_revisions: Vec<String>,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenWithdrawnProposalRequest {
    pub command_schema: String,
    pub reopen_withdrawn_proposal_input: ReopenWithdrawnProposalInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenWithdrawnReceiptResult {
    Resolved,
    Conflicted,
    Refused,
    NoEffect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenWithdrawnReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub author_command_admission_id: String,
    pub proposal_id: String,
    pub source_proposal_revision_id: String,
    pub resulting_proposal_revision_id: Option<String>,
    pub withdrawal_event_ref: String,
    pub expected_target_revisions: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub result: ReopenWithdrawnReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenWithdrawnProposalRefusalReason {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenWithdrawnProposalConflictReason {
    ChangedHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReopenWithdrawnProposalNoEffectReason {
    TerminalSupersession,
    ClosureNotWithdrawn,
    WithdrawalEventMismatch,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReopenWithdrawnProposalEffect {
    Resolved {
        author_action_sequence: String,
        undo_disposition: AuthorUndoDisposition,
        resulting_proposal_revision_id: String,
        prior_closure: String,
        resulting_closure: String,
        resulting_validation: String,
        preserved_generation: String,
        preserved_operation_resolution: String,
        withdrawal_event_ref: String,
        state_event_refs: Vec<String>,
    },
    Conflicted {
        reason: ReopenWithdrawnProposalConflictReason,
    },
    Refused {
        reason: ReopenWithdrawnProposalRefusalReason,
    },
    NoEffect {
        reason: ReopenWithdrawnProposalNoEffectReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReopenWithdrawnProposalResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: ReopenWithdrawnReceipt,
    pub project: ControlledProject,
    pub effect: ReopenWithdrawnProposalEffect,
}

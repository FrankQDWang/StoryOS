use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, QueryOperation};
use crate::release1_reject_proposal_operations::AuthorUndoDisposition;

pub const REPLAN_PROPOSAL_REQUEST_SCHEMA_ID: &str = "storyos.command.replan-proposal.request.v1";
pub const REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID: &str = "storyos.command.replan-proposal.response.v1";
pub const REPLAN_PROPOSAL_DIGEST_PROFILE: &str = "storyos.command.replanProposal.jcs.v1";

pub(super) const REPLAN_PROPOSAL: QueryOperation = QueryOperation {
    operation_id: "replanProposal",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
    request_schema: REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
    response_schema: REPLAN_PROPOSAL_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Replan settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Replan refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.replanProposal.positive.v1",
        "storyos.golden.replanProposal.invalid.v1",
        "storyos.golden.replanProposal.boundary.v1",
    ],
};

pub const REPLAN_PROPOSAL_PATH: &str = REPLAN_PROPOSAL.path;
pub const REPLAN_PROPOSAL_METHOD: &str = REPLAN_PROPOSAL.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplanSourceCondition {
    ProposalConflict {
        proposal_conflict_ref: String,
    },
    ProposalRecoveryConflict {
        proposal_recovery_conflict_ref: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplanProposalInput {
    pub conflicted_proposal_revision_id: String,
    pub expected_current_proposal_head: String,
    pub expected_current_target_revisions: Vec<String>,
    pub replacement_operations: Vec<String>,
    pub source_condition: ReplanSourceCondition,
    pub editor_session_id: String,
    pub client_contract_revision: String,
    pub security_policy_revision: String,
    pub correlation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplanProposalRequest {
    pub command_schema: String,
    pub replan_proposal_input: ReplanProposalInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReplanReceiptResult {
    Resolved,
    Conflicted,
    Refused,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplanReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    pub author_command_admission_id: String,
    pub proposal_id: String,
    pub source_proposal_revision_id: String,
    pub resulting_proposal_revision_id: String,
    pub expected_current_target_revisions: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub result: ReplanReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReplanProposalRefusalReason {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
    NotEligible,
    UnavailableProof,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReplanProposalConflictReason {
    ChangedHead,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplanProposalEffect {
    Resolved {
        author_action_sequence: String,
        undo_disposition: AuthorUndoDisposition,
        resulting_proposal_revision_id: String,
        resulting_validation: String,
        preserved_generation: String,
        preserved_closure: String,
        source_condition: ReplanSourceCondition,
        state_event_refs: Vec<String>,
    },
    Conflicted {
        reason: ReplanProposalConflictReason,
    },
    Refused {
        reason: ReplanProposalRefusalReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplanProposalResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    pub author_command_admission_id: String,
    pub receipt: ReplanReceipt,
    pub project: ControlledProject,
    pub effect: ReplanProposalEffect,
}

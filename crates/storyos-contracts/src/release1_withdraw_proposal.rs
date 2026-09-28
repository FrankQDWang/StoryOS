use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::release1::{ControlledProject, DigestValue, QueryOperation};
use crate::release1_reject_proposal_operations::{AuthorUndoDisposition, BoundedAuthorNote};

pub const WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID: &str =
    "storyos.command.withdraw-proposal.request.v1";
pub const WITHDRAW_PROPOSAL_RESPONSE_SCHEMA_ID: &str =
    "storyos.command.withdraw-proposal.response.v1";
pub const WITHDRAW_PROPOSAL_DIGEST_PROFILE: &str = "storyos.command.withdrawProposal.jcs.v1";

pub(super) const WITHDRAW_PROPOSAL: QueryOperation = QueryOperation {
    operation_id: "withdrawProposal",
    method: "POST",
    path: "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
    request_schema: WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID,
    response_schema: WITHDRAW_PROPOSAL_RESPONSE_SCHEMA_ID,
    responses: &[
        (200, "Withdrawal settled"),
        (400, "Invalid request"),
        (401, "Authentication required"),
        (403, "Request origin refused"),
        (404, "Resource unavailable"),
        (405, "Method not allowed"),
        (409, "Idempotency or Admission conflict"),
        (412, "Session or writer binding refused"),
        (413, "Request too large"),
        (415, "Unsupported content type"),
        (422, "Withdrawal refused"),
        (428, "Precondition required"),
        (429, "Rate limited"),
        (503, "Service unavailable"),
    ],
    fixtures: &[
        "storyos.golden.withdrawProposal.positive.v1",
        "storyos.golden.withdrawProposal.invalid.v1",
        "storyos.golden.withdrawProposal.boundary.v1",
    ],
};

pub const WITHDRAW_PROPOSAL_PATH: &str = WITHDRAW_PROPOSAL.path;
pub const WITHDRAW_PROPOSAL_METHOD: &str = WITHDRAW_PROPOSAL.method;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthorWithdrawalReason {
    AuthorWithdrew { note: BoundedAuthorNote },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CurrentProducerWithdrawalReason {
    CurrentProducerWithdrew,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposalWithdrawalReason {
    AuthorWithdrew { note: BoundedAuthorNote },
    CurrentProducerWithdrew,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunDecisionKind {
    AgentRunDecision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct AgentRunDecisionProducer {
    pub kind: AgentRunDecisionKind,
    pub run_id: String,
    pub decision_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "cause", rename_all = "snake_case", deny_unknown_fields)]
pub enum WithdrawProposalInput {
    Author {
        proposal_revision_id: String,
        expected_closure: String,
        expected_target_revisions: Vec<String>,
        withdrawal_reason: AuthorWithdrawalReason,
        editor_session_id: String,
        client_contract_revision: String,
        security_policy_revision: String,
        correlation_id: String,
    },
    CurrentProducer {
        producer: AgentRunDecisionProducer,
        proposal_revision_id: String,
        expected_closure: String,
        expected_target_revisions: Vec<String>,
        withdrawal_reason: CurrentProducerWithdrawalReason,
        client_contract_revision: String,
        security_policy_revision: String,
        correlation_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WithdrawProposalRequest {
    pub command_schema: String,
    pub withdraw_proposal_input: WithdrawProposalInput,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalReceiptResult {
    Resolved,
    Conflicted,
    Refused,
    NoEffect,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalReceipt {
    pub receipt_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_digest: DigestValue,
    pub idempotency_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_command_admission_id: Option<String>,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub expected_target_revisions: Vec<String>,
    pub prior_authoritative_revision_ids: Vec<String>,
    pub resulting_authoritative_revision_ids: Vec<String>,
    pub authoritative_commit_ids: Vec<String>,
    pub result: WithdrawalReceiptResult,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawProposalRefusalReason {
    WrongScope,
    WrongAdmission,
    StaleProposalRevision,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawProposalConflictReason {
    ChangedHead,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawProposalNoEffectReason {
    UnsupportedCause,
    TerminalSupersession,
    ClosureNotOpen,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WithdrawProposalEffect {
    Resolved {
        #[serde(skip_serializing_if = "Option::is_none")]
        author_action_sequence: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        undo_disposition: Option<AuthorUndoDisposition>,
        preserved_generation: String,
        preserved_validation: String,
        prior_closure: String,
        resulting_closure: String,
        withdrawal_reason: ProposalWithdrawalReason,
        closure_event_refs: Vec<String>,
    },
    Conflicted {
        reason: WithdrawProposalConflictReason,
    },
    Refused {
        reason: WithdrawProposalRefusalReason,
    },
    NoEffect {
        reason: WithdrawProposalNoEffectReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct WithdrawProposalResponse {
    pub schema_id: String,
    pub correlation_id: String,
    pub project_scope: crate::release1::ProjectScope,
    pub command_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_command_admission_id: Option<String>,
    pub receipt: WithdrawalReceipt,
    pub project: ControlledProject,
    pub effect: WithdrawProposalEffect,
}

use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplanProposalCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub conflicted_proposal_revision_id: String,
    pub expected_current_proposal_head: String,
    pub expected_authoritative_revision_id: String,
    pub replacement_operation_id: String,
    pub source_condition: storyos_contracts::ReplanSourceCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplanProposalSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: ReplanProposalSettlementEffect,
    pub receipt_created_at: String,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplanProposalSettlementEffect {
    Resolved {
        author_action_sequence: u64,
        resulting_proposal_revision_id: String,
        preserved_generation: String,
        preserved_closure: String,
        source_condition: storyos_contracts::ReplanSourceCondition,
        state_event_id: String,
    },
    Conflicted {
        reason: storyos_core::ReplanProposalConflict,
    },
    Refused {
        reason: storyos_core::ReplanProposalRefusal,
    },
}

#[derive(Debug)]
pub enum ReplanProposalError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ReplanProposalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Replan binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => {
                formatter.write_str("The original Replan acknowledgement cannot be recovered")
            }
            Self::InvalidChallenge => formatter.write_str("The Replan challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Replan store is unavailable"),
        }
    }
}

impl std::error::Error for ReplanProposalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict
            | Self::HistoricalAcknowledgementUnavailable
            | Self::InvalidChallenge
            | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Replan and its atomic Core settlement.
pub trait ReplanProposalStore: Sync {
    fn replan_proposal(
        &self,
        command: &ReplanProposalCommand,
    ) -> impl Future<Output = Result<ReplanProposalSettlement, ReplanProposalError>> + Send;
}

pub async fn replan_proposal(
    store: &impl ReplanProposalStore,
    command: &ReplanProposalCommand,
) -> Result<ReplanProposalSettlement, ReplanProposalError> {
    let challenge = &command.challenge_binding;
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&command.canonical_command_bytes)
            .iter()
            .fold(String::with_capacity(64), |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            });
        format!("sha256:storyos.command.replanProposal.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "replanProposal"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template
            != "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans"
        || challenge.command_schema != "storyos.command.replan-proposal.request.v1"
        || command.proposal_id.is_empty()
        || command.conflicted_proposal_revision_id.is_empty()
        || command.expected_current_proposal_head.is_empty()
        || command.expected_authoritative_revision_id.is_empty()
        || command.replacement_operation_id.is_empty()
    {
        return Err(ReplanProposalError::BindingConflict);
    }
    store.replan_proposal(command).await
}

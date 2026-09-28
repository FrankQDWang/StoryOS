use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawalNote {
    Omitted,
    Present { text: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawProposalCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub ids: AuthorCommandAdmissionIds,
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub expected_authoritative_revision_id: String,
    pub withdrawal_note: WithdrawalNote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawProposalSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: WithdrawProposalSettlementEffect,
    pub receipt_created_at: String,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawProposalSettlementEffect {
    Resolved {
        author_action_sequence: u64,
        withdrawal_note: WithdrawalNote,
        preserved_generation: String,
        preserved_validation: String,
        withdrawal_event_id: String,
    },
    Conflicted {
        reason: storyos_core::WithdrawProposalConflict,
    },
    Refused {
        reason: storyos_core::WithdrawProposalRefusal,
    },
    NoEffect {
        reason: storyos_core::WithdrawProposalNoEffect,
    },
}

#[derive(Debug)]
pub enum WithdrawProposalError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for WithdrawProposalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Withdrawal binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => {
                formatter.write_str("The original Withdrawal acknowledgement cannot be recovered")
            }
            Self::InvalidChallenge => formatter.write_str("The Withdrawal challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Withdrawal store is unavailable"),
        }
    }
}

impl std::error::Error for WithdrawProposalError {
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

/// Owns one admitted author Withdrawal and its atomic Core settlement.
pub trait WithdrawProposalStore: Sync {
    fn withdraw_proposal(
        &self,
        command: &WithdrawProposalCommand,
    ) -> impl Future<Output = Result<WithdrawProposalSettlement, WithdrawProposalError>> + Send;
}

pub async fn withdraw_proposal(
    store: &impl WithdrawProposalStore,
    command: &WithdrawProposalCommand,
) -> Result<WithdrawProposalSettlement, WithdrawProposalError> {
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
        format!(
            "sha256:{}:{value}",
            storyos_contracts::WITHDRAW_PROPOSAL_DIGEST_PROFILE
        )
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "withdrawProposal"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != storyos_contracts::WITHDRAW_PROPOSAL_METHOD
        || challenge.route_template != storyos_contracts::WITHDRAW_PROPOSAL_PATH
        || challenge.command_schema != storyos_contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID
        || command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(WithdrawProposalError::BindingConflict);
    }
    store.withdraw_proposal(command).await
}

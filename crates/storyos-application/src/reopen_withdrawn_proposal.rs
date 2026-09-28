use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenWithdrawnProposalCommand {
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
    pub withdrawal_event_id: String,
    pub expected_closure: String,
    pub expected_authoritative_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenWithdrawnProposalSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: ReopenWithdrawnProposalSettlementEffect,
    pub receipt_created_at: String,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenWithdrawnProposalSettlementEffect {
    Resolved {
        author_action_sequence: u64,
        resulting_proposal_revision_id: String,
        preserved_generation: String,
        preserved_operation_resolution: String,
    },
    Conflicted {
        reason: storyos_core::ReopenWithdrawnProposalConflict,
    },
    Refused {
        reason: storyos_core::ReopenWithdrawnProposalRefusal,
    },
    NoEffect {
        reason: storyos_core::ReopenWithdrawnProposalNoEffect,
    },
}

#[derive(Debug)]
pub enum ReopenWithdrawnProposalError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ReopenWithdrawnProposalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Reopen binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => {
                formatter.write_str("The original Reopen acknowledgement cannot be recovered")
            }
            Self::InvalidChallenge => formatter.write_str("The Reopen challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Reopen store is unavailable"),
        }
    }
}

impl std::error::Error for ReopenWithdrawnProposalError {
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

/// Settles one author reopen of a withdrawn Proposal.
pub trait ReopenWithdrawnProposalStore: Sync {
    fn reopen_withdrawn_proposal(
        &self,
        command: &ReopenWithdrawnProposalCommand,
    ) -> impl Future<
        Output = Result<ReopenWithdrawnProposalSettlement, ReopenWithdrawnProposalError>,
    > + Send;
}

pub async fn reopen_withdrawn_proposal(
    store: &impl ReopenWithdrawnProposalStore,
    command: &ReopenWithdrawnProposalCommand,
) -> Result<ReopenWithdrawnProposalSettlement, ReopenWithdrawnProposalError> {
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
            storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_DIGEST_PROFILE
        )
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "reopenWithdrawnProposal"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_METHOD
        || challenge.route_template != storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_PATH
        || challenge.command_schema
            != storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID
        || command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.withdrawal_event_id.is_empty()
        || command.expected_closure != "withdrawn"
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(ReopenWithdrawnProposalError::BindingConflict);
    }
    store.reopen_withdrawn_proposal(command).await
}

use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptProposalCommand {
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
    pub validation_receipt_id: String,
    pub selected_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptProposalSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: AcceptProposalSettlementEffect,
    pub receipt_created_at: String,
    pub condition_refs: Vec<String>,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptProposalSettlementEffect {
    Applied {
        author_action_sequence: u64,
        authoritative_commit_id: String,
        revision_id: String,
        body: String,
        blocks: Vec<crate::ManuscriptBlock>,
        project_activity_position: u64,
    },
    Invalid {
        reason: storyos_core::AcceptProposalInvalid,
    },
    Conflicted {
        reason: storyos_core::AcceptProposalConflict,
    },
    Refused {
        reason: storyos_core::AcceptProposalRefusal,
    },
}

#[derive(Debug)]
pub enum AcceptProposalError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    PreAdmissionRefused {
        reason: crate::AcceptanceRefusalReason,
    },
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for AcceptProposalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreAdmissionRefused { .. } => {
                formatter.write_str("Acceptance was refused before Admission")
            }
            Self::BindingConflict => formatter.write_str("The Acceptance binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => {
                formatter.write_str("The original Acceptance acknowledgement cannot be recovered")
            }
            Self::InvalidChallenge => formatter.write_str("The Acceptance challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Acceptance store is unavailable"),
        }
    }
}

impl std::error::Error for AcceptProposalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::PreAdmissionRefused { .. }
            | Self::BindingConflict
            | Self::HistoricalAcknowledgementUnavailable
            | Self::InvalidChallenge
            | Self::MissingProject => None,
        }
    }
}

/// Owns one admitted Acceptance and its atomic Core settlement.
pub trait AcceptProposalStore: Sync {
    fn accept_proposal(
        &self,
        command: &AcceptProposalCommand,
    ) -> impl Future<Output = Result<AcceptProposalSettlement, AcceptProposalError>> + Send;
}

pub async fn accept_proposal(
    store: &impl AcceptProposalStore,
    command: &AcceptProposalCommand,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
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
        format!("sha256:storyos.command.acceptProposal.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "acceptProposal"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template
            != "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances"
        || challenge.command_schema != "storyos.command.accept-proposal.request.v1"
        || command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.validation_receipt_id.is_empty()
        || command.selected_operation_ids.is_empty()
        || command.selected_operation_ids.iter().any(String::is_empty)
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(AcceptProposalError::BindingConflict);
    }
    store.accept_proposal(command).await
}

use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectionNote {
    Omitted,
    Present { text: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectProposalOperationsCommand {
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
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
    pub rejection_note: RejectionNote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectProposalOperationsSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: RejectProposalOperationsSettlementEffect,
    pub receipt_created_at: String,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectProposalOperationsSettlementEffect {
    Resolved {
        author_action_sequence: u64,
        operation_ids: Vec<String>,
        rejection_note: RejectionNote,
        preserved_generation: String,
        preserved_validation: String,
        preserved_closure: String,
        resolution_event_id: String,
    },
    Conflicted {
        reason: storyos_core::RejectProposalOperationsConflict,
    },
    Refused {
        reason: storyos_core::RejectProposalOperationsRefusal,
    },
}

#[derive(Debug)]
pub enum RejectProposalOperationsError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for RejectProposalOperationsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The Rejection binding conflicts"),
            Self::HistoricalAcknowledgementUnavailable => {
                formatter.write_str("The original Rejection acknowledgement cannot be recovered")
            }
            Self::InvalidChallenge => formatter.write_str("The Rejection challenge is invalid"),
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => formatter.write_str("The Rejection store is unavailable"),
        }
    }
}

impl std::error::Error for RejectProposalOperationsError {
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

/// Owns one admitted Rejection and its atomic Core settlement.
pub trait RejectProposalOperationsStore: Sync {
    fn reject_proposal_operations(
        &self,
        command: &RejectProposalOperationsCommand,
    ) -> impl Future<
        Output = Result<RejectProposalOperationsSettlement, RejectProposalOperationsError>,
    > + Send;
}

pub async fn reject_proposal_operations(
    store: &impl RejectProposalOperationsStore,
    command: &RejectProposalOperationsCommand,
) -> Result<RejectProposalOperationsSettlement, RejectProposalOperationsError> {
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
        format!("sha256:storyos.command.rejectProposalOperations.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "rejectProposalOperations"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template
            != "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections"
        || challenge.command_schema != "storyos.command.reject-proposal-operations.request.v1"
        || command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.selected_pending_operation_ids.is_empty()
        || command
            .selected_pending_operation_ids
            .iter()
            .any(String::is_empty)
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(RejectProposalOperationsError::BindingConflict);
    }
    store.reject_proposal_operations(command).await
}

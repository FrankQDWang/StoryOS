use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenRejectedOperationsCommand {
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
    pub selected_rejected_operation_id: String,
    pub rejection_event_id: String,
    pub expected_authoritative_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReopenRejectedOperationsSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: ReopenRejectedOperationsSettlementEffect,
    pub receipt_created_at: String,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReopenRejectedOperationsSettlementEffect {
    Resolved {
        author_action_sequence: u64,
        operation_id: String,
        rejection_event_id: String,
        resulting_proposal_revision_id: String,
        preserved_generation: String,
        preserved_closure: String,
        state_event_id: String,
    },
    Conflicted {
        reason: storyos_core::ReopenRejectedOperationsConflict,
    },
    Refused {
        reason: storyos_core::ReopenRejectedOperationsRefusal,
    },
}

#[derive(Debug)]
pub enum ReopenRejectedOperationsError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ReopenRejectedOperationsError {
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

impl std::error::Error for ReopenRejectedOperationsError {
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

/// Owns one admitted Reopen and its atomic Core settlement.
pub trait ReopenRejectedOperationsStore: Sync {
    fn reopen_rejected_operations(
        &self,
        command: &ReopenRejectedOperationsCommand,
    ) -> impl Future<
        Output = Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError>,
    > + Send;
}

pub async fn reopen_rejected_operations(
    store: &impl ReopenRejectedOperationsStore,
    command: &ReopenRejectedOperationsCommand,
) -> Result<ReopenRejectedOperationsSettlement, ReopenRejectedOperationsError> {
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
        format!("sha256:storyos.command.reopenRejectedOperations.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "reopenRejectedOperations"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template
            != "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings"
        || challenge.command_schema != "storyos.command.reopen-rejected-operations.request.v1"
        || command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.selected_rejected_operation_id.is_empty()
        || command.rejection_event_id.is_empty()
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(ReopenRejectedOperationsError::BindingConflict);
    }
    store.reopen_rejected_operations(command).await
}

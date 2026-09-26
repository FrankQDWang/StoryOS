use std::future::Future;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, EditorSessionId,
    ProjectCommandChallengeBinding, ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteReadyPartialProposalCommand {
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
    pub generation_id: String,
    pub expected_candidate_digest: String,
    pub last_applied_stream_seq: u64,
    pub expected_authoritative_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinueProposalGenerationCommand {
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
    pub prior_generation_id: String,
    pub expected_generation_state: String,
    pub expected_candidate_digest: String,
    pub selected_pending_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposalGenerationSettlement<T> {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: T,
    pub receipt_created_at: String,
    pub response_project: crate::Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompleteReadyPartialProposalEffect {
    Completed {
        author_action_sequence: u64,
        generation_id: String,
        preserved_validation: String,
        preserved_closure: String,
        preserved_operation_resolution: String,
        generation_event_id: String,
    },
    Conflicted {
        reason: storyos_core::ProposalGenerationConflict,
    },
    Refused {
        reason: storyos_core::CompleteReadyPartialProposalRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ContinueProposalGenerationEffect {
    Started {
        author_action_sequence: u64,
        prior_generation_id: String,
        new_generation_id: String,
        prior_generation_state: String,
        prior_run_id: String,
        resulting_run_id: String,
        preserved_validation: String,
        preserved_closure: String,
        preserved_operation_resolution: String,
        generation_event_id: String,
    },
    Conflicted {
        reason: storyos_core::ProposalGenerationConflict,
    },
    Refused {
        reason: storyos_core::ContinueProposalGenerationRefusal,
    },
}

#[derive(Debug)]
pub enum ProposalGenerationDecisionError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ProposalGenerationDecisionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => {
                formatter.write_str("The generation decision binding conflicts")
            }
            Self::HistoricalAcknowledgementUnavailable => formatter
                .write_str("The original generation decision acknowledgement cannot be recovered"),
            Self::InvalidChallenge => {
                formatter.write_str("The generation decision challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => {
                formatter.write_str("The generation decision store is unavailable")
            }
        }
    }
}

impl std::error::Error for ProposalGenerationDecisionError {
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

/// Owns one admitted generation completion or continuation.
pub trait ProposalGenerationDecisionStore: Sync {
    fn complete_ready_partial_proposal(
        &self,
        command: &CompleteReadyPartialProposalCommand,
    ) -> impl Future<
        Output = Result<
            ProposalGenerationSettlement<CompleteReadyPartialProposalEffect>,
            ProposalGenerationDecisionError,
        >,
    > + Send;

    fn continue_proposal_generation(
        &self,
        command: &ContinueProposalGenerationCommand,
    ) -> impl Future<
        Output = Result<
            ProposalGenerationSettlement<ContinueProposalGenerationEffect>,
            ProposalGenerationDecisionError,
        >,
    > + Send;
}

pub async fn complete_ready_partial_proposal(
    store: &impl ProposalGenerationDecisionStore,
    command: &CompleteReadyPartialProposalCommand,
) -> Result<
    ProposalGenerationSettlement<CompleteReadyPartialProposalEffect>,
    ProposalGenerationDecisionError,
> {
    bind_challenge(
        &command.project_scope,
        &command.client_binding,
        &command.challenge_binding,
        &command.canonical_command_bytes,
        "completeReadyPartialProposal",
        "storyos.command.completeReadyPartialProposal.jcs.v1",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-completions",
        "storyos.command.complete-ready-partial-proposal.request.v1",
    )?;
    if command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.generation_id.is_empty()
        || command.expected_candidate_digest.is_empty()
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(ProposalGenerationDecisionError::BindingConflict);
    }
    store.complete_ready_partial_proposal(command).await
}

pub async fn continue_proposal_generation(
    store: &impl ProposalGenerationDecisionStore,
    command: &ContinueProposalGenerationCommand,
) -> Result<
    ProposalGenerationSettlement<ContinueProposalGenerationEffect>,
    ProposalGenerationDecisionError,
> {
    bind_challenge(
        &command.project_scope,
        &command.client_binding,
        &command.challenge_binding,
        &command.canonical_command_bytes,
        "continueProposalGeneration",
        "storyos.command.continueProposalGeneration.jcs.v1",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
        "storyos.command.continue-proposal-generation.request.v1",
    )?;
    if command.proposal_id.is_empty()
        || command.proposal_revision_id.is_empty()
        || command.prior_generation_id.is_empty()
        || command.expected_generation_state.is_empty()
        || command.expected_candidate_digest.is_empty()
        || command.selected_pending_operation_ids.is_empty()
        || command.expected_authoritative_revision_id.is_empty()
    {
        return Err(ProposalGenerationDecisionError::BindingConflict);
    }
    store.continue_proposal_generation(command).await
}

#[allow(clippy::too_many_arguments)]
fn bind_challenge(
    project_scope: &ProjectScope,
    client_binding: &EditorClientBinding,
    challenge: &ProjectCommandChallengeBinding,
    canonical_command_bytes: &[u8],
    command_kind: &str,
    digest_profile: &str,
    route_template: &str,
    command_schema: &str,
) -> Result<(), ProposalGenerationDecisionError> {
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(canonical_command_bytes).iter().fold(
            String::with_capacity(64),
            |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            },
        );
        format!("sha256:{digest_profile}:{value}")
    };
    if challenge.project_scope != *project_scope
        || challenge.client_session_binding_digest != client_binding.binding_ref
        || challenge.client_session_generation != client_binding.session_generation
        || challenge.client_contract_revision != client_binding.client_contract_revision
        || challenge.security_policy_revision != client_binding.security_policy_revision
        || challenge.command_kind != command_kind
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "POST"
        || challenge.route_template != route_template
        || challenge.command_schema != command_schema
    {
        return Err(ProposalGenerationDecisionError::BindingConflict);
    }
    Ok(())
}

use std::future::Future;

use storyos_core::AssistanceAvailability;

use crate::{
    AuthorCommandAdmissionIds, EditorClientBinding, Project, ProjectCommandChallengeBinding,
    ProjectScope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAssistanceRecord {
    pub availability: AssistanceAvailability,
    pub revision: u64,
    pub model_registration_revision: String,
    pub processing_destination_identity: String,
    pub processing_destination_identity_evidence_revision: u64,
    pub project_model_use_binding_revision: String,
    pub external_compatibility_decision: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectAssistanceCommand {
    pub project_scope: ProjectScope,
    pub client_binding: EditorClientBinding,
    pub challenge_binding: ProjectCommandChallengeBinding,
    pub nonce_digest: String,
    pub canonical_command_bytes: Vec<u8>,
    pub correlation_id: String,
    pub availability: AssistanceAvailability,
    pub expected_revision: u64,
    pub ids: AuthorCommandAdmissionIds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectAssistanceSettlement {
    pub ids: AuthorCommandAdmissionIds,
    pub effect: UpdateProjectAssistanceSettlementEffect,
    pub receipt_created_at: String,
    pub project_activity_position: u64,
    pub project_activity_event_id: String,
    pub response_project: Project,
    pub assistance: Option<ProjectAssistanceRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectAssistanceSettlementEffect {
    Initialized {
        availability: AssistanceAvailability,
        revision: u64,
    },
    Applied {
        availability: AssistanceAvailability,
        revision: u64,
    },
    NoEffect {
        reason: storyos_core::UpdateProjectAssistanceNoEffect,
    },
    Conflicted {
        reason: storyos_core::UpdateProjectAssistanceConflict,
    },
    Refused {
        reason: storyos_core::UpdateProjectAssistanceRefusal,
    },
}

#[derive(Debug)]
pub enum UpdateProjectAssistanceError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for UpdateProjectAssistanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => {
                formatter.write_str("The Update Project Assistance binding conflicts")
            }
            Self::HistoricalAcknowledgementUnavailable => formatter.write_str(
                "The original Update Project Assistance acknowledgement cannot be recovered",
            ),
            Self::InvalidChallenge => {
                formatter.write_str("The Update Project Assistance challenge is invalid")
            }
            Self::MissingProject => formatter.write_str("The Project is not in exact Scope"),
            Self::Unavailable(_) => {
                formatter.write_str("The Update Project Assistance store is unavailable")
            }
        }
    }
}

impl std::error::Error for UpdateProjectAssistanceError {
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

/// Owns one admitted Update Project Assistance and its atomic Core settlement.
pub trait UpdateProjectAssistanceStore: Sync {
    fn update_project_assistance(
        &self,
        command: &UpdateProjectAssistanceCommand,
    ) -> impl Future<
        Output = Result<UpdateProjectAssistanceSettlement, UpdateProjectAssistanceError>,
    > + Send;
}

pub async fn update_project_assistance(
    store: &impl UpdateProjectAssistanceStore,
    command: &UpdateProjectAssistanceCommand,
) -> Result<UpdateProjectAssistanceSettlement, UpdateProjectAssistanceError> {
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
        format!("sha256:storyos.command.updateProjectAssistance.jcs.v1:{value}")
    };
    if challenge.project_scope != command.project_scope
        || challenge.client_session_binding_digest != command.client_binding.binding_ref
        || challenge.client_session_generation != command.client_binding.session_generation
        || challenge.client_contract_revision != command.client_binding.client_contract_revision
        || challenge.security_policy_revision != command.client_binding.security_policy_revision
        || challenge.command_kind != "updateProjectAssistance"
        || challenge.canonical_command_digest != command_digest
        || challenge.method != "PUT"
        || challenge.route_template != "/api/v1/projects/{project_id}/assistance"
        || challenge.command_schema != "storyos.command.update-project-assistance.request.v1"
    {
        return Err(UpdateProjectAssistanceError::BindingConflict);
    }
    store.update_project_assistance(command).await
}

#[cfg(test)]
#[path = "update_project_assistance_tests.rs"]
mod tests;

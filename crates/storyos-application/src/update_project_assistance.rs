//! The input, acknowledgement, and settlement of the Update Project Assistance command.

use std::convert::Infallible;

use storyos_core::{
    AssistanceAvailability, DeploymentDestination, DestinationKind, RuntimeQualification,
    UpdateProjectAssistanceApplied, UpdateProjectAssistanceConflict,
    UpdateProjectAssistanceNoEffect,
};

use crate::{ActivityApplied, Project, ProjectCommandSettlement};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAssistanceRecord {
    pub availability: AssistanceAvailability,
    pub revision: u64,
    pub model_registration_revision: String,
    pub processing_destination_identity: String,
    pub processing_destination_identity_evidence_revision: u64,
    pub project_model_use_binding_revision: String,
    pub grant_id: String,
    pub external_compatibility_decision: String,
    pub destination: DestinationKind,
    pub runtime_qualification: RuntimeQualification,
}

/// One author request to make Project assistance available or unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectAssistanceInput {
    pub availability: AssistanceAvailability,
    /// Zero when the Project has no assistance binding yet.
    pub expected_revision: u64,
    /// The destination that a first setting binds. The Host supplies it from its configuration.
    pub destination: DeploymentDestination,
}

/// The Command-response Project and the Project assistance record after the writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAssistanceAcknowledgement {
    pub project: Project,
    /// Absent while the Project has no assistance binding.
    pub assistance: Option<ProjectAssistanceRecord>,
}

pub type UpdateProjectAssistanceSettlement = ProjectCommandSettlement<
    ActivityApplied<UpdateProjectAssistanceApplied>,
    UpdateProjectAssistanceNoEffect,
    UpdateProjectAssistanceConflict,
    Infallible,
    ProjectAssistanceAcknowledgement,
>;

//! The input, record, and settlement of the Update Project Instruction command (ADR 0094 section 2).

use std::convert::Infallible;

use storyos_core::{
    UpdateProjectInstructionConflict, UpdateProjectInstructionNoEffect,
    UpdateProjectInstructionRefusal,
};

use crate::{ActivityApplied, ProjectCommandSettlement, RefusableCommandError};

/// One author save of the Project Instruction text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectInstructionInput {
    /// The exact text. An empty text means that no Project Instruction is configured.
    pub text: String,
    /// Zero when the Project has no Revision yet.
    pub expected_revision: u64,
}

/// One immutable Revision of the Project Instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectInstructionRevision {
    pub instruction_revision_id: String,
    pub revision: u64,
    pub text: String,
}

/// The new Revision of an applied save, without its text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectInstructionSaved {
    pub instruction_revision_id: String,
    pub revision: u64,
}

pub type UpdateProjectInstructionSettlement = ProjectCommandSettlement<
    ActivityApplied<ProjectInstructionSaved>,
    UpdateProjectInstructionNoEffect,
    UpdateProjectInstructionConflict,
    Infallible,
>;

pub type UpdateProjectInstructionError = RefusableCommandError<UpdateProjectInstructionRefusal>;

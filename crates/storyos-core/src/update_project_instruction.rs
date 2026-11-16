//! Pure Core classification for one save of the Project Instruction (ADR 0094 section 2).

use super::TransitionOutcome;
use crate::assemble_context::{CONTEXT_ITEM_TOKEN_LIMIT, count_context_item_tokens};
use crate::transition_outcome::reason_codes;

/// The facts of one save. A Project with no Revision has the revision 0 and the empty text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectInstruction {
    pub current_revision: u64,
    pub current_text: String,
    pub expected_revision: u64,
    pub text: String,
}

/// One new Revision with the next revision counter of the Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpdateProjectInstructionApplied {
    pub revision: u64,
}

pub type UpdateProjectInstructionResult = TransitionOutcome<
    UpdateProjectInstructionApplied,
    UpdateProjectInstructionNoEffect,
    UpdateProjectInstructionConflict,
    UpdateProjectInstructionRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectInstructionNoEffect {
    TextUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectInstructionConflict {
    StaleInstructionRevision,
}

/// A refusal that the adapter returns before Admission. No Receipt records it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateProjectInstructionRefusal {
    /// The text is above the Context item limit (ADR 0052). The Host never shortens it.
    InstructionOverItemLimit,
}

reason_codes!(UpdateProjectInstructionNoEffect { TextUnchanged => "text_unchanged" });
reason_codes!(UpdateProjectInstructionConflict {
    StaleInstructionRevision => "stale_instruction_revision",
});

/// Classify one save of the Project Instruction against the current Revision.
pub fn update_project_instruction(
    command: &UpdateProjectInstruction,
) -> UpdateProjectInstructionResult {
    if count_context_item_tokens(&command.text) > CONTEXT_ITEM_TOKEN_LIMIT {
        return TransitionOutcome::Refused(
            UpdateProjectInstructionRefusal::InstructionOverItemLimit,
        );
    }
    if command.expected_revision != command.current_revision {
        return TransitionOutcome::Conflicted(
            UpdateProjectInstructionConflict::StaleInstructionRevision,
        );
    }
    if command.text == command.current_text {
        return TransitionOutcome::NoEffect(UpdateProjectInstructionNoEffect::TextUnchanged);
    }
    TransitionOutcome::Applied(UpdateProjectInstructionApplied {
        revision: command.current_revision + 1,
    })
}

#[cfg(test)]
#[path = "update_project_instruction_tests.rs"]
mod tests;

use super::{
    TransitionOutcome, UpdateProjectInstruction, UpdateProjectInstructionApplied,
    UpdateProjectInstructionConflict, UpdateProjectInstructionNoEffect,
    UpdateProjectInstructionRefusal, update_project_instruction,
};
use crate::CONTEXT_ITEM_TOKEN_LIMIT;

fn save(
    current_revision: u64,
    current_text: &str,
    expected: u64,
    text: &str,
) -> UpdateProjectInstruction {
    UpdateProjectInstruction {
        current_revision,
        current_text: current_text.to_owned(),
        expected_revision: expected,
        text: text.to_owned(),
    }
}

#[test]
fn each_row_of_the_outcome_table_classifies_in_order() {
    let at_limit = "字".repeat(CONTEXT_ITEM_TOKEN_LIMIT as usize);
    let above_limit = "字".repeat(CONTEXT_ITEM_TOKEN_LIMIT as usize + 1);
    let cases = [
        // The limit goes first, also before a stale revision.
        (
            save(2, "old", 1, &above_limit),
            TransitionOutcome::Refused(UpdateProjectInstructionRefusal::InstructionOverItemLimit),
        ),
        (
            save(2, "old", 1, "new"),
            TransitionOutcome::Conflicted(
                UpdateProjectInstructionConflict::StaleInstructionRevision,
            ),
        ),
        (
            save(0, "", 1, "new"),
            TransitionOutcome::Conflicted(
                UpdateProjectInstructionConflict::StaleInstructionRevision,
            ),
        ),
        (
            save(2, "old", 2, "old"),
            TransitionOutcome::NoEffect(UpdateProjectInstructionNoEffect::TextUnchanged),
        ),
        // A Project with no Revision has the empty text.
        (
            save(0, "", 0, ""),
            TransitionOutcome::NoEffect(UpdateProjectInstructionNoEffect::TextUnchanged),
        ),
        (
            save(0, "", 0, &at_limit),
            TransitionOutcome::Applied(UpdateProjectInstructionApplied { revision: 1 }),
        ),
        (
            save(2, "old", 2, ""),
            TransitionOutcome::Applied(UpdateProjectInstructionApplied { revision: 3 }),
        ),
    ];
    for (command, expected) in cases {
        assert_eq!(update_project_instruction(&command), expected);
    }
}

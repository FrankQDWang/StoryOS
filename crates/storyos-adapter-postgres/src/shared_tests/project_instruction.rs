//! The shared test support from `project_instruction_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_project_session::UPDATE_PROJECT_INSTRUCTION;
use crate::shared_tests::command_sequence_support::issued;
use std::convert::Infallible;
use storyos_application::{
    ProjectInstructionSaved, ProjectScope, RefusableCommandError, UpdateProjectInstructionInput,
};
use storyos_core::{
    TransitionOutcome, UpdateProjectInstructionConflict, UpdateProjectInstructionNoEffect,
    UpdateProjectInstructionRefusal,
};

pub(crate) type Outcome = Result<
    TransitionOutcome<
        ProjectInstructionSaved,
        UpdateProjectInstructionNoEffect,
        UpdateProjectInstructionConflict,
        Infallible,
    >,
    UpdateProjectInstructionRefusal,
>;

/// Saves `text` in the Project of `scope` with the Command Challenge `suffix`.
pub(crate) async fn save(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    text: &str,
    expected_revision: u64,
) -> Outcome {
    let input = UpdateProjectInstructionInput {
        text: text.to_owned(),
        expected_revision,
    };
    let call = issued(store, scope, suffix, &UPDATE_PROJECT_INSTRUCTION, input).await;
    match store
        .update_project_instruction(&call.envelope, &call.input)
        .await
    {
        Ok(settlement) => Ok(settlement.outcome.map_applied(|applied| applied.effect)),
        Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => Err(refusal),
        Err(RefusableCommandError::Command(error)) => panic!("the save must settle: {error:?}"),
    }
}

pub(crate) fn applied(outcome: &Outcome) -> ProjectInstructionSaved {
    match outcome {
        Ok(TransitionOutcome::Applied(saved)) => saved.clone(),
        other => panic!("expected an applied save, got {other:?}"),
    }
}

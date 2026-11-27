//! The shared test support from `command_sequence_tests/project_session_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::{CommandCall, Route, issued, seed_project};
use storyos_application::{
    UpdateProjectAssistanceInput, UpdateProjectInput, UpdateProjectInstructionInput,
};
use storyos_contracts::CommandKind;
use storyos_core::AssistanceAvailability;

pub(crate) const UPDATE_PROJECT_ASSISTANCE: Route = Route {
    kind: CommandKind::UpdateProjectAssistance,
    method: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
};

pub(crate) const UPDATE_PROJECT_INSTRUCTION: Route = Route {
    kind: CommandKind::UpdateProjectInstruction,
    method: storyos_contracts::UPDATE_PROJECT_INSTRUCTION_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_INSTRUCTION_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_INSTRUCTION_REQUEST_SCHEMA_ID,
};

pub(crate) const UPDATE_PROJECT: Route = Route {
    kind: CommandKind::UpdateProject,
    method: storyos_contracts::UPDATE_PROJECT_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_REQUEST_SCHEMA_ID,
};

/// One applicable first Update Project Assistance in a new Project.
pub(crate) async fn update_project_assistance_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectAssistanceInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectAssistanceInput {
        availability: AssistanceAvailability::Available,
        expected_revision: 0,
        destination: Some(storyos_core::DeploymentDestination::HostFakeToolLoop),
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT_ASSISTANCE, input).await
}

/// One applicable first save of the Project Instruction in a new Project.
pub(crate) async fn update_project_instruction_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectInstructionInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectInstructionInput {
        text: "Write in the past tense.".to_owned(),
        expected_revision: 0,
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT_INSTRUCTION, input).await
}

/// One applicable Update Project in a new Project.
pub(crate) async fn update_project_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectInput {
        title: "Renamed".to_owned(),
        expected_revision: 1,
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT, input).await
}

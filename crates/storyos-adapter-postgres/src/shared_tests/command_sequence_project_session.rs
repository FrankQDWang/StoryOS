//! The shared test support from `command_sequence_tests/project_session_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, issued, seed_project, two_chapter_writer,
};
use crate::shared_tests::set_current_chapter_authority::open_session;
use storyos_application::EditorSessionId;
use storyos_application::{
    ProjectCommandError, UpdateProjectAssistanceInput, UpdateProjectInput,
    UpdateProjectInstructionInput,
};
use storyos_application::{TakeOverProjectWriterInput, TakeOverProjectWriterSettlement};
use storyos_contracts::CommandKind;
use storyos_core::AssistanceAvailability;

pub(crate) async fn take_over_project_writer(
    store: &PostgresProjectReader,
    call: &CommandCall<TakeOverProjectWriterInput>,
) -> Result<TakeOverProjectWriterSettlement, ProjectCommandError> {
    store
        .take_over_project_writer(&call.envelope, &call.input)
        .await
}

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

pub(crate) const TAKE_OVER_PROJECT_WRITER: Route = Route {
    kind: CommandKind::TakeOverProjectWriter,
    method: storyos_contracts::TAKE_OVER_PROJECT_WRITER_METHOD,
    path: storyos_contracts::TAKE_OVER_PROJECT_WRITER_PATH,
    schema: storyos_contracts::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
};

pub(crate) const UPDATE_PROJECT: Route = Route {
    kind: CommandKind::UpdateProject,
    method: storyos_contracts::UPDATE_PROJECT_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_REQUEST_SCHEMA_ID,
};

/// One applicable writer takeover by a second Editor Session in a new Project.
pub(crate) async fn take_over_project_writer_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<TakeOverProjectWriterInput> {
    let (scope, ..) = two_chapter_writer(store, base).await;
    let observer = open_session(store, &scope, &format!("{:04x}", base + 5)).await;
    let input = TakeOverProjectWriterInput {
        editor_session_id: EditorSessionId::new(observer),
        observed_writer_generation: 1,
        editor_contract_revision: storyos_contracts::EDITOR_CONTRACT_REVISION.to_owned(),
    };
    issued(store, &scope, base + 9, &TAKE_OVER_PROJECT_WRITER, input).await
}

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

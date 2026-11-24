//! The shared test support from `command_sequence_tests/steer_agent_run_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_agent_run::seed_run;
use crate::shared_tests::command_sequence_support::{
    CommandCall, Route, SequenceError, issued, seed_project,
};
use crate::statement_cache::Client;
use storyos_application::{ProjectCommandError, SteerAgentRunInput, SteerAgentRunSettlement};
use storyos_contracts::CommandKind;
use uuid::Uuid;

pub(crate) const STEER_AGENT_RUN: Route = Route {
    kind: CommandKind::SteerAgentRun,
    method: storyos_contracts::STEER_AGENT_RUN_METHOD,
    path: storyos_contracts::STEER_AGENT_RUN_PATH,
    schema: storyos_contracts::STEER_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(crate) async fn steer_agent_run(
    store: &PostgresProjectReader,
    call: &CommandCall<SteerAgentRunInput>,
) -> Result<SteerAgentRunSettlement, ProjectCommandError> {
    store
        .steer_agent_run(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

/// A steering input of `characters` characters for the seeded AgentRun `run_id`, in the
/// conversation of that Run.
pub(crate) async fn steering(
    admin: &Client,
    run_id: &str,
    characters: usize,
) -> SteerAgentRunInput {
    let conversation_id = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    SteerAgentRunInput {
        run_id: run_id.to_owned(),
        conversation_id,
        author_message: "a".repeat(characters),
        selection_references: None,
    }
}

/// One retained steerAgentRun of a waiting AgentRun in a new Project.
pub(crate) async fn steer_agent_run_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<SteerAgentRunInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let run_id = Uuid::now_v7().to_string();
    seed_run(admin, &scope, &run_id, "waiting").await;
    let input = steering(admin, &run_id, /*characters*/ 12).await;
    issued(store, &scope, base + 9, &STEER_AGENT_RUN, input).await
}

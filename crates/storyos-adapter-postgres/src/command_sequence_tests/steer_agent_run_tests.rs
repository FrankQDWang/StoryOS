use storyos_application::{
    AgentRunControlRefusal, ProjectCommandError, RefusableCommandError, SteerAgentRunInput,
    SteerAgentRunSettlement, SteeringRetained,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::update_volume_tests::seed_project;

use super::agent_run::{request_rows, seed_run};
use super::support::{CommandCall, Route, SequenceError, issued, stores};

pub(super) const STEER_AGENT_RUN: Route = Route {
    kind: "steerAgentRun",
    method: storyos_contracts::STEER_AGENT_RUN_METHOD,
    path: storyos_contracts::STEER_AGENT_RUN_PATH,
    schema: storyos_contracts::STEER_AGENT_RUN_REQUEST_SCHEMA_ID,
};

pub(super) async fn steer_agent_run(
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
pub(super) async fn steering(
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
    }
}

/// One retained steerAgentRun of a waiting AgentRun in a new Project.
pub(super) async fn steer_agent_run_call(
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_steering_refusal_before_admission_writes_no_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "c700").await;
    let run_id = Uuid::now_v7().to_string();
    seed_run(&admin, &scope, &run_id, "waiting").await;
    let first = issued(
        &store,
        &scope,
        /*suffix*/ 0xc701,
        &STEER_AGENT_RUN,
        steering(&admin, &run_id, /*characters*/ 8000).await,
    )
    .await;
    steer_agent_run(&store, &first).await.unwrap();
    // The Run message and the first input count 8 + 8000 + 1 characters, so 1990 more
    // characters reach the Context item bound.
    let within = steering(&admin, &run_id, /*characters*/ 1990).await;
    let refused = [
        SteerAgentRunInput {
            run_id: Uuid::now_v7().to_string(),
            ..within.clone()
        },
        SteerAgentRunInput {
            conversation_id: Uuid::now_v7().to_string(),
            ..within.clone()
        },
        SteerAgentRunInput {
            author_message: "a".repeat(/*n*/ 1991),
            ..within.clone()
        },
    ];
    let mut observed = Vec::new();
    for (suffix, input) in (0xc702..).zip(refused) {
        let call = issued(&store, &scope, suffix, &STEER_AGENT_RUN, input).await;
        let refusal = match store.steer_agent_run(&call.envelope, &call.input).await {
            Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => Some(refusal),
            Err(RefusableCommandError::Command(_)) | Ok(_) => None,
        };
        let key = &call.envelope.challenge_binding.idempotency_key;
        observed.push((refusal, request_rows(&admin, key).await));
    }
    let boundary = issued(
        &store,
        &scope,
        /*suffix*/ 0xc705,
        &STEER_AGENT_RUN,
        within,
    )
    .await;
    let settled = steer_agent_run(&store, &boundary).await.unwrap();
    let unused = [0, 0, 0, 1];
    assert_eq!(
        observed,
        vec![
            (Some(AgentRunControlRefusal::MissingRun), unused),
            (Some(AgentRunControlRefusal::MissingRun), unused),
            (Some(AgentRunControlRefusal::InputLimit), unused),
        ]
    );
    assert_eq!(
        settled.zero_authority_effect,
        Some(SteeringRetained {
            run_id,
            steering_input_id: boundary.envelope.ids.author_command_admission_id,
            input_position: 2,
        })
    );
}

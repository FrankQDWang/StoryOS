use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::{DeploymentDestination, RESEARCH_TOOL_OFFER, ResearchToolOffer};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};
use crate::agent_run_host_tool::tests::run_facts;

/// Claims and completes the queued Run `run_id` in a process with no Tool adapter.
async fn complete(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    run_id: &str,
) {
    let claim = claim_run(admin, scope.clone(), run_id.to_owned()).await;
    complete_agent_run(
        store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    )
    .await
    .unwrap();
}

/// Completes the first Run of a new Project of the tool loop profile, then completes one Run with
/// `author_message` in `conversation` (`None` for a new one). `prefixes` give the identities of
/// the two Runs.
async fn tool_loop_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    author_message: &str,
    research: Option<&'static ResearchToolOffer>,
    same_conversation: bool,
) -> (ProjectScope, String, String) {
    let destination = DeploymentDestination::HostFakeToolLoop;
    let (scope, chapter_id, first) =
        queued_run_offering(store, prefixes[0], destination.clone(), research).await;
    complete(store, admin, &scope, &first).await;
    let conversation = if same_conversation {
        ConversationSelection::Existing {
            conversation_id: format!("018f0000-0000-7001-8000-00000006{}6", prefixes[0]),
        }
    } else {
        ConversationSelection::New
    };
    let run_id = admit_run_in(
        store,
        &scope,
        &chapter_id,
        prefixes[1],
        destination,
        author_message,
        conversation,
        research,
    )
    .await;
    complete(store, admin, &scope, &run_id).await;
    (scope, first, run_id)
}

/// The decision kind and the `tool_choice` of each RunStep of the Run.
fn steps(facts: &Value) -> Vec<Value> {
    facts["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| json!([step["decision"], step["tool_choice"]]))
        .collect()
}

/// The response reference of each decision Model Attempt of the Run, and the prior response
/// reference that its request sent, in RunStep order.
async fn references(admin: &Client, run_id: &str) -> Vec<(Option<String>, Option<String>)> {
    admin
        .query(
            "SELECT payload ->> 'response_reference',
                    (payload #>> '{wire,serialized_payload}')::jsonb
                      ->> 'previous_response_reference'
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_of_the_conversation_continues_from_the_closing_response() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, first, next) =
        tool_loop_run(&store, &admin, ["1f1", "1f2"], "Help again.", None, true).await;

    let closing = references(&admin, &first).await[1].0.clone();
    assert!(closing.is_some());
    assert_eq!(references(&admin, &next).await[0].1, closing);
    let answer = store
        .read_agent_run(&scope, &first, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        answer.decision,
        AgentRunDecisionInspect::Advisory {
            continuation_binding_id: Some(_),
            ..
        }
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_text_before_a_finish_call_opens_the_next_run_step_from_that_text() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (_, _, run_id) = tool_loop_run(
        &store,
        &admin,
        ["1f3", "1f4"],
        "Search the web: SCRIPT:text_before_finish",
        Some(&RESEARCH_TOOL_OFFER),
        false,
    )
    .await;

    let facts = run_facts(&admin, &run_id).await;
    assert_eq!(
        (steps(&facts), &facts["status"]),
        (
            vec![
                json!(["tool_request", "required"]),
                json!([null, "required"]),
                json!(["tool_request", "required"]),
                json!(["advisory", "none"]),
            ],
            &json!("completed"),
        )
    );
    // The model continues from its own text, and the outputs of the first RunStep do not answer
    // that text again.
    let references = references(&admin, &run_id).await;
    assert_eq!(references[2].1, references[1].0);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_function_call_in_the_closing_response_makes_the_run_continue() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (_, _, run_id) = tool_loop_run(
        &store,
        &admin,
        ["1f5", "1f6"],
        "SCRIPT:finish_twice",
        None,
        false,
    )
    .await;

    let facts = run_facts(&admin, &run_id).await;
    assert_eq!(
        (steps(&facts), &facts["status"]),
        (
            vec![
                json!(["tool_request", "required"]),
                json!(["tool_request", "none"]),
                json!(["advisory", "none"]),
            ],
            &json!("completed"),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_final_answer_with_no_text_gives_the_run_no_result() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = tool_loop_run(
        &store,
        &admin,
        ["1f7", "1f8"],
        "SCRIPT:empty_final_answer",
        None,
        false,
    )
    .await;

    let facts = run_facts(&admin, &run_id).await;
    assert_eq!(
        (steps(&facts), &facts["status"]),
        (
            vec![json!(["tool_request", "required"]), json!([null, "none"]),],
            &json!("completed"),
        )
    );
    let record = store
        .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.decision, AgentRunDecisionInspect::Absent);
}

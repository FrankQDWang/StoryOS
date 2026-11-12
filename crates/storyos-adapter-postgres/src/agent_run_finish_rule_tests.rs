use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunDecisionInspect, AgentRunReadSelection, AgentRunReadStore, ConversationSelection,
    NoContractFaults, ProjectScope, complete_agent_run,
};
use storyos_core::{
    ADVISORY_TEXT, AgentRunStatus, DeploymentDestination, RESEARCH_TOOL_OFFER, ResearchToolOffer,
};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{
    RuntimeClock, admit_run_in, claim_run, queued_run_offering, stores,
};

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
/// `author_message` in a new Conversation or in the Conversation of the first Run. `prefixes`
/// give the identities of the two Runs.
async fn tool_loop_run(
    store: &PostgresProjectReader,
    admin: &Client,
    prefixes: [&str; 2],
    author_message: &str,
    research: Option<&'static ResearchToolOffer>,
    same_conversation: bool,
) -> (ProjectScope, String, String, String) {
    let destination = DeploymentDestination::HostFakeToolLoop;
    let (scope, chapter_id, first) =
        queued_run_offering(store, prefixes[0], destination.clone(), research).await;
    complete(store, admin, &scope, &first).await;
    let conversation = match same_conversation {
        true => conversation_of(prefixes[0]),
        false => ConversationSelection::New,
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
    (scope, chapter_id, first, run_id)
}

/// The Conversation of the first Run of the Project of `prefix`.
fn conversation_of(prefix: &str) -> ConversationSelection {
    ConversationSelection::Existing {
        conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
    }
}

/// For each decision Model Attempt of the Run in RunStep order: the decision kind, the
/// `tool_choice` of its request, its response reference, and the prior response reference that
/// its request sent.
async fn steps(admin: &Client, run_id: &str) -> Vec<Value> {
    admin
        .query(
            "SELECT jsonb_build_array(payload #>> '{decision,kind}', wire ->> 'tool_choice',
                                      payload ->> 'response_reference',
                                      wire ->> 'previous_response_reference')::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
              ORDER BY decision_position",
            &[&run_id],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str(row.get(0)).unwrap())
        .collect()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_run_ends_only_with_the_text_of_the_closing_response_after_a_finish_call() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (required, none) = ("required", "none");
    let cases = [
        (
            ["1f3", "1f4"],
            "Search the web: SCRIPT:text_before_finish",
            Some(&RESEARCH_TOOL_OFFER),
            json!([
                ["tool_request", required],
                [null, required],
                ["tool_request", required],
                ["advisory", none]
            ]),
            json!(ADVISORY_TEXT),
        ),
        (
            ["1f5", "1f6"],
            "SCRIPT:finish_twice",
            None,
            json!([
                ["tool_request", required],
                ["tool_request", none],
                ["advisory", none]
            ]),
            json!(ADVISORY_TEXT),
        ),
        (
            ["1f7", "1f8"],
            "SCRIPT:empty_final_answer",
            None,
            json!([["tool_request", required], [null, none]]),
            Value::Null,
        ),
        // A closing text with the shape of a reply form is also the final answer text.
        (
            ["1f9", "1fa"],
            "SCRIPT:reply_form_answer",
            None,
            json!([["tool_request", required], ["advisory", none]]),
            json!("Which chapter do you mean?"),
        ),
    ];
    for (prefixes, message, research, expected_steps, expected_answer) in cases {
        let (scope, _, _, run_id) =
            tool_loop_run(&store, &admin, prefixes, message, research, false).await;
        let steps = steps(&admin, &run_id).await;
        let record = store
            .read_agent_run(&scope, &run_id, &AgentRunReadSelection::Current)
            .await
            .unwrap()
            .unwrap();
        let answer = match record.decision {
            AgentRunDecisionInspect::Advisory { text, .. } => json!(text),
            AgentRunDecisionInspect::Absent => Value::Null,
            other => panic!("{message}: {other:?}"),
        };
        assert_eq!(
            (
                json!(
                    steps
                        .iter()
                        .map(|step| json!([step[0], step[1]]))
                        .collect::<Vec<_>>()
                ),
                record.status,
                answer,
            ),
            (expected_steps, AgentRunStatus::Completed, expected_answer),
            "{message}"
        );
        // Each RunStep continues from the response of the RunStep before it, also from a text.
        for pair in steps.windows(2) {
            assert_eq!((&pair[1][3], pair[0][2].is_string()), (&pair[0][2], true));
        }
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_of_the_conversation_continues_from_the_closing_response() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    // The second Run ends with a final answer with no text, and the third Run follows it.
    let message = "SCRIPT:empty_final_answer";
    let (scope, chapter_id, first, second) =
        tool_loop_run(&store, &admin, ["1f1", "1f2"], message, None, true).await;
    let record = store
        .read_agent_run(&scope, &first, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        record.decision,
        AgentRunDecisionInspect::Advisory {
            continuation_binding_id: Some(_),
            ..
        }
    ));
    let third = admit_run_in(
        &store,
        &scope,
        &chapter_id,
        "1fb",
        DeploymentDestination::HostFakeToolLoop,
        "Help again.",
        conversation_of("1f1"),
        None,
    )
    .await;
    complete(&store, &admin, &scope, &third).await;

    for (run, next) in [(&first, &second), (&second, &third)] {
        let closing = steps(&admin, run).await[1][2].clone();
        assert!(closing.is_string());
        assert_eq!(steps(&admin, next).await[0][3], closing);
    }
}

use serde_json::{Value, json};
use storyos_application::{AgentRunReadSelection, AgentRunReadStore, ProjectScope};
use storyos_core::{ADVISORY_TEXT, AgentRunStatus, ContextSourceClass, RESEARCH_TOOL_OFFER};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::{complete, conversation_of};
use crate::shared_tests::agent_run_steering::{
    AuthorAction, AuthorAt, Moment, cancellation, queued, settle, steering,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::Admission;

const FULL: &str = "Submit the complete current request. Repeat the first sentence.";

/// For each decision request of the Run in RunStep order: its position, whether it sent a prior
/// response reference, and the earlier turns that it sent.
async fn sent(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(decision_position,
                      wire ? 'previous_response_reference', wire->'earlier_turns')
                      ORDER BY decision_position, model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
        &[&run_id],
    )
    .await
}

/// For each decision Context Assembly of the Run in RunStep order: its position and the Run and
/// the kind of each earlier-turn source that its Operation Requirement records.
async fn recorded(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(decision_position,
                      (SELECT jsonb_agg(jsonb_build_array(source->>'run_id',
                                                          source->>'message_kind'))
                         FROM jsonb_array_elements(
                                payload #> '{operation_requirement,earlier_turns,sources}')
                              AS source))
                      ORDER BY decision_position)::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'",
        &[&run_id],
    )
    .await
}

/// The earlier turns of the fake wire: each turn as its messages of kind and text.
fn turns(turns: &[&[(&str, &str)]]) -> Value {
    json!({"older_turns_left_out": false, "turns": turns})
}

/// Admits and completes one Run with `message` in the Conversation of the Project of `first`.
async fn next_run(
    store: &PostgresProjectReader,
    admin: &Client,
    (scope, chapter_id): (&ProjectScope, &str),
    first: &str,
    message: &str,
    prefix: &str,
) -> String {
    let run_id = Admission {
        message,
        conversation: conversation_of(first),
        ..Default::default()
    }
    .admit(store, scope, chapter_id, prefix)
    .await;
    complete(store, admin, scope, &run_id).await;
    run_id
}

async fn status(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    run_id: &str,
) -> AgentRunStatus {
    (store.read_agent_run(scope, run_id, &AgentRunReadSelection::Current))
        .await
        .unwrap()
        .unwrap()
        .status
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_full_input_with_no_prior_response_sends_the_earlier_turn_in_exact_words() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The first Run reads a steering input at RunStep 1 and completes with a final answer.
    let (scope, chapter_id, first) = queued(&store, "3e1").await;
    let input = steering(&store, &scope, 0x3ec0, &first, "Keep the voice.", None).await;
    let author = AuthorAt::new(
        &store,
        Moment::DuringRequest(1),
        AuthorAction::Steer(vec![input]),
    );
    settle(&store, &admin, &scope, &first, &author)
        .await
        .unwrap();
    let second = next_run(&store, &admin, (&scope, &chapter_id), "3e1", FULL, "3e2").await;
    let read = store
        .read_agent_run(&scope, &second, &AgentRunReadSelection::Current)
        .await
        .unwrap()
        .unwrap();
    let answer = (read.context.record.selected.iter())
        .filter(|item| item.source_class == ContextSourceClass::EarlierAnswer)
        .map(|item| item.content.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        (read.status, answer, sent(&admin, &second).await),
        (
            AgentRunStatus::Completed,
            vec![ADVISORY_TEXT.to_owned()],
            // The first request sends no prior response reference and the earlier turn. The
            // RunStep after the `finish` call continues its response with no earlier turn.
            json!([
                [
                    0,
                    false,
                    turns(&[&[
                        ("author", "Help with this passage."),
                        ("author", "Keep the voice."),
                        ("answer", ADVISORY_TEXT),
                    ]])
                ],
                [1, true, null],
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_continuing_request_sends_no_earlier_turn_and_a_refused_or_cancelled_run_gives_none() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, first) = queued(&store, "3e3").await;
    complete(&store, &admin, &scope, &first).await;
    let project = (&scope, chapter_id.as_str());
    let refused = next_run(&store, &admin, project, "3e3", "SCRIPT:refusal", "3e4").await;
    let cancelled = Admission {
        message: "Name the inn.",
        conversation: conversation_of("3e3"),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "3e5")
    .await;
    let cancel = cancellation(&store, &scope, 0x3ed0, &cancelled).await;
    store
        .cancel_agent_run(&cancel.envelope, &cancel.input)
        .await
        .unwrap();
    let ordinary = next_run(&store, &admin, project, "3e3", "Make Mara older.", "3e6").await;
    let full = next_run(&store, &admin, project, "3e3", FULL, "3e7").await;

    assert_eq!(
        (
            [
                status(&store, &scope, &refused).await,
                status(&store, &scope, &cancelled).await,
            ],
            sent(&admin, &ordinary).await,
            sent(&admin, &full).await,
            recorded(&admin, &full).await,
        ),
        (
            [AgentRunStatus::Refused, AgentRunStatus::Cancelled],
            json!([[0, true, null], [1, true, null]]),
            json!([
                [
                    0,
                    false,
                    turns(&[
                        &[
                            ("author", "Help with this passage."),
                            ("answer", ADVISORY_TEXT)
                        ],
                        &[("author", "Make Mara older."), ("answer", ADVISORY_TEXT)],
                    ])
                ],
                [1, true, null],
            ]),
            // Only the two completed Runs give a turn.
            json!([
                [
                    0,
                    [
                        [first, "author"],
                        [first, "answer"],
                        [ordinary, "author"],
                        [ordinary, "answer"]
                    ]
                ],
                [
                    1,
                    [
                        [first, "author"],
                        [first, "answer"],
                        [ordinary, "author"],
                        [ordinary, "answer"]
                    ]
                ],
            ]),
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_new_transport_and_a_rebuilt_full_input_send_the_earlier_turns() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    // The destination loses the responses of the first Run, so the next Run rebuilds a full
    // input after the rejection of its incremental request.
    let lost = "Help with this passage. SCRIPT:reference-unusable";
    let (scope, chapter_id, first) = queued(&store, "3e8").await;
    complete(&store, &admin, &scope, &first).await;
    let project = (&scope, chapter_id.as_str());
    next_run(&store, &admin, project, "3e8", lost, "3e9").await;
    let after_loss = next_run(&store, &admin, project, "3e8", "Make Mara older.", "3eb").await;
    // The deployment starts to offer the research destination, so the Tool Exposure changes.
    let exposed = Admission {
        message: "Check the tide tables of 1890.",
        conversation: conversation_of("3e8"),
        research: Some(&RESEARCH_TOOL_OFFER),
        ..Default::default()
    }
    .admit(&store, &scope, &chapter_id, "3ec")
    .await;
    complete(&store, &admin, &scope, &exposed).await;
    let earlier = |count: usize| {
        let messages: [&[(&str, &str)]; 3] = [
            &[
                ("author", "Help with this passage."),
                ("answer", ADVISORY_TEXT),
            ],
            &[("author", lost), ("answer", ADVISORY_TEXT)],
            &[("author", "Make Mara older."), ("answer", ADVISORY_TEXT)],
        ];
        turns(&messages[..count])
    };

    assert_eq!(
        (
            sent(&admin, &after_loss).await,
            sent(&admin, &exposed).await
        ),
        (
            // The incremental request is rejected with `continuation_unusable`, and the full
            // input that follows it sends the earlier turns.
            json!([[0, true, null], [0, false, earlier(2)], [1, true, null]]),
            // The changed Tool Exposure sends a new transport with the earlier turns.
            json!([[0, false, earlier(3)], [1, true, null]]),
        )
    );
}

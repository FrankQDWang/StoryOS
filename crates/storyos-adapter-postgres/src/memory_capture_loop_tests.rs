//! The loop of a capture (ADR 0094 sections 5 and 8): each request has only the output Tool and
//! requires a call, an unexposed call and a refused list do not end it, and the settled call
//! stores the observations, settles the capture, and completes the Run.

use serde_json::{Value, json};
use storyos_application::{ConversationSelection, MemoryCaptureSource, ProjectScope};
use storyos_core::{DeploymentDestination, ObservationRefusal, RESEARCH_TOOL_OFFER};
use tokio_postgres::Client;

use crate::shared_tests::agent_run_finish_rule::{calls, complete};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::create_agent_run_skill::{fixture, set};
use crate::shared_tests::memory_capture::{park_queued_author_runs, project, run_capture, start};
use crate::shared_tests::memory_capture_loop::{
    other_records, record, requests, script, settlement,
};
use crate::shared_tests::model_gateway::{admit_selected_run, queued_run_offering};

/// The Tool Exposure of each RunStep of a capture (ADR 0094 section 5).
pub(crate) const CAPTURE_TOOLS: [&str; 4] = [
    "read_chapter",
    "search_manuscript",
    "read_memory_entry",
    "record_memory_observations",
];

fn refused(position: u64, refusal: ObservationRefusal) -> Value {
    let (label, text) = (refusal.label(), refusal.result().text);
    json!([
        position,
        "record_memory_observations",
        "refused",
        label,
        text
    ])
}

/// A Project whose first Run of the author has a research binding, and whose second Run pins a
/// Skill catalog of the deployment, and the source of the second Run.
async fn bound_project(
    store: &crate::PostgresProjectReader,
    admin: &Client,
) -> (ProjectScope, MemoryCaptureSource) {
    park_queued_author_runs(admin).await;
    let destination = DeploymentDestination::HostFakeToolLoop;
    let research = Some(&RESEARCH_TOOL_OFFER);
    let (scope, chapter_id, first) = queued_run_offering(store, "fd8", destination, research).await;
    complete(store, admin, &scope, &first).await;
    let new = ConversationSelection::New;
    let skills = (set(vec![fixture("dialogue-style-guide")]), Vec::new());
    let second = admit_selected_run(store, &scope, &chapter_id, "fd9", "Help.", new, skills).await;
    complete(store, admin, &scope, &second).await;
    let conversation_id: String = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&second],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let (first_run_id, last_run_id) = (second.clone(), second);
    let source = MemoryCaptureSource::Conversation {
        conversation_id,
        first_run_id,
        last_run_id,
    };
    (scope, source)
}

/// In a Project with a research binding and a Skill, each request of a capture has only the output
/// Tool and requires a call. An unexposed call and a refused list do not end the capture: the next
/// RunStep corrects the list, the settled call stores each observation in list order and
/// completes the Run, and the capture writes no Memory entry, no Proposal, no Research Artifact,
/// and no manuscript change.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_records_its_observations_after_refused_calls_and_writes_no_other_record() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, source) = bound_project(&store, &admin).await;
    let before = other_records(&admin, &scope).await;
    let item = |kind, title| json!({"type": kind, "title": title, "text": "雨夜。"});
    let kinds = ["character", "other", "other", "major_event", "relationship"];
    let five: Vec<Value> = kinds.iter().map(|kind| item(*kind, "林舟")).collect();
    let input = script(&[
        ("propose_edit", json!({"reason": "改写"})),
        record(json!(vec![item("other", "林舟"); 6])),
        record(json!([item("other", "林舟"), item("place", "港口")])),
        record(json!(five)),
    ]);
    let started = start(&store, &scope, &source, &input).await.unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    let calls = calls(&admin, &started.run_id).await;
    assert_eq!(
        (calls[0][1].clone(), calls[0][2].clone()),
        (json!("propose_edit"), json!("refused"))
    );
    let settled = "The Host recorded 5 observations, and the capture is complete.";
    assert_eq!(
        calls[1..],
        [
            refused(1, ObservationRefusal::TooMany),
            refused(2, ObservationRefusal::Invalid { position: 2 }),
            json!([3, "record_memory_observations", "settled", null, settled]),
        ]
    );
    let observation = |position, kind| json!([position, kind, "林舟", "雨夜。", "waiting"]);
    let observations: Vec<Value> = (1..).zip(kinds).map(|(p, k)| observation(p, k)).collect();
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!([
            "completed",
            "settled",
            "observations_recorded",
            null,
            observations
        ])
    );
    let request = json!(["memory_capture", "required", CAPTURE_TOOLS]);
    assert_eq!(
        requests(&admin, &started.run_id).await,
        json!(vec![request; 4])
    );
    assert_eq!(other_records(&admin, &scope).await, before);
}

/// U+0000, an empty title, and a long text refuse the call with their labels, and an empty list
/// settles the capture with no observation.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_empty_list_settles_the_capture_with_no_observation_after_each_refusal() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "fda").await;
    let long = "字".repeat(storyos_core::MEMORY_TEXT_MAX_CHARS + 1);
    let input = script(&[
        record(json!([{"type": "other", "title": "雨", "text": "a\u{0}"}])),
        record(json!([{"type": "other", "title": " ", "text": "雨。"}])),
        record(json!([{"type": "other", "title": "雨", "text": long}])),
    ]);
    let started = start(&store, &scope, &source, &input).await.unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    let labels: Vec<Value> = calls(&admin, &started.run_id)
        .await
        .iter()
        .map(|call| json!([call[2], call[3]]))
        .collect();
    assert_eq!(
        labels,
        [
            json!(["refused", "unstorable_text"]),
            json!(["refused", "observation_invalid"]),
            json!(["refused", "observation_too_long"]),
            json!(["settled", null]),
        ]
    );
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!(["completed", "settled", "no_observation", null, []])
    );
}

/// The step limit of a capture bounds its loop: a script that never calls the output Tool gets
/// one request for each RunStep, each request requires a call, and after the last RunStep the
/// capture settles with `step_limit`, it stores no observation, and the Run completes.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_that_never_records_settles_with_the_step_limit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "fdb").await;
    let max = usize::try_from(storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS).unwrap();
    let edits = vec![("propose_edit", json!({"reason": "改写"})); max + 2];
    let started = start(&store, &scope, &source, &script(&edits))
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    let states: Vec<Value> = calls(&admin, &started.run_id)
        .await
        .iter()
        .map(|call| json!([call[1], call[2]]))
        .collect();
    assert_eq!(states, vec![json!(["propose_edit", "refused"]); max]);
    let request = json!(["memory_capture", "required", CAPTURE_TOOLS]);
    assert_eq!(
        requests(&admin, &started.run_id).await,
        json!(vec![request; max])
    );
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!(["completed", "settled", "step_limit", null, []])
    );
}

/// The Host executes the output call of the last RunStep. When that call is refused, the capture
/// settles with `step_limit` and stores no observation.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_output_call_in_the_last_run_step_settles_with_the_step_limit() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "fdc").await;
    let max = usize::try_from(storyos_core::MEMORY_CAPTURE_MAX_RUN_STEPS).unwrap();
    let mut lines = vec![("propose_edit", json!({"reason": "改写"})); max - 1];
    let item = json!({"type": "other", "title": "雨", "text": "雨。"});
    lines.push(record(json!(vec![item; 6])));
    let started = start(&store, &scope, &source, &script(&lines))
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    let calls = calls(&admin, &started.run_id).await;
    assert_eq!(
        (calls.len(), calls[max - 1].clone()),
        (
            max,
            refused(u64::try_from(max - 1).unwrap(), ObservationRefusal::TooMany)
        )
    );
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!(["completed", "settled", "step_limit", null, []])
    );
}

/// Each request of a capture has the kind of the input of the capture row: a Conversation or a
/// Chapter. The adapter gives each kind its own section (ADR 0094 section 5).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn each_request_of_a_capture_has_the_kind_of_its_input() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, conversation) = project(&store, &admin, "590").await;
    let revision_id: String = admin
        .query_one(
            "SELECT revision_id::text FROM storyos.authoritative_revisions
              WHERE manuscript_object_id = $1::text::uuid LIMIT 1",
            &[&chapter_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let chapter = MemoryCaptureSource::Chapter {
        chapter_id,
        revision_id,
    };
    let mut kinds = Vec::new();
    for source in [&conversation, &chapter] {
        let started = start(&store, &scope, source, "雨。").await.unwrap();
        run_capture(&store, &admin, &scope, &started.run_id).await;
        let row = admin
            .query_one(
                "SELECT jsonb_agg(wire ->> 'capture_input')::text
                   FROM storyos.model_attempts,
                        LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
                  WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
                &[&started.run_id],
            )
            .await
            .unwrap();
        kinds.push(serde_json::from_str::<Value>(row.get(/*idx*/ 0)).unwrap());
    }

    assert_eq!(kinds, [json!(["conversation"]), json!(["chapter"])]);
}

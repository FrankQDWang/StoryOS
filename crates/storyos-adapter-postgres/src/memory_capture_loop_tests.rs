//! The loop of a capture (ADR 0094 sections 5 and 8): each request has only the output Tool and
//! requires a call, an unexposed call and a refused list do not end it, and the settled call
//! stores the observations, settles the capture, and completes the Run.

use serde_json::{Value, json};
use storyos_application::{ConversationSelection, MemoryCaptureSource, ProjectScope};
use storyos_core::{DeploymentDestination, ObservationRefusal, RESEARCH_TOOL_OFFER};
use tokio_postgres::Client;

use super::tests::{project, run_capture, start};
use crate::agent_run_dispatch::tests::{admit_selected_run, queued_run_offering, stores};
use crate::agent_run_finish_rule::tests::{calls, complete};
use crate::create_agent_run_tests::skill_tests::{fixture, set};

/// The input of a capture: one script line for each call, in RunStep order.
pub(crate) fn script(calls: &[(&str, Value)]) -> String {
    let lines: Vec<String> = calls
        .iter()
        .map(|(name, arguments)| format!("SCRIPT:capture {name} {arguments}"))
        .collect();
    format!("林舟在雨夜撑船。\n{}", lines.join("\n"))
}

pub(crate) fn record(observations: Value) -> (&'static str, Value) {
    let arguments = json!({ "observations": observations });
    ("record_memory_observations", arguments)
}

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

/// The Run status, the capture state, outcome, and failure reason, and each observation of the
/// Run `run_id`.
pub(crate) async fn settlement(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(run.status, capture.state, capture.outcome,
                      capture.failure_reason,
                      COALESCE((SELECT jsonb_agg(jsonb_build_array(item.position, item.entry_type,
                                  item.title, item.text, item.state) ORDER BY item.position)
                                  FROM storyos.memory_observations AS item
                                 WHERE item.capture_id = capture.capture_id), '[]'))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.memory_captures AS capture USING (owner_user_id, project_id, run_id)
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// The count of the Memory entry versions, the Proposals, the Research Artifacts, and the
/// Authoritative Revisions of the Project of `scope`.
pub(crate) async fn other_records(admin: &Client, scope: &ProjectScope) -> Value {
    let tables = [
        "memory_entry_versions",
        "proposals",
        "research_artifacts",
        "authoritative_revisions",
    ];
    let mut counts = Vec::new();
    for table in tables {
        let query = format!(
            "SELECT count(*) FROM storyos.{table}
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid"
        );
        let values = [scope.owner_user_id.as_ref(), scope.project_id.as_ref()];
        let row = admin.query_one(&query, &[&values[0], &values[1]]).await;
        counts.push(row.unwrap().get::<_, i64>(/*idx*/ 0));
    }
    json!(counts)
}

/// For each decision request of the Run: its purpose, its `tool_choice`, and its Tools.
pub(crate) async fn requests(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(wire ->> 'purpose', wire ->> 'tool_choice',
                      wire -> 'tools') ORDER BY decision_position)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// A Project whose first Run of the author has a research binding, and whose second Run pins a
/// Skill catalog of the deployment, and the source of the second Run.
async fn bound_project(
    store: &crate::PostgresProjectReader,
    admin: &Client,
) -> (ProjectScope, MemoryCaptureSource) {
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let request = json!(["memory_capture", "required", ["record_memory_observations"]]);
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
    let request = json!(["memory_capture", "required", ["record_memory_observations"]]);
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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

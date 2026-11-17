//! A capture in the store (ADR 0094 sections 5 and 8): the start and its refusals, the Worker
//! loop with only the output Tool, the settled call and its observations, the runtime role, the
//! Project Scope, and the Project Export Archive.

use serde_json::{Value, json};
use storyos_application::{
    MemoryCaptureSource, MemoryCaptureStartRefusal, MemoryCaptureStore, ProjectScope,
    StartedMemoryCapture,
};
use storyos_core::{CaptureRefusal, ObservationRefusal};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_finish_rule::tests::{calls, complete};
use crate::agent_run_steering::tests::queued;
use crate::command_sequence::tests::seed_project;
use crate::memory_entries::tests::{archived_rows, as_runtime};

/// A Project of `prefix` with one completed author Run, and the source of that Run.
async fn project(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (ProjectScope, MemoryCaptureSource) {
    let (scope, _chapter_id, run_id) = queued(store, prefix).await;
    complete(store, admin, &scope, &run_id).await;
    let conversation_id: String = admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let source = MemoryCaptureSource::Conversation {
        conversation_id,
        first_run_id: run_id.clone(),
        last_run_id: run_id,
    };
    (scope, source)
}

async fn start(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    source: &MemoryCaptureSource,
    input: &str,
) -> Result<StartedMemoryCapture, MemoryCaptureStartRefusal> {
    store
        .start_memory_capture(scope, source, input)
        .await
        .unwrap()
}

/// The input of a capture: one script line for each call, in RunStep order.
fn script(calls: &[(&str, Value)]) -> String {
    let lines: Vec<String> = calls
        .iter()
        .map(|(name, arguments)| format!("SCRIPT:capture {name} {arguments}"))
        .collect();
    format!("林舟在雨夜撑船。\n{}", lines.join("\n"))
}

fn record(observations: Value) -> (&'static str, Value) {
    (
        "record_memory_observations",
        json!({ "observations": observations }),
    )
}

fn refused(position: u64, label: &str, text: &str) -> Value {
    json!([
        position,
        "record_memory_observations",
        "refused",
        label,
        text
    ])
}

/// The Run status, the capture state and outcome, and each observation of the Run `run_id`.
async fn settlement(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(run.status, capture.state, capture.outcome,
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
async fn other_records(admin: &Client, scope: &ProjectScope) -> Value {
    let tables = [
        "memory_entry_versions",
        "proposals",
        "research_artifacts",
        "authoritative_revisions",
    ];
    let mut counts = Vec::new();
    for table in tables {
        let row = admin
            .query_one(
                &format!(
                    "SELECT count(*) FROM storyos.{table}
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid"
                ),
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .unwrap();
        counts.push(row.get::<_, i64>(/*idx*/ 0));
    }
    json!(counts)
}

/// For each decision request of the Run: its purpose, its `tool_choice`, and the names of its
/// Tools.
async fn requests(admin: &Client, run_id: &str) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(wire ->> 'purpose', wire ->> 'tool_choice',
                      wire -> 'tools')
                      ORDER BY decision_position)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'",
            &[&run_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// An unexposed call and a refused list do not end the capture: the next RunStep corrects the
/// list, the settled call stores each observation in list order and completes the Run, and the
/// capture writes no Memory entry, no Proposal, no Research Artifact, and no manuscript change.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_capture_records_its_observations_after_refused_calls_and_writes_no_other_record() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, source) = project(&store, &admin, "fd1").await;
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
    complete(&store, &admin, &scope, &started.run_id).await;

    let calls = calls(&admin, &started.run_id).await;
    assert_eq!(
        (calls[0][1].clone(), calls[0][2].clone()),
        (json!("propose_edit"), json!("refused"))
    );
    let too_many = ObservationRefusal::TooMany;
    let invalid = ObservationRefusal::Invalid { position: 2 };
    assert_eq!(
        calls[1..],
        [
            refused(1, too_many.label(), &too_many.result().text),
            refused(2, invalid.label(), &invalid.result().text),
            json!([
                3,
                "record_memory_observations",
                "settled",
                null,
                "The Host recorded 5 observations, and the capture is complete."
            ]),
        ]
    );
    let observation = |position, kind| json!([position, kind, "林舟", "雨夜。", "waiting"]);
    assert_eq!(
        settlement(&admin, &started.run_id).await,
        json!([
            "completed",
            "settled",
            "observations_recorded",
            [
                observation(1, "character"),
                observation(2, "other"),
                observation(3, "other"),
                observation(4, "major_event"),
                observation(5, "relationship"),
            ]
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
    let (scope, source) = project(&store, &admin, "fd2").await;
    let long = "字".repeat(storyos_core::MEMORY_TEXT_MAX_CHARS + 1);
    let input = script(&[
        record(json!([{"type": "other", "title": "雨", "text": "a\u{0}"}])),
        record(json!([{"type": "other", "title": " ", "text": "雨。"}])),
        record(json!([{"type": "other", "title": "雨", "text": long}])),
    ]);
    let started = start(&store, &scope, &source, &input).await.unwrap();
    complete(&store, &admin, &scope, &started.run_id).await;

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
        json!(["completed", "settled", "no_observation", []])
    );
}

/// Each refused start writes no Run and no capture: no binding, a grant that does not name the
/// Purpose, an input above the item limit, and a source of another Project.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_start_writes_no_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, source) = project(&store, &admin, "fd3").await;
    let (other, other_source) = project(&store, &admin, "fd4").await;
    let unbound = seed_project(&store, "fd50").await;
    let rows = async |scope: &ProjectScope| {
        admin
            .query_one(
                "SELECT (SELECT count(*) FROM storyos.agent_runs AS run
                          WHERE run.project_id = $1::text::uuid),
                        (SELECT count(*) FROM storyos.memory_captures AS capture
                          WHERE capture.project_id = $1::text::uuid)",
                &[&scope.project_id.as_ref()],
            )
            .await
            .map(|row| (row.get::<_, i64>(/*idx*/ 0), row.get::<_, i64>(/*idx*/ 1)))
            .unwrap()
    };
    let before = rows(&scope).await;
    let over = "字".repeat(2 * usize::try_from(storyos_core::CONTEXT_ITEM_TOKEN_LIMIT).unwrap());
    let chapter = MemoryCaptureSource::Chapter {
        chapter_id: "018f0000-0000-7001-8000-0000000fd4ff".to_owned(),
        revision_id: "018f0000-0000-7001-8000-0000000fd4fe".to_owned(),
    };
    let admission = MemoryCaptureStartRefusal::Admission;

    assert_eq!(
        [
            start(&store, &unbound, &source, "雨。").await,
            start(&store, &scope, &source, &over).await,
            start(&store, &scope, &other_source, "雨。").await,
            start(&store, &scope, &chapter, "雨。").await,
        ],
        [
            Err(admission(CaptureRefusal::NoModelUseBinding)),
            Err(admission(CaptureRefusal::InputOverItemLimit)),
            Err(MemoryCaptureStartRefusal::SourceNotFound),
            Err(MemoryCaptureStartRefusal::SourceNotFound),
        ]
    );
    admin
        .execute(
            "UPDATE storyos.project_destination_grants
                SET grant_scope = 'project_writing_assistance_v4'
              WHERE project_id = $1::text::uuid",
            &[&other.project_id.as_ref()],
        )
        .await
        .unwrap();
    assert_eq!(
        start(&store, &other, &other_source, "雨。").await,
        Err(admission(CaptureRefusal::PurposeNotGranted))
    );
    assert_eq!((rows(&scope).await, rows(&unbound).await), (before, (0, 0)));
}

/// The runtime role cannot change the content of a capture or of an observation and cannot
/// delete them, a read in the Scope of another Project sees no row, and the archive has each
/// row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_cannot_change_a_capture_and_the_archive_has_its_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, source) = project(&store, &admin, "fd6").await;
    let other = seed_project(&store, "fd70").await;
    let input = script(&[record(json!([
        {"type": "foreshadowing", "title": "铜铃", "text": "铜铃会再响。"}
    ]))]);
    let started = start(&store, &scope, &source, &input).await.unwrap();
    complete(&store, &admin, &scope, &started.run_id).await;
    let statements = [
        "SELECT 1 FROM storyos.memory_observations",
        "UPDATE storyos.memory_captures SET source_kind = 'chapter'",
        "UPDATE storyos.memory_observations SET text = 'x'",
        "DELETE FROM storyos.memory_captures",
        "DELETE FROM storyos.memory_observations",
    ];
    let refused = || Err("42501".to_owned());
    let reads = [
        "SELECT 1 FROM storyos.memory_captures",
        "SELECT 1 FROM storyos.memory_observations",
    ];

    assert_eq!(
        (
            as_runtime(&scope, &statements).await,
            as_runtime(&other, &reads).await
        ),
        (
            vec![Ok(1), refused(), refused(), refused(), refused()],
            vec![Ok(0), Ok(0)]
        )
    );
    let [captures, observations] = archived_rows(
        &store,
        &scope,
        "fd",
        ["memory_captures", "memory_observations"],
    )
    .await;
    assert_eq!(
        (
            captures
                .iter()
                .map(|row| json!([row["source_kind"], row["state"], row["outcome"]]))
                .collect::<Vec<_>>(),
            observations
                .iter()
                .map(|row| json!([row["entry_type"], row["title"], row["state"]]))
                .collect::<Vec<_>>()
        ),
        (
            vec![json!(["conversation", "settled", "observations_recorded"])],
            vec![json!(["foreshadowing", "铜铃", "waiting"])]
        )
    );
}

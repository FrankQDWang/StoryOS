//! The start of a capture in the store (ADR 0094 sections 5 and 8): the queued Run, the capture,
//! and the first RunStep, each refusal, the runtime role, the Project Scope, and the Project
//! Export Archive. Each test parks its capture Run.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    AgentRunWorkStore, ClaimedAgentRun, ConversationSelection, MemoryCaptureSource,
    MemoryCaptureStartRefusal, MemoryCaptureStore, ModelProviderAdapter, NoContractFaults,
    ProjectScope, StartedMemoryCapture, complete_agent_run,
};
use storyos_core::CaptureRefusal;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_steering::tests::queued;
use crate::command_sequence::tests::seed_project;
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::memory_entries::tests::{archived_rows, as_runtime};
use crate::project_research_binding::tests::park_queued_runs;

/// A Project of `prefix` with one completed author Run, its Chapter, and the source of that Run.
pub(crate) async fn project(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (ProjectScope, String, MemoryCaptureSource) {
    let (scope, chapter_id, run_id) = queued(store, prefix).await;
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
    (scope, chapter_id, source)
}

pub(crate) async fn start(
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

/// The Run of the capture as its kind and status, the capture as its source and state, the
/// Context Assembly of its first RunStep as its Purpose, its cause, and each selected item, and
/// the count of its Model Attempts.
async fn started_rows(admin: &Client, started: &StartedMemoryCapture) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_build_array(run.run_kind, run.status, run.chapter_id,
                      run.conversation_id, capture.source_kind, capture.state, capture.outcome,
                      capture.capture_id::text = $2,
                      (SELECT jsonb_build_array(
                                payload #>> '{operation_requirement,purpose}',
                                payload #>> '{operation_requirement,cause}',
                                jsonb_path_query_array(payload, '$.selected[*].source_class'),
                                jsonb_path_query_array(payload, '$.selected[*].projection_mode'),
                                jsonb_path_query_array(payload, '$.selected[*].content'))
                         FROM storyos.context_assembly_manifests
                        WHERE run_id = run.run_id AND manifest_role = 'decision'
                          AND decision_position = 0),
                      (SELECT count(*) FROM storyos.model_attempts
                        WHERE run_id = run.run_id))::text
               FROM storyos.agent_runs AS run
               JOIN storyos.memory_captures AS capture USING (owner_user_id, project_id, run_id)
              WHERE run.run_id = $1::text::uuid",
            &[&started.run_id, &started.capture_id],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// The next `count` claims of the Worker while the claimable work of each other Project is held.
/// An earlier test can leave claimable work for the HTTP phase, so the claims do not see it: a
/// queued Run is paused, and another Run gets a far lease. The helper restores each Run after
/// the claims.
pub(crate) async fn claim_beside_other_work(
    store: &PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
    count: usize,
) -> Vec<ClaimedAgentRun> {
    let hide = "WITH old AS (SELECT run_id, status, lease_expires_at FROM storyos.agent_runs
                   WHERE project_id <> $1::text::uuid
                     AND status IN ('queued', 'claimed', 'cancelled'))
                UPDATE storyos.agent_runs AS run
                   SET status = CASE old.status WHEN 'queued' THEN 'paused' ELSE old.status END,
                       lease_expires_at = CASE old.status WHEN 'queued' THEN NULL
                                          ELSE 'infinity'::timestamptz END
                  FROM old WHERE run.run_id = old.run_id
             RETURNING old.run_id::text, old.status, old.lease_expires_at::text";
    let hidden = admin.query(hide, &[&scope.project_id.as_ref()]).await;
    let hidden = hidden.unwrap();
    let mut claims = Vec::new();
    for _ in 0..count {
        claims.push(store.claim_next_agent_run(FakeDestination::ADAPTERS).await);
    }
    let restore = "UPDATE storyos.agent_runs AS run
                      SET status = old.status, lease_expires_at = old.lease::timestamptz
                     FROM unnest($1::text[], $2::text[], $3::text[]) AS old(run_id, status, lease)
                    WHERE run.run_id = old.run_id::uuid";
    let column = |idx| -> Vec<Option<String>> { hidden.iter().map(|row| row.get(idx)).collect() };
    let values = [column(0), column(1), column(2)];
    let params: [&(dyn tokio_postgres::types::ToSql + Sync); 3] =
        [&values[0], &values[1], &values[2]];
    admin.execute(restore, &params).await.unwrap();
    claims
        .into_iter()
        .map(|claim| claim.unwrap().unwrap())
        .collect()
}

/// A start writes the queued Run, the capture, and the first RunStep. The Worker claims the capture
/// Run after the earlier Run of the author, and the empty list of the fake settles it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_started_capture_has_its_rows_and_the_worker_claims_it_after_the_run_of_the_author() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "fd1").await;
    let started = start(&store, &scope, &source, "林舟在雨夜撑船。")
        .await
        .unwrap();
    let new = ConversationSelection::New;
    let author = admitted(&store, &scope, &chapter_id, "fd2", "Help.", new).await;
    let queued_rows = started_rows(&admin, &started).await;
    let claims = claim_beside_other_work(&store, &admin, &scope, 2).await;
    let mut claimed = Vec::new();
    for claim in claims {
        let adapter = FakeDestination;
        complete_agent_run(&store, &adapter, &NoContractFaults, &RuntimeClock, &claim)
            .await
            .unwrap();
        claimed.push(claim.run_id);
    }

    assert_eq!(claimed, [author, started.run_id.clone()]);
    let settled = started_rows(&admin, &started).await;
    assert_eq!(
        [&settled[1], &settled[5], &settled[6], &settled[9]],
        [
            &json!("completed"),
            &json!("settled"),
            &json!("no_observation"),
            &json!(1)
        ]
    );
    assert_eq!(
        queued_rows,
        json!([
            "memory_capture",
            "queued",
            null,
            null,
            "conversation",
            "running",
            null,
            true,
            [
                "memory_capture",
                "host_schedule",
                ["memory_capture_input"],
                ["exact_required"],
                ["林舟在雨夜撑船。"]
            ],
            0
        ])
    );
}

/// Each refused start writes no Run and no capture: no binding, an input above the item limit, a
/// source of another Project, a grant that does not name the Purpose, and no assistance.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_start_writes_no_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "fd3").await;
    let (other, _, other_source) = project(&store, &admin, "fd4").await;
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
    let other_id = other.project_id.as_ref();
    let grant = "UPDATE storyos.project_destination_grants
                    SET grant_scope = 'project_writing_assistance_v4' WHERE project_id = $1::text::uuid";
    admin.execute(grant, &[&other_id]).await.unwrap();
    let not_granted = start(&store, &other, &other_source, "雨。").await;
    let policy = "UPDATE storyos.project_policy_revisions SET availability = 'unavailable'
                   WHERE project_id = $1::text::uuid";
    admin.execute(policy, &[&other_id]).await.unwrap();
    assert_eq!(
        [
            not_granted,
            start(&store, &other, &other_source, "雨。").await
        ],
        [
            Err(admission(CaptureRefusal::PurposeNotGranted)),
            Err(admission(CaptureRefusal::AssistanceUnavailable))
        ]
    );
    assert_eq!((rows(&scope).await, rows(&unbound).await), (before, (0, 0)));
}

/// The runtime role cannot change the content of a capture or of an observation and cannot
/// delete them, a read in the Scope of another Project sees no row, and the archive has each
/// row. SQL writes the observation as a test row, because S7-D3b2 settles the output call.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_cannot_change_a_capture_and_the_archive_has_its_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "fd6").await;
    let other = seed_project(&store, "fd70").await;
    let started = start(&store, &scope, &source, "铜铃响了。").await.unwrap();
    park_queued_runs(&admin, &scope).await;
    admin
        .execute(
            "INSERT INTO storyos.memory_observations
               (owner_user_id, project_id, capture_id, position, entry_type, title, text)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 1, 'foreshadowing', '铜铃',
                     '铜铃会再响。')",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &started.capture_id,
            ],
        )
        .await
        .unwrap();
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
    let tables = ["memory_captures", "memory_observations"];
    let [captures, observations] = archived_rows(&store, &scope, "fd", tables).await;
    let keys = ["source_kind", "state", "outcome", "entry_type", "title"];
    let fields = |rows: Vec<Value>| -> Vec<_> {
        rows.iter()
            .map(|row| json!(keys.map(|k| row[k].clone())))
            .collect()
    };
    assert_eq!(
        (fields(captures), fields(observations)),
        (
            vec![json!(["conversation", "running", null, null, null])],
            vec![json!([null, "waiting", null, "foreshadowing", "铜铃"])]
        )
    );
}

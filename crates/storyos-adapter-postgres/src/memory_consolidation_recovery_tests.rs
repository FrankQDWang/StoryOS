//! The bound, the recovery, and the Scope of a consolidation pass (ADR 0094 sections 6 and 8,
//! ADR 0056, ADR 0086). Each test starts a pass with a direct call, and each pass Run completes.
//! The lead of these tests is `61`.

use serde_json::{Value, json};
use storyos_adapter_fake_destination::FakeDestination;
use storyos_application::{
    ContractFaultObserver, ContractFaultPoint, ConversationSelection, MemoryCaptureSource,
    MemoryEntrySourceItem, NoContractFaults, ProjectScope, StartedMemoryConsolidation,
    ToolDispatchStore, complete_agent_run, list_memory_entries,
};
use storyos_core::{AGENT_PLAN_REGISTRATION, MemoryEntryOrigin};
use tokio::sync::Mutex;
use tokio_postgres::Client;

use super::tests::{
    capture, create, delete as delete_operation, run_pass, scripted, settled, update, versions,
};
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::{RuntimeClock, claim_run, stores};
use crate::agent_run_finish_rule::tests::complete;
use crate::agent_run_tool_recovery::tests::{Fault, FaultAt};
use crate::agent_run_write_memory::tests::first_index;
use crate::create_agent_run::context::chapter_list_tests::admitted;
use crate::delete_memory_entry::tests::delete;
use crate::memory_capture::tests::project;
use crate::memory_entries::tests::write_versions;

/// The pass takes the oldest observations whose expected input stays below the window after the
/// output bound of the binding. The pass row records them, and the other observations wait. The
/// Run names the Agent Plan Registration, so the start used the binding that the SQL moved. The
/// Host refuses the request of the pass on the fake destination, so the pass fails, applies
/// nothing, and each observation stays `waiting`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_takes_the_oldest_observations_that_fit_the_request() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "610").await;
    let first = json!([{ "type": "other", "title": "渡口", "text": "渡口在河边。" }]);
    capture(&store, &admin, &scope, &source, first).await;
    let project_id = scope.project_id.as_ref();
    admin
        .execute(
            "INSERT INTO storyos.memory_observations
               (owner_user_id, project_id, capture_id, position, entry_type, title, text)
             SELECT owner_user_id, project_id, capture_id, position + later, entry_type, title,
                    repeat('长', 30000)
               FROM storyos.memory_observations, generate_series(1, 40) AS later
              WHERE project_id = $1::text::uuid",
            &[&project_id],
        )
        .await
        .unwrap();
    crate::model_registration::insert_model_registration(&admin, &AGENT_PLAN_REGISTRATION)
        .await
        .unwrap();
    // The earlier Runs of the Project reference the binding with its Registration, so one
    // statement changes both, and the foreign keys hold at its end.
    admin
        .execute(
            "WITH runs AS (
                 UPDATE storyos.agent_runs SET model_registration_revision = $2::text::uuid
                  WHERE project_id = $1::text::uuid)
             UPDATE storyos.project_external_use_binding_revisions
                SET model_registration_revision = $2::text::uuid,
                    budget_bounds = 'agent_plan_request_v2'
              WHERE project_id = $1::text::uuid",
            &[&project_id, &AGENT_PLAN_REGISTRATION.revision],
        )
        .await
        .unwrap();
    let started = run_pass(&store, &admin, &scope).await;
    let taken: Value = admin
        .query_one(
            "SELECT jsonb_build_array(jsonb_array_length(pass.observations),
                      pass.observations -> -1 ->> 1, run.model_registration_revision)::text
               FROM storyos.memory_consolidation_passes AS pass
               JOIN storyos.agent_runs AS run USING (run_id)
              WHERE pass.pass_id = $1::text::uuid",
            &[&started.pass_id],
        )
        .await
        .unwrap()
        .get::<_, &str>(/*idx*/ 0)
        .parse::<Value>()
        .unwrap();

    assert_eq!(
        (taken, settled(&admin, &started.run_id).await),
        (
            json!([34, "34", AGENT_PLAN_REGISTRATION.revision]),
            json!([
                "refused",
                "failed",
                "destination_request_unsupported",
                [],
                vec!["waiting"; 41]
            ])
        )
    );
}

/// The identity of the entry 1 of the Project of `scope`.
async fn entry_id(admin: &Client, scope: &ProjectScope) -> String {
    admin
        .query_one(
            "SELECT DISTINCT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = 1",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// A Project with an entry of the author (number 1) and one observation whose script creates the
/// entry 2. Returns the Project and the queued pass.
async fn pass_with_author_entry(
    store: &PostgresProjectReader,
    admin: &Client,
    prefix: &str,
) -> (ProjectScope, String, StartedMemoryConsolidation) {
    let (scope, chapter_id, source) = project(store, admin, prefix).await;
    write_versions(
        admin,
        &scope,
        &[(1, 1, 1, "current", "other", "阿青", "船夫")],
    )
    .await;
    let script = json!([scripted("灯", &[create("foreshadowing", "灯", json!([1]))])]);
    capture(store, admin, &scope, &source, script).await;
    let started = crate::memory_consolidation::tests::start_pass(store, &scope)
        .await
        .unwrap();
    (scope, chapter_id, started)
}

/// The Contract Fault Point observer that deletes the entry 1 on the settings page at the dispatch
/// claim of the pass, and keeps the outcome of the delete.
struct DeleteAtDispatch<'a> {
    store: &'a PostgresProjectReader,
    scope: &'a ProjectScope,
    entry: String,
    deleted: Mutex<Option<bool>>,
}

impl ContractFaultObserver for DeleteAtDispatch<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        let mut deleted = self.deleted.lock().await;
        if point == ContractFaultPoint::DispatchClaimed && deleted.is_none() {
            let outcome = delete(self.store, self.scope, 0x611a, &self.entry, 1).await;
            *deleted = Some(outcome.is_ok());
        }
    }
}

/// The author deletes an entry between the dispatch claim and the settlement of the pass. The
/// delete settles at once, and the pass is stale: it applies nothing, and its observation waits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_delete_of_the_author_during_a_pass_makes_the_pass_stale() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, started) = pass_with_author_entry(&store, &admin, "611").await;
    let observer = DeleteAtDispatch {
        store: &store,
        scope: &scope,
        entry: entry_id(&admin, &scope).await,
        deleted: Mutex::new(None),
    };
    let claim = claim_run(&admin, scope.clone(), started.run_id.clone()).await;
    complete_agent_run(&store, &FakeDestination, &observer, &RuntimeClock, &claim)
        .await
        .unwrap();

    assert_eq!(
        (
            *observer.deleted.lock().await,
            settled(&admin, &started.run_id).await,
            versions(&admin, &scope).await,
        ),
        (
            Some(true),
            json!([
                "completed",
                "stale",
                null,
                [["not_applied", null, null]],
                ["waiting"]
            ]),
            json!([])
        )
    );
}

/// Waits until `count` sessions wait on a lock. A blocked session gives no signal, so the test
/// reads `pg_locks` in steps of 10 ms, with a bound.
async fn wait_for_blocked(admin: &Client, count: i64) {
    for _ in 0..1000 {
        let row = admin
            .query_one("SELECT count(*) FROM pg_locks WHERE NOT granted", &[])
            .await
            .unwrap();
        if row.get::<_, i64>(/*idx*/ 0) >= count {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("{count} sessions do not wait on a lock");
}

/// The settlement of a pass takes the lock of the Memory head first. A delete of the author during
/// the settlement waits on that lock, and it applies after the pass, at the next Memory revision.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_delete_of_the_author_waits_on_the_lock_of_the_settlement() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (_, holder) = stores().await;
    let (scope, _, started) = pass_with_author_entry(&store, &admin, "612").await;
    let entry = entry_id(&admin, &scope).await;
    // The holder locks the pass row, so the settlement waits after it took the Memory head lock.
    holder.batch_execute("BEGIN").await.unwrap();
    holder
        .execute(
            "SELECT 1 FROM storyos.memory_consolidation_passes WHERE pass_id = $1::text::uuid
                FOR UPDATE",
            &[&started.pass_id],
        )
        .await
        .unwrap();
    let claim = claim_run(&admin, scope.clone(), started.run_id.clone()).await;
    let pass = complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &claim,
    );
    let author = async {
        wait_for_blocked(&admin, 1).await;
        delete(&store, &scope, 0x612a, &entry, 1).await
    };
    let release = async {
        wait_for_blocked(&admin, 2).await;
        holder.batch_execute("ROLLBACK").await.unwrap();
    };
    let (pass, author, ()) = tokio::join!(pass, author, release);
    let revisions = admin
        .query(
            "SELECT entry_number::text || ':' || memory_revision::text || ':'
                    || COALESCE(cause ->> 'kind', 'none')
               FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid ORDER BY memory_revision",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();

    assert_eq!(
        (
            pass.is_ok() && author.is_ok(),
            settled(&admin, &started.run_id).await[1].clone(),
            revisions
                .iter()
                .map(|row| row.get::<_, &str>(/*idx*/ 0))
                .collect::<Vec<_>>(),
        ),
        (
            true,
            json!("applied"),
            vec!["1:1:none", "2:2:consolidation", "1:3:author_delete"]
        )
    );
}

/// The Worker loses the Run Lease at `point` in a pass. Its stale fence stores nothing, the next
/// claim continues the pass, and a second delivery of the settled call finds no ToolCall.
async fn lost_lease(prefix: &str, point: ContractFaultPoint) -> Value {
    let (store, admin) = stores().await;
    let (scope, _, started) = pass_with_author_entry(&store, &admin, prefix).await;
    let run_id = started.run_id.clone();
    let stale = claim_run(&admin, scope.clone(), run_id.clone()).await;
    let fault = Fault::TakeOver {
        admin: &admin,
        run_id: run_id.clone(),
    };
    let observer = FaultAt::new(point, 1, fault);
    let lost = complete_agent_run(&store, &FakeDestination, &observer, &RuntimeClock, &stale).await;
    let stale_delivery = store.next_tool_call(&stale).await;
    let current = claim_run(&admin, scope.clone(), run_id.clone()).await;
    complete_agent_run(
        &store,
        &FakeDestination,
        &NoContractFaults,
        &RuntimeClock,
        &current,
    )
    .await
    .unwrap();
    let second_delivery = store.next_tool_call(&current).await.unwrap();
    json!([
        lost.is_err() && stale_delivery.is_err(),
        second_delivery.is_none(),
        settled(&admin, &run_id).await,
        versions(&admin, &scope).await.as_array().unwrap().len(),
    ])
}

/// The applied list of a recovered pass: one create, one version, and the consumed observation.
fn applied_one_time() -> Value {
    json!([
        true,
        true,
        [
            "completed",
            "applied",
            null,
            [["applied", null, 2]],
            ["consumed"]
        ],
        1
    ])
}

/// A lost Run Lease after the response of the pass committed (a Model Gateway point).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_lost_lease_after_the_response_applies_the_list_one_time() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let ended = lost_lease("613", ContractFaultPoint::StreamCommitted).await;
    assert_eq!(ended, applied_one_time());
}

/// A lost Run Lease before the Tool Gateway settles the call (a Tool Gateway point).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_lost_lease_before_the_settlement_applies_the_list_one_time() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let ended = lost_lease("614", ContractFaultPoint::ToolCallsCommitted).await;
    assert_eq!(ended, applied_one_time());
}

/// Two Projects each have the entry 1. A pass of one Project changes only its own entry 1, and
/// the observation of the other Project waits.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_changes_only_the_entries_and_observations_of_its_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut projects = Vec::new();
    for prefix in ["615", "616"] {
        let (scope, _, source) = project(&store, &admin, prefix).await;
        let script = json!([scripted("林舟", &[create("character", "林舟", json!([1]))])]);
        capture(&store, &admin, &scope, &source, script).await;
        run_pass(&store, &admin, &scope).await;
        projects.push((scope, source));
    }
    let [(first, first_source), (second, second_source)] = &projects[..] else {
        panic!("two Projects")
    };
    let update = json!([scripted("林舟", &[update(1, "沉默的船夫。")])]);
    capture(&store, &admin, first, first_source, update).await;
    let waiting = json!([{ "type": "other", "title": "渡口", "text": "渡口在河边。" }]);
    capture(&store, &admin, second, second_source, waiting).await;
    let changed = run_pass(&store, &admin, first).await;

    assert_eq!(
        (
            settled(&admin, &changed.run_id).await[4].clone(),
            versions(&admin, first).await.as_array().unwrap().len(),
            versions(&admin, second).await.as_array().unwrap().len(),
            observation_states(&admin, second).await,
        ),
        (
            json!(["consumed", "consumed"]),
            2,
            1,
            json!(["consumed", "waiting"])
        )
    );
}

/// The state of each observation of the Project of `scope`, in the order of its captures.
async fn observation_states(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(item.state ORDER BY capture.started_at, item.position)::text
               FROM storyos.memory_observations AS item
               JOIN storyos.memory_captures AS capture USING (capture_id)
              WHERE item.project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    serde_json::from_str(row.get(/*idx*/ 0)).unwrap()
}

/// After a pass creates two entries and a second pass deletes one, the Memory Index of the next
/// Run of the author lists only the created entry. The settings page gives its origin and its
/// source.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_run_and_the_settings_page_read_the_entries_of_a_pass() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "617").await;
    let created = [
        create("character", "林舟", json!([1])),
        create("foreshadowing", "灯", json!([1])),
    ];
    let script = json!([scripted("林舟", &created)]);
    capture(&store, &admin, &scope, &source, script).await;
    run_pass(&store, &admin, &scope).await;
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([scripted("灯", &[delete_operation(2)])]),
    )
    .await;
    run_pass(&store, &admin, &scope).await;
    let new = ConversationSelection::New;
    let run_id = admitted(&store, &scope, &chapter_id, "618", "Write.", new).await;
    complete(&store, &admin, &scope, &run_id).await;
    let MemoryCaptureSource::Conversation {
        conversation_id, ..
    } = source
    else {
        panic!("the source is a Conversation")
    };
    let listed: Vec<_> = list_memory_entries(&store, &scope)
        .await
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|entry| (entry.number, entry.origin, entry.source))
        .collect();

    assert_eq!(
        (first_index(&admin, &run_id).await, listed),
        (
            json!([r#"{"type":"character"}
{"description":"述。","id":1,"title":"林舟"}"#]),
            vec![(
                1,
                MemoryEntryOrigin::Conversation,
                Some(MemoryEntrySourceItem::Conversation {
                    conversation_id,
                    available: true
                })
            )]
        )
    );
}

//! The return of the observations of a refused operation of a pass (ADR 0094 section 6), each with
//! a pass on the fake destination. The lead of these tests is `51`.

use crate::statement_cache::Client;
use serde_json::{Value, json};
use storyos_application::ProjectScope;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::memory_capture::{project, run_capture};
use crate::shared_tests::memory_consolidation::{
    capture, create, run_pass, scripted, settled, update,
};
use crate::shared_tests::memory_consolidation_schedule::pass_due;
use crate::shared_tests::memory_entries::write_versions;

/// The state of each observation of the Project, from the settlement of the pass of `run_id`.
async fn states(admin: &Client, run_id: &str) -> Value {
    settled(admin, run_id).await[4].clone()
}

/// The value of the key `refused` of each observation line of the request of the pass of `run_id`.
async fn refused_keys(admin: &Client, run_id: &str) -> Vec<Value> {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(item ->> 'content' ORDER BY number)::text
               FROM storyos.context_assembly_manifests AS manifest,
                    jsonb_array_elements(manifest.payload -> 'selected')
                      WITH ORDINALITY AS selected(item, number)
              WHERE manifest.run_id = $1::text::uuid AND manifest.manifest_role = 'decision'
                AND item ->> 'source_class' = 'memory_observations'",
            &[&run_id],
        )
        .await
        .unwrap();
    let contents: Vec<String> = serde_json::from_str(row.get(/*idx*/ 0)).unwrap();
    contents
        .iter()
        .flat_map(|content| content.lines())
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["refused"].clone())
        .collect()
}

/// An update of an entry that the Project does not have, which the Host refuses.
fn refused_update(entry: u64) -> Value {
    update(entry, "新文。")
}

/// The Project of the lead `prefix` with the one capture `observations`.
async fn captured(
    prefix: &str,
    observations: Value,
) -> (crate::PostgresProjectReader, Client, ProjectScope) {
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, prefix).await;
    capture(&store, &admin, &scope, &source, observations).await;
    (store, admin, scope)
}

/// An observation that only a refused operation names stays `waiting`, and an observation that no
/// operation names becomes `consumed`. The next pass has the first one with the key `refused`,
/// and an applied operation of that pass consumes it. The consolidation row is due after the first
/// pass, 30 minutes after its start.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_refused_observation_returns_with_its_label_and_an_applied_operation_consumes_it() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let observations = json!([
        scripted("林舟", &[refused_update(1)]),
        scripted("林雪", &[create("character", "林舟", json!([2]))]),
        { "type": "other", "title": "渡口", "text": "渡口在河边。" }
    ]);
    let (store, admin, scope) = captured("510", observations).await;

    let first = run_pass(&store, &admin, &scope).await;
    let first_states = states(&admin, &first.run_id).await;
    let due = pass_due(&admin, &scope).await;
    let second = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            first_states,
            due,
            refused_keys(&admin, &first.run_id).await,
            refused_keys(&admin, &second.run_id).await,
            settled(&admin, &second.run_id).await,
        ),
        (
            json!(["waiting", "consumed", "consumed"]),
            Some(json!([true, true, "00:30:00"])),
            vec![Value::Null, Value::Null, Value::Null],
            vec![json!("memory_entry_not_found")],
            json!([
                "completed",
                "applied",
                null,
                [["applied", null, 1]],
                ["consumed", "consumed", "consumed"]
            ]),
        )
    );
}

/// An observation that a second pass refuses again becomes `consumed`, so it returns one time. Its
/// line has the label of the first refused operation that named it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_observation_that_a_second_pass_refuses_again_is_consumed() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let operations = [create("unknown", "桥", json!([1])), refused_update(999)];
    let observations = json!([scripted("桥", &operations)]);
    let (store, admin, scope) = captured("511", observations).await;

    let first = run_pass(&store, &admin, &scope).await;
    let first_states = states(&admin, &first.run_id).await;
    let second = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            first_states,
            refused_keys(&admin, &second.run_id).await,
            settled(&admin, &second.run_id).await,
        ),
        (
            json!(["waiting"]),
            vec![json!("memory_entry_invalid")],
            json!([
                "completed",
                "no_operation",
                null,
                [
                    ["refused", "memory_entry_invalid", null],
                    ["refused", "memory_entry_not_found", null]
                ],
                ["consumed"]
            ]),
        )
    );
}

/// An observation that one refused operation and one applied operation name becomes `consumed`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_observation_that_an_applied_operation_also_names_is_consumed() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let operations = [refused_update(999), create("character", "林舟", json!([1]))];
    let observations = json!([scripted("林舟", &operations)]);
    let (store, admin, scope) = captured("512", observations).await;

    let pass = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        settled(&admin, &pass.run_id).await,
        json!([
            "completed",
            "applied",
            null,
            [
                ["refused", "memory_entry_not_found", null],
                ["applied", null, 1]
            ],
            ["consumed"]
        ])
    );
}

/// A stale pass refuses no operation, so it does not use the one return. The observation of a
/// stale pass that the next pass refuses still returns one time, and the second refusal consumes
/// it.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_observation_of_a_stale_pass_still_returns_one_time() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let observations = json!([scripted("桥", &[refused_update(999)])]);
    let (store, admin, scope) = captured("513", observations).await;
    write_versions(
        &admin,
        &scope,
        &[(1, 1, 1, "current", "other", "阿青", "船夫")],
    )
    .await;
    let stale = crate::shared_tests::memory_consolidation::start_pass(&store, &scope)
        .await
        .unwrap();
    let id: String = admin
        .query_one(
            "SELECT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = 1",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let deleted =
        crate::shared_tests::delete_memory_entry::delete(&store, &scope, 0x513a, &id, 1).await;
    run_capture(&store, &admin, &scope, &stale.run_id).await;
    let stale_pass = settled(&admin, &stale.run_id).await;

    let refused = run_pass(&store, &admin, &scope).await;
    let refused_pass = settled(&admin, &refused.run_id).await;
    let again = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            deleted.is_ok(),
            [&stale_pass[1], &stale_pass[4]],
            [&refused_pass[1], &refused_pass[4]],
            refused_keys(&admin, &refused.run_id).await,
            refused_keys(&admin, &again.run_id).await,
            states(&admin, &again.run_id).await,
        ),
        (
            true,
            [&json!("stale"), &json!(["waiting"])],
            [&json!("no_operation"), &json!(["waiting"])],
            vec![Value::Null],
            vec![json!("memory_entry_not_found")],
            json!(["consumed"]),
        )
    );
}

//! The conflict item of a capture and the flags that the Host writes from it (ADR 0094 section
//! 7), and the whole pass with each duty. Each test completes each capture and each pass Run. The
//! lead of these tests is `79`.

use serde_json::{Value, json};
use storyos_application::{MemoryConsolidationStore, MemoryWorkOutcome, ProjectScope};
use storyos_core::ConsolidationSkip;
use tokio_postgres::Client;

use super::super::schedule_tests::claim_pass;
use super::super::tests::{capture, create, delete, run_pass, scripted, settled};
use super::{flag, flag_lines, merge, seed, version_lines};
use crate::agent_run_dispatch::tests::stores;
use crate::agent_run_load_skill::tests::calls;
use crate::memory_capture::loop_tests::{record, script};
use crate::memory_capture::tests::{project, run_capture, start};
use crate::memory_entries::tests::archived_rows;

/// A conflict item of a capture with the entry `entry` and the reason `reason`.
pub(super) fn conflict(entry: u64, reason: &str) -> Value {
    json!({
        "kind": "conflict", "type": null, "title": null, "text": null, "entry": entry,
        "reason": reason
    })
}

/// Each observation row of the Project as one line: the position, the kind, the title, the entry
/// number, the reason, and the state, in the order of the captures.
async fn observation_lines(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(item.position, item.kind, item.title,
                      item.entry_number, item.reason, item.state)
                      ORDER BY capture.started_at, item.position)::text
               FROM storyos.memory_observations AS item
               JOIN storyos.memory_captures AS capture USING (owner_user_id, project_id, capture_id)
              WHERE item.project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    version_lines(&serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
}

/// The identity of the entry `number` of the Project.
pub(super) async fn entry_id(admin: &Client, scope: &ProjectScope, number: u64) -> String {
    let row = admin
        .query_one(
            "SELECT memory_entry_id::text FROM storyos.memory_entry_versions
              WHERE project_id = $1::text::uuid AND entry_number = $2::text::numeric LIMIT 1",
            &[&scope.project_id.as_ref(), &number.to_string()],
        )
        .await
        .unwrap();
    row.get(/*idx*/ 0)
}

/// A conflict item is stored as an observation that waits, with its entry number and its reason,
/// and the Project Export Archive has its row. A conflict of a background entry refuses the list
/// with `conflict_entry_unavailable`, and the next RunStep of the capture settles an empty list.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_conflict_item_waits_as_an_observation_and_names_only_an_entry_of_the_author() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "790").await;
    let rows = [
        (1, "other", "author_request", "阿青"),
        (2, "other", "chapter", "渡口"),
    ];
    seed(&admin, &scope, &rows).await;
    let fact = json!({ "type": "other", "title": "桥", "text": "桥在渡口下游。" });
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([fact, conflict(1, " 与第三章不符。 ")]),
    )
    .await;
    let input = script(&[record(json!([conflict(2, "不符。")]))]);
    let refused = start(&store, &scope, &source, &input).await.unwrap();
    run_capture(&store, &admin, &scope, &refused.run_id).await;
    let calls = calls(&admin, &refused.run_id).await;
    let states: Vec<Value> = (calls[1].as_array().unwrap().iter())
        .map(|call| json!([call[1], call[2]]))
        .collect();
    let [archived] = archived_rows(&store, &scope, "7f", ["memory_observations"]).await;
    let archived: Vec<Value> = (archived.iter())
        .filter(|row| row["kind"] == "conflict")
        .map(|row| json!([row["entry_number"], row["reason"], row["entry_type"]]))
        .collect();

    assert_eq!(
        (observation_lines(&admin, &scope).await, states, archived),
        (
            json!([
                "1 observation 桥 null null waiting",
                "2 conflict null 1 与第三章不符。 waiting",
            ]),
            vec![
                json!(["refused", "conflict_entry_unavailable"]),
                json!(["settled", null])
            ],
            vec![json!([1, "与第三章不符。", null])]
        )
    );
}

/// A pass that a delete of the author makes stale writes no flag, and its conflicts wait. The
/// next pass writes the Host flag of the entry that is still current after the flag of the model,
/// so the Host flag is the current flag. The conflict of the deleted entry gives no flag. Each
/// observation is then consumed.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_next_pass_that_is_not_stale_flags_each_current_entry_of_a_conflict() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "791").await;
    let rows = [
        (1, "other", "author_request", "阿青"),
        (2, "other", "author_request", "阿舟"),
    ];
    seed(&admin, &scope, &rows).await;
    let conflicts = json!([conflict(1, "甲"), conflict(2, "乙")]);
    capture(&store, &admin, &scope, &source, conflicts).await;
    let script = json!([scripted("脚本", &[flag(1, "模型")])]);
    capture(&store, &admin, &scope, &source, script).await;
    let stale = store
        .start_memory_consolidation(&scope)
        .await
        .unwrap()
        .unwrap();
    let id = entry_id(&admin, &scope, 2).await;
    let deleted = crate::delete_memory_entry::tests::delete(&store, &scope, 0x79fa, &id, 1).await;
    run_capture(&store, &admin, &scope, &stale.run_id).await;
    let flagged = run_pass(&store, &admin, &scope).await;

    assert_eq!(
        (
            deleted.is_ok(),
            settled(&admin, &stale.run_id).await[1].clone(),
            settled(&admin, &flagged.run_id).await[3].clone(),
            flag_lines(&admin, &scope).await,
            observation_lines(&admin, &scope).await
        ),
        (
            true,
            json!("stale"),
            json!([["applied", null, 1]]),
            json!([
                "1 1 1 current author_request 阿青 述。 文。 seed null null",
                "1 2 4 current author_request 阿青 述。 文。 conflict_flag 模型 conversation",
                "1 3 5 current author_request 阿青 述。 文。 conflict_flag 甲 conversation",
                "2 1 2 current author_request 阿舟 述。 文。 seed null null",
                "2 2 3 deleted author_request 阿舟 述。  author_delete null null",
            ]),
            json!([
                "1 conflict null 1 甲 consumed",
                "2 conflict null 2 乙 consumed",
                "1 observation 脚本 null null consumed",
            ])
        )
    );
}

/// Two Projects each have an entry of the author with the number 1. A conflict of one Project
/// flags only its own entry, and a conflict with a number that only the other Project has is
/// refused.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_conflict_flags_only_an_entry_of_its_own_project() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (first, _, first_source) = project(&store, &admin, "792").await;
    let (other, _, other_source) = project(&store, &admin, "793").await;
    let rows = [
        (1, "other", "author_request", "阿青"),
        (2, "other", "author_request", "阿舟"),
    ];
    seed(&admin, &first, &rows).await;
    seed(&admin, &other, &rows[..1]).await;
    capture(
        &store,
        &admin,
        &first,
        &first_source,
        json!([conflict(1, "甲")]),
    )
    .await;
    let input = script(&[record(json!([conflict(2, "乙")]))]);
    let refused = start(&store, &other, &other_source, &input).await.unwrap();
    run_capture(&store, &admin, &other, &refused.run_id).await;
    run_pass(&store, &admin, &first).await;

    assert_eq!(
        (
            flag_lines(&admin, &first).await[1].clone(),
            flag_lines(&admin, &other).await,
            calls(&admin, &refused.run_id).await[1][0][2].clone()
        ),
        (
            json!("1 2 3 current author_request 阿青 述。 文。 conflict_flag 甲 conversation"),
            json!(["1 1 1 current author_request 阿青 述。 文。 seed null null"]),
            json!("conflict_entry_unavailable")
        )
    );
}

/// A pass with only a conflict item sends the entries and no observation. The model gives an
/// empty list, and the Host writes the flag after it, so the pass is `applied`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_with_only_a_conflict_sends_the_entries_and_flags_the_entry() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "795").await;
    seed(&admin, &scope, &[(1, "other", "author_request", "阿青")]).await;
    capture(&store, &admin, &scope, &source, json!([conflict(1, "甲")])).await;
    let started = run_pass(&store, &admin, &scope).await;
    // The source classes of the decision Context Assembly of the pass.
    let classes: String = admin
        .query_one(
            "SELECT jsonb_path_query_array(payload, '$.selected[*].source_class')::text
               FROM storyos.context_assembly_manifests
              WHERE run_id = $1::text::uuid AND manifest_role = 'decision'",
            &[&started.run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            serde_json::from_str::<Value>(&classes).unwrap(),
            settled(&admin, &started.run_id).await,
            flag_lines(&admin, &scope).await[1].clone()
        ),
        (
            json!(["memory_entries"]),
            json!(["completed", "applied", null, [], ["consumed"]]),
            json!("1 2 2 current author_request 阿青 述。 文。 conflict_flag 甲 conversation")
        )
    );
}

/// Only conflicts wait, and the Project has no current entry. Then the claim of the consolidation
/// row starts no pass. There is no Run and no pass row, the conflict waits, and the due time is
/// empty.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn no_pass_starts_for_conflicts_when_the_project_has_no_current_entry() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "796").await;
    seed(&admin, &scope, &[(1, "other", "author_request", "阿青")]).await;
    capture(&store, &admin, &scope, &source, json!([conflict(1, "甲")])).await;
    let id = entry_id(&admin, &scope, 1).await;
    let deleted = crate::delete_memory_entry::tests::delete(&store, &scope, 0x79fb, &id, 1).await;
    let outcome = claim_pass(&store, &admin, &scope).await;
    let passes: i64 = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.memory_consolidation_passes
                      WHERE project_id = $1::text::uuid)
                  + (SELECT count(*) FROM storyos.agent_runs
                      WHERE project_id = $1::text::uuid AND run_kind = 'memory_consolidation')",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);

    assert_eq!(
        (
            deleted.is_ok(),
            outcome,
            passes,
            observation_lines(&admin, &scope).await
        ),
        (
            true,
            MemoryWorkOutcome::PassSkipped(ConsolidationSkip::NothingWaits),
            0,
            json!(["1 conflict null 1 甲 waiting"])
        )
    );
}

/// The last version of each entry of the Project as one line: the number, the state, the title,
/// the length of the text, and the reason of the flag.
async fn last_versions(admin: &Client, scope: &ProjectScope) -> Value {
    let row = admin
        .query_one(
            "SELECT jsonb_agg(jsonb_build_array(entry_number, state, title, char_length(text),
                      flag_reason) ORDER BY entry_number)::text
               FROM (SELECT DISTINCT ON (entry_number) * FROM storyos.memory_entry_versions
                      WHERE project_id = $1::text::uuid
                      ORDER BY entry_number, memory_revision DESC) AS latest",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    version_lines(&serde_json::from_str(row.get(/*idx*/ 0)).unwrap())
}

/// One pass with each duty: it deletes an out-of-date entry, merges two duplicated entries,
/// shortens a long entry, and creates an entry from a new observation. The Host flags the entry
/// of the author from the conflict of a capture, and its text stays. The create of the title of a
/// delete mark is refused with `deleted_by_author` in the pass row.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn one_pass_does_each_duty_of_consolidation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "794").await;
    let rows = [
        (1, "other", "chapter", "旧桥"),
        (2, "character", "chapter", "林舟"),
        (3, "character", "conversation", "阿舟"),
        (4, "other", "chapter", "长条"),
        (5, "other", "author_request", "阿青"),
        (6, "other", "author_delete", "旧灯"),
    ];
    seed(&admin, &scope, &rows).await;
    admin
        .execute(
            "UPDATE storyos.memory_entry_versions SET text = repeat('长', 600)
              WHERE project_id = $1::text::uuid AND entry_number = 4",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([conflict(5, "不符。")]),
    )
    .await;
    let shorter = json!({
        "action": "update", "entry": 4, "entries": null, "type": null, "title": "长条",
        "description": "述。", "text": "短文。", "observations": [1], "reason": null
    });
    let operations = [
        delete(1),
        merge(json!([2, 3]), "林舟"),
        shorter,
        create("other", "新桥", json!([1])),
        create("other", "旧灯", json!([1])),
    ];
    let observations = json!([scripted("脚本", &operations)]);
    capture(&store, &admin, &scope, &source, observations).await;
    let started = run_pass(&store, &admin, &scope).await;
    let applied = |entry: u64| json!(["applied", null, entry]);

    assert_eq!(
        (
            settled(&admin, &started.run_id).await[3].clone(),
            last_versions(&admin, &scope).await
        ),
        (
            json!([
                applied(1),
                applied(2),
                applied(4),
                applied(7),
                ["refused", "deleted_by_author", null]
            ]),
            json!([
                "1 deleted 旧桥 0 null",
                "2 current 林舟 3 null",
                "3 deleted 阿舟 0 null",
                "4 current 长条 3 null",
                "5 current 阿青 2 不符。",
                "6 deleted 旧灯 2 null",
                "7 current 新桥 2 null",
            ])
        )
    );
}

//! The other start rules of an intent pass in the store (ADR 0101 section 4): the writes of the
//! author, the consolidation work first, the rate, the result that is not kept, and a failed pass.
//! The lead of these tests is `35`. Each test completes each Run that it queued.

use std::time::SystemTime;

use serde_json::json;
use storyos_application::MemoryWorkOutcome;
use storyos_core::{ConsolidationSkip, IntentPassSkip};

use super::tests::{
    NOT_KEPT, age, change, character, claim, consolidate, file, finish, intent_due, intents,
    make_due, outcomes, results, run_intent,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::delete_memory_entry::delete as delete_entry;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::{project, run_capture};
use crate::shared_tests::memory_conflict_flag_show::{run, write};
use crate::shared_tests::memory_consolidation::{capture, run_pass, scripted, settled, start_pass};
use crate::shared_tests::memory_consolidation_conflict::{conflict, entry_id};
use crate::shared_tests::memory_consolidation_schedule::{claim_pass, pass_due};

const FAKE: &str = "我明天一早去渡口，看看那条船还在不在。";

/// The file item of a pass of 林舟 with the part of the author `author` and the text `text`.
fn told(author: Option<&str>, text: &str) -> serde_json::Value {
    let author = author.map_or(String::new(), |author| {
        format!("Told by the author:\n{author}\n\n")
    });
    json!(format!(
        "{{\"name\":\"林舟\"}}\n\n{author}From the manuscript:\n{text}"
    ))
}

/// The started pass of a claim.
fn started(outcome: MemoryWorkOutcome) -> String {
    let MemoryWorkOutcome::IntentPassStarted(started) = outcome else {
        panic!("the claim must start an intent pass: {outcome:?}");
    };
    started.run_id
}

/// The `write_memory` create of the part of the author of 林舟 with `text`.
fn author_part(text: &str) -> String {
    let arguments =
        json!({"type": "character", "title": "林舟", "description": "船夫。", "text": text});
    write("create", arguments)
}

/// An applied `write_memory` create of the part of the author and an applied delete of it each
/// make the row due, and the next pass reads the file of that time.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_write_and_a_delete_of_the_author_make_the_row_due() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "350").await;
    consolidate(
        &store,
        &admin,
        &scope,
        &source,
        &[character("林舟", "林舟是船夫。")],
    )
    .await;
    let mut files = vec![file(&admin, &run_intent(&store, &admin, &scope).await).await];
    run(
        &store,
        &admin,
        (&scope, &chapter_id),
        "354",
        &author_part("他怕水。"),
    )
    .await;
    let mut due = vec![intent_due(&admin, &scope).await];
    files.push(file(&admin, &run_intent(&store, &admin, &scope).await).await);
    let author = entry_id(&admin, &scope, 2).await;
    delete_entry(&store, &scope, 0x35a0, &author, 1)
        .await
        .unwrap();
    due.push(intent_due(&admin, &scope).await);
    files.push(file(&admin, &run_intent(&store, &admin, &scope).await).await);

    assert_eq!(
        (due, files, outcomes(&admin, &scope).await),
        (
            vec![json!(true), json!(true)],
            vec![
                told(None, "林舟是船夫。"),
                told(Some("他怕水。"), "林舟是船夫。"),
                told(None, "林舟是船夫。")
            ],
            json!([
                [1, "recorded", 1, null],
                [1, "recorded", 1, 1],
                [1, "recorded", 1, null]
            ])
        )
    );
}

/// No pass starts while a consolidation pass is in progress or while an observation waits, and
/// the claim makes the due time empty. The settlement of a consolidation pass with the outcome
/// `no_operation` makes the row due again, and the pass starts. A consolidation pass after the
/// result reads the text of the entry and applies its operations.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_passes_wait_for_the_consolidation_work_of_the_project() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "351").await;
    consolidate(
        &store,
        &admin,
        &scope,
        &source,
        &[character("林舟", "林舟是船夫。")],
    )
    .await;
    // A pass of only a conflict item is in progress, and no observation waits.
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([conflict(1, "他不会游泳。")]),
    )
    .await;
    let pass = start_pass(&store, &scope).await.unwrap().run_id;
    let in_progress = (
        claim(&store, &admin, &scope).await,
        intent_due(&admin, &scope).await,
    );
    run_capture(&store, &admin, &scope, &pass).await;
    let after_conflict = (
        settled(&admin, &pass).await[1].clone(),
        intent_due(&admin, &scope).await,
    );
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([scripted("脚本", &[])]),
    )
    .await;
    let waits = (
        claim(&store, &admin, &scope).await,
        intent_due(&admin, &scope).await,
    );
    let empty = run_pass(&store, &admin, &scope).await.run_id;
    let after_empty = (
        settled(&admin, &empty).await[1].clone(),
        intent_due(&admin, &scope).await,
    );
    run_intent(&store, &admin, &scope).await;
    let changed = [change(1, "林舟", "林舟换了船。")];
    let changed = consolidate(&store, &admin, &scope, &source, &changed).await;
    let read = "SELECT to_jsonb(bool_or(payload::text LIKE '%林舟是船夫。%'))::text
                  FROM storyos.context_assembly_manifests
                 WHERE run_id = $1::text::uuid AND manifest_role = 'decision'";
    let skipped = |skip| MemoryWorkOutcome::IntentPassSkipped(skip);

    assert_eq!(
        (in_progress, after_conflict, waits, after_empty),
        (
            (
                skipped(IntentPassSkip::ConsolidationInProgress),
                json!(false)
            ),
            (json!("no_operation"), json!(true)),
            (skipped(IntentPassSkip::ObservationWaits), json!(false)),
            (json!("no_operation"), json!(true))
        )
    );
    assert_eq!(
        (
            json_of(&admin, read, &[&changed]).await,
            settled(&admin, &changed).await[1].clone(),
            intents(&admin, &scope, 1).await,
            intent_due(&admin, &scope).await
        ),
        (
            json!(true),
            json!("applied"),
            json!([
                [1, "consolidation", null, null],
                [2, "intent_pass", FAKE, ["我以前去过河对岸吗？"]],
                [3, "consolidation", null, null]
            ]),
            json!(true)
        )
    );
}

/// A Project starts at most one pass in one minute: the claim before that time starts no pass,
/// and the row is due at the start of the last pass and one minute. A failed pass gets no second
/// start for the same versions, also after two more claims.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn one_pass_starts_in_a_minute_and_a_failed_pass_does_not_start_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) = project(&store, &admin, "352").await;
    let operations = [
        character("林舟", "林舟是船夫。\nSCRIPT:intent-no-call"),
        character("阿青", "阿青是船夫。"),
    ];
    consolidate(&store, &admin, &scope, &source, &operations).await;
    run_intent(&store, &admin, &scope).await;
    let limited = claim(&store, &admin, &scope).await;
    let row = admin
        .query_one(
            "SELECT work.due_at, work.due_at = pass.started_at + interval '60 seconds',
                    work.due_at > clock_timestamp()
               FROM storyos.memory_work_schedules AS work
               JOIN storyos.intent_passes AS pass USING (owner_user_id, project_id)
              WHERE work.project_id = $1::text::uuid AND work.work_kind = 'intent_pass'",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let due_at: SystemTime = row.get(/*idx*/ 0);
    run_intent(&store, &admin, &scope).await;
    let mut again = Vec::new();
    for _ in 0..2 {
        make_due(&admin, &scope).await;
        again.push((
            claim(&store, &admin, &scope).await,
            intent_due(&admin, &scope).await,
        ));
    }

    assert_eq!(
        (
            limited,
            [row.get::<_, bool>(/*idx*/ 1), row.get(/*idx*/ 2)],
            outcomes(&admin, &scope).await,
            again
        ),
        (
            MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::RateLimited { due_at }),
            [true, true],
            json!([[1, "failed", 1, null], [2, "recorded", 1, null]]),
            vec![
                (
                    MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::NoEntryNeedsPass),
                    json!(false)
                );
                2
            ]
        )
    );
}

/// A consolidation change and a `write_memory` change of the part of the author between the
/// start and the settlement of a pass each give `not_kept`, with no intent version. The entry
/// needs a pass again, and the later pass records a result for the new text.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_change_during_a_pass_keeps_no_result_and_a_later_pass_reads_the_new_text() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "353").await;
    let project = (&scope, chapter_id.as_str());
    consolidate(
        &store,
        &admin,
        &scope,
        &source,
        &[character("林舟", "林舟是船夫。")],
    )
    .await;
    let first = started(claim(&store, &admin, &scope).await);
    make_due(&admin, &scope).await;
    let running = (
        claim(&store, &admin, &scope).await,
        intent_due(&admin, &scope).await,
    );
    // The test starts the consolidation pass directly, as the claim does not while a pass runs.
    let changed = [change(1, "林舟", "林舟换了船。")];
    consolidate(&store, &admin, &scope, &source, &changed).await;
    finish(&store, &admin, &scope, &first).await;
    let second = run_intent(&store, &admin, &scope).await;
    run(&store, &admin, project, "355", &author_part("他怕水。")).await;
    age(&admin, &scope).await;
    let third = started(claim(&store, &admin, &scope).await);
    // An observation waits during the pass: the claim of the consolidation row starts no pass, and
    // the settlement of the intent pass makes the row due again.
    capture(
        &store,
        &admin,
        &scope,
        &source,
        json!([scripted("脚本", &[])]),
    )
    .await;
    let consolidation = (
        claim_pass(&store, &admin, &scope).await,
        pass_due(&admin, &scope).await,
    );
    let author_change = write("change", json!({"entry": 2, "text": "他会游泳。"}));
    run(&store, &admin, project, "356", &author_change).await;
    finish(&store, &admin, &scope, &third).await;
    let consolidation_due = pass_due(&admin, &scope).await;
    run_pass(&store, &admin, &scope).await;
    let fourth = run_intent(&store, &admin, &scope).await;
    let skipped = (
        MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::PassInProgress),
        MemoryWorkOutcome::PassSkipped(ConsolidationSkip::IntentPassInProgress),
    );

    assert_eq!(
        (running, consolidation, consolidation_due),
        (
            (skipped.0, json!(false)),
            (skipped.1, Some(json!([true, true, null]))),
            Some(json!([true, true, "00:30:00"]))
        )
    );

    assert_eq!(
        (
            outcomes(&admin, &scope).await,
            [results(&admin, &first).await, results(&admin, &third).await],
            [file(&admin, &second).await, file(&admin, &fourth).await],
            intents(&admin, &scope, 1).await
        ),
        (
            json!([
                [1, "not_kept", 1, null],
                [1, "recorded", 2, null],
                [1, "not_kept", 2, 1],
                [1, "recorded", 2, 2]
            ]),
            [json!([NOT_KEPT]), json!([NOT_KEPT])],
            [
                told(None, "林舟换了船。"),
                told(Some("他会游泳。"), "林舟换了船。")
            ],
            json!([
                [1, "consolidation", null, null],
                [2, "consolidation", null, null],
                [3, "intent_pass", FAKE, ["我以前去过河对岸吗？"]],
                [4, "intent_pass", FAKE, ["我以前去过河对岸吗？"]]
            ])
        )
    );
}

//! The other start rules of an intent pass in the store (ADR 0101 section 4): the writes of the
//! author, the consolidation work first, the rate, the result that is not kept, and a failed pass.
//! The lead of these tests is `35`. Each test completes each Run that it queued.

use std::time::SystemTime;

use serde_json::json;
use storyos_application::MemoryWorkOutcome;
use storyos_core::IntentPassSkip;

use super::tests::{
    NOT_KEPT, age, change, character, claim, consolidate, file, finish, intent_due, intents,
    make_due, memory_revision, outcomes, results, run_intent,
};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::delete_memory_entry::delete as delete_entry;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::{project, run_capture};
use crate::shared_tests::memory_conflict_flag_show::{run, write};
use crate::shared_tests::memory_consolidation::{capture, run_pass, scripted, settled, start_pass};
use crate::shared_tests::memory_consolidation_conflict::{conflict, entry_id};
use crate::shared_tests::memory_consolidation_schedule::claim_pass;

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
/// make the row due, and the next pass reads the file of that time. While a character needs a
/// pass and the row is not due, an applied create and an applied delete of an entry of another
/// type leave the row not due.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_write_and_a_delete_of_the_author_make_the_row_due() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "350").await;
    let project = (&scope, chapter_id.as_str());
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    let first = started(claim(&store, &admin, &scope).await);
    // The claim during the running pass makes the row not due. 林舟 still needs a pass.
    make_due(&admin, &scope).await;
    claim(&store, &admin, &scope).await;
    let other =
        json!({"type": "other", "title": "渡口", "description": "河边。", "text": "渡口。"});
    run(&store, &admin, project, "357", &write("create", other)).await;
    let mut due = vec![intent_due(&admin, &scope).await];
    let other = entry_id(&admin, &scope, 2).await;
    delete_entry(&store, &scope, 0x35a1, &other, 1)
        .await
        .unwrap();
    due.push(intent_due(&admin, &scope).await);
    let states = "SELECT jsonb_agg(state ORDER BY version)::text FROM storyos.memory_entry_versions
                   WHERE project_id = $1::text::uuid AND entry_number = 2";
    let states = json_of(&admin, states, &[&scope.project_id.as_ref()]).await;
    finish(&store, &admin, &scope, &first).await;
    let mut files = vec![file(&admin, &first).await];
    run(&store, &admin, project, "354", &author_part("他怕水。")).await;
    due.push(intent_due(&admin, &scope).await);
    files.push(file(&admin, &run_intent(&store, &admin, &scope).await).await);
    let author = entry_id(&admin, &scope, 3).await;
    delete_entry(&store, &scope, 0x35a0, &author, 1)
        .await
        .unwrap();
    due.push(intent_due(&admin, &scope).await);
    files.push(file(&admin, &run_intent(&store, &admin, &scope).await).await);

    assert_eq!(
        (states, due, files, outcomes(&admin, &scope).await),
        (
            json!(["current", "deleted"]),
            vec![json!(false), json!(false), json!(true), json!(true)],
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

/// A Project starts at most one pass in one minute: the settlement makes the row due at the start
/// of the last pass and one minute, and a claim before that time starts no pass and writes that
/// due time again. A failed pass gets no second
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
    // The due time of the row, whether it is the start of the pass and one minute, and whether it
    // is later than now.
    let row = || async {
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
        (due_at, [row.get::<_, bool>(/*idx*/ 1), row.get(/*idx*/ 2)])
    };
    let settled = row().await;
    // A claim before that time.
    make_due(&admin, &scope).await;
    let limited = claim(&store, &admin, &scope).await;
    let (due_at, after_claim) = row().await;
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
            (settled, limited, after_claim),
            outcomes(&admin, &scope).await,
            again
        ),
        (
            (
                (due_at, [true, true]),
                MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::RateLimited { due_at }),
                [true, true]
            ),
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

/// A consolidation pass that the claim starts during an intent pass: when it settles first with a
/// new text, the intent pass settles with `not_kept`, and the next pass records a result for that
/// text. When it is still in progress at the intent
/// settlement, the consolidation work goes first: the intent pass settles with `not_kept`, writes
/// no version, and leaves the Memory revision that the consolidation pass pinned, so that pass
/// applies its operations. A `write_memory` change of the part of the author during a pass also
/// gives `not_kept`. Each later pass records a result for the new text.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_change_during_a_pass_keeps_no_result_and_a_later_pass_reads_the_new_text() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "353").await;
    let project = (&scope, chapter_id.as_str());
    let create = [character("林舟", "林舟是船夫。")];
    consolidate(&store, &admin, &scope, &source, &create).await;
    // An observation arrives during the intent pass, and the claim starts a consolidation pass.
    let consolidation = |text: &'static str| {
        let (store, admin, scope, source) = (&store, &admin, &scope, &source);
        async move {
            let changed = [change(1, "林舟", text)];
            capture(
                store,
                admin,
                scope,
                source,
                json!([scripted("脚本", &changed)]),
            )
            .await;
            let MemoryWorkOutcome::PassStarted(pass) = claim_pass(store, admin, scope).await else {
                panic!("the claim must start a consolidation pass");
            };
            pass.run_id
        }
    };
    let first = started(claim(&store, &admin, &scope).await);
    make_due(&admin, &scope).await;
    let running = (
        claim(&store, &admin, &scope).await,
        intent_due(&admin, &scope).await,
    );
    let applied_first = consolidation("林舟换了船。").await;
    run_capture(&store, &admin, &scope, &applied_first).await;
    finish(&store, &admin, &scope, &first).await;
    let between = run_intent(&store, &admin, &scope).await;
    // The part of the author gives the entry a need for the next pass.
    run(&store, &admin, project, "355", &author_part("他怕水。")).await;
    age(&admin, &scope).await;
    let second = started(claim(&store, &admin, &scope).await);
    let in_progress = consolidation("林舟又换了船。").await;
    let pinned = memory_revision(&admin, &scope).await;
    finish(&store, &admin, &scope, &second).await;
    let after_intent = memory_revision(&admin, &scope).await;
    run_capture(&store, &admin, &scope, &in_progress).await;
    let third = run_intent(&store, &admin, &scope).await;
    let author_change = |text| write("change", json!({"entry": 2, "text": text}));
    run(&store, &admin, project, "358", &author_change("他会撑船。")).await;
    age(&admin, &scope).await;
    let fourth = started(claim(&store, &admin, &scope).await);
    run(&store, &admin, project, "356", &author_change("他会游泳。")).await;
    finish(&store, &admin, &scope, &fourth).await;
    let fifth = run_intent(&store, &admin, &scope).await;
    let first_text = "The Host did not keep the intent: a consolidation pass of the Memory is in \
                      progress.";
    let applied = [
        settled(&admin, &applied_first).await[1].clone(),
        settled(&admin, &in_progress).await[1].clone(),
    ];

    assert_eq!(
        (running, after_intent, applied),
        (
            (
                MemoryWorkOutcome::IntentPassSkipped(IntentPassSkip::PassInProgress),
                json!(false)
            ),
            pinned,
            [json!("applied"), json!("applied")]
        )
    );
    assert_eq!(
        (
            outcomes(&admin, &scope).await,
            [
                results(&admin, &first).await,
                results(&admin, &second).await,
                results(&admin, &fourth).await
            ],
            [
                file(&admin, &between).await,
                file(&admin, &third).await,
                file(&admin, &fifth).await
            ],
            intents(&admin, &scope, 1).await
        ),
        (
            json!([
                [1, "not_kept", 1, null],
                [1, "recorded", 2, null],
                [1, "not_kept", 2, 1],
                [1, "recorded", 4, 1],
                [1, "not_kept", 4, 2],
                [1, "recorded", 4, 3]
            ]),
            [json!([NOT_KEPT]), json!([first_text]), json!([NOT_KEPT])],
            [
                told(None, "林舟换了船。"),
                told(Some("他怕水。"), "林舟又换了船。"),
                told(Some("他会游泳。"), "林舟又换了船。")
            ],
            json!([
                [1, "consolidation", null, null],
                [2, "consolidation", null, null],
                [3, "intent_pass", FAKE, ["我以前去过河对岸吗？"]],
                [4, "consolidation", null, null],
                [5, "intent_pass", FAKE, ["我以前去过河对岸吗？"]],
                [6, "intent_pass", FAKE, ["我以前去过河对岸吗？"]]
            ])
        )
    );
}

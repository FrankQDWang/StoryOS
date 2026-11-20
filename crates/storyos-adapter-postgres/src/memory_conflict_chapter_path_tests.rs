//! The path of ADR 0094 section 7 with a Chapter as the source (S7-D6b): the commit of a Chapter,
//! its capture from the schedule of S7-D5, the pass, the flag in a Run, and its removal by a
//! change of the author. Each Run completes. The lead of this test is `a0`.

use serde_json::{Value, json};
use storyos_application::MemoryWorkSource;
use storyos_application::{MemoryEntrySourceItem, MemoryWorkOutcome, MemoryWorkScheduleStore};

use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_read_chapter::project;
use crate::shared_tests::agent_run_write_memory::first_index;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::chapter_capture_schedule::make_chapter_due;
use crate::shared_tests::memory_capture::run_capture;
use crate::shared_tests::memory_capture_loop::record;
use crate::shared_tests::memory_conflict_flag_show::{listed_flags, run, write};
use crate::shared_tests::memory_consolidation::settled;
use crate::shared_tests::memory_consolidation_conflict::conflict;
use crate::shared_tests::memory_consolidation_merge::{flag_lines, seed};
use crate::shared_tests::memory_consolidation_schedule::{claim_pass, pass_due};
use crate::shared_tests::model_gateway::stores;

/// The path on the fake destination. The second Block of the Chapter is the capture script of
/// the fake model, so the Chapter capture gives a conflict item for entry 1. The pass has only
/// that conflict, and it records the flag with the Chapter as its source. The next Run has the
/// flag in its Index and reads the Chapter as the source. A change of the author removes the flag.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_chapter_capture_flags_an_entry_of_the_author_and_a_change_removes_the_flag() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let reason = "第一章写林舟游过了河。";
    let (call, arguments) = record(json!([conflict(1, reason)]));
    let script = format!("SCRIPT:capture {call} {arguments}");
    // The Project has a completed Run, and the commit of Chapter A writes the row of the Chapter.
    let blocks = ["林舟游过了河。", script.as_str()];
    let (scope, chapter_a, _, _) = project(&store, &admin, "a00", 0xa0f0, &blocks).await;
    let project = (&scope, chapter_a.as_str());
    seed(
        &admin,
        &scope,
        &[(1, "character", "author_request", "林舟")],
    )
    .await;
    let row = "SELECT jsonb_build_array(held, due_at > clock_timestamp())::text
                 FROM storyos.memory_work_schedules
                WHERE work_kind = 'chapter_capture' AND source_id = $1::text::uuid";
    let row: String = admin
        .query_one(row, &[&chapter_a])
        .await
        .unwrap()
        .get(/*idx*/ 0);

    // The live run makes the row due with one statement, as the test does here.
    make_chapter_due(&admin, &scope, &chapter_a, /*seconds*/ 1).await;
    let claimed = store.claim_due_memory_work().await.unwrap().unwrap();
    let MemoryWorkOutcome::Started(capture) = claimed.outcome else {
        panic!("the claim of the Chapter row must start a capture");
    };
    run_capture(&store, &admin, &scope, &capture.run_id).await;
    let due = pass_due(&admin, &scope).await;
    let MemoryWorkOutcome::PassStarted(pass) = claim_pass(&store, &admin, &scope).await else {
        panic!("a pass with only the conflict must start");
    };
    run_capture(&store, &admin, &scope, &pass.run_id).await;

    let read = run(&store, &admin, project, "a01", "SCRIPT:read_memory_entry 1").await;
    let flagged = listed_flags(&store, &scope).await;
    let change = write("change", json!({"entry": 1, "text": "他会游泳。"}));
    run(&store, &admin, project, "a02", &change).await;
    let after = run(&store, &admin, project, "a03", "SCRIPT:read_memory_entry 1").await;
    let result = |calls: Value| calls[1][0][3].as_str().unwrap().to_owned();
    let line = "{\"description\":\"述。\",\"id\":1,\"title\":\"林舟\"}";
    let index = |line: &str| json!([format!("{{\"type\":\"character\"}}\n{line}")]);
    let source = MemoryEntrySourceItem::Chapter {
        chapter_id: chapter_a.clone(),
        title: Some("Chapter A".to_owned()),
        available: true,
    };

    assert_eq!(
        (
            serde_json::from_str::<Value>(&row).unwrap(),
            claimed.source,
            due,
            settled(&admin, &pass.run_id).await,
            first_index(&admin, &read).await,
            first_index(&admin, &after).await,
        ),
        (
            json!([false, true]),
            MemoryWorkSource::Chapter(chapter_a.clone()),
            Some(json!([true, true, "now"])),
            json!(["completed", "applied", null, [], ["consumed"]]),
            index(&line.replace("{", "{\"conflict_flag\":true,")),
            index(line),
        )
    );
    let read = result(calls(&admin, &read).await);
    let after = result(calls(&admin, &after).await);
    let chapter_line = "conflict_source: Chapter 1 of the Chapter list, Chapter A";
    assert_eq!(
        (
            read.ends_with(&format!(
                "\nconflict_flag: {reason}\n{chapter_line}\n\n文。"
            )),
            after.contains("conflict_"),
            after.ends_with("\n\n他会游泳。"),
            flagged,
            listed_flags(&store, &scope).await,
            flag_lines(&admin, &scope).await,
        ),
        (
            true,
            false,
            true,
            vec![Some((reason.to_owned(), source))],
            vec![None],
            json!([
                "1 1 1 current author_request 林舟 述。 文。 seed null null",
                format!(
                    "1 2 2 current author_request 林舟 述。 文。 conflict_flag {reason} chapter"
                ),
                "1 3 3 current author_request 林舟 述。 他会游泳。 author_request null null",
            ])
        )
    );
}

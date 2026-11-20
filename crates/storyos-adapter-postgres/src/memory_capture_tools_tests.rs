//! The read-only Tools and the reference items of a capture (ADR 0094 sections 4, 5, and 8). The
//! first request sends the Memory Index, the Chapter list, and the delete marks after the input.
//! The manuscript and Memory reads execute as in a Run of the author.

use serde_json::{Value, json};
use storyos_application::{ChapterId, DeleteChapterInput, ProjectScope};
use tokio_postgres::Client;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_structure::{DELETE_CHAPTER, delete_chapter};
use crate::shared_tests::command_sequence_support::issued;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::memory_capture::{run_capture, start};
use crate::shared_tests::memory_capture_loop::{record, script, settlement};
use crate::shared_tests::memory_capture_tools::conversation_source;
use crate::shared_tests::memory_entries::write_versions;

/// The source class and the content of each Context item of the first request of the Run.
async fn first_request_items(admin: &Client, run_id: &str) -> Value {
    json_of(
        admin,
        "SELECT ((payload #>> '{wire,serialized_payload}')::jsonb -> 'context')::text
               FROM storyos.model_attempts
              WHERE run_id = $1::text::uuid AND attempt_role = 'decision'
                AND decision_position = 0",
        &[&run_id],
    )
    .await
}

/// Gives the last version of the entry `number` the cause `kind`.
async fn set_cause(admin: &Client, scope: &ProjectScope, number: u64, kind: &str) {
    admin
        .execute(
            "UPDATE storyos.memory_entry_versions SET cause = jsonb_build_object('kind', $3::text)
              WHERE project_id = $1::text::uuid AND entry_number = $2::text::numeric
                AND version = (SELECT max(version) FROM storyos.memory_entry_versions
                                WHERE project_id = $1::text::uuid
                                  AND entry_number = $2::text::numeric)",
            &[&scope.project_id.as_ref(), &number.to_string(), &kind],
        )
        .await
        .unwrap();
}

/// The first request of a capture sends the Memory Index, the Chapter list, and the delete marks
/// after its input, with no Conversation Memory Settings. Only the deletes of the author are
/// marks, newest first. A Project with no entry sends no Index and no marks. A Project with no
/// live Chapter, no entry, and no mark sends only the input.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_first_request_of_a_capture_sends_its_reference_items() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, source) =
        crate::shared_tests::memory_capture::project(&store, &admin, "fe7").await;
    let input = "林舟在雨夜撑船。";
    let before = start(&store, &scope, &source, input).await.unwrap();
    run_capture(&store, &admin, &scope, &before.run_id).await;
    let versions = [
        (1, 1, 1, "current", "character", "林舟", "撑船的人。"),
        (2, 1, 2, "current", "other", "旧船", "一条旧船。"),
        (3, 1, 3, "current", "foreshadowing", "铜铃", "铜铃会响。"),
        (4, 1, 4, "current", "other", "雨伞", "一把伞。"),
        (2, 2, 5, "deleted", "other", "旧船", "一条旧船。"),
        (4, 2, 6, "deleted", "other", "雨伞", "一把伞。"),
        (3, 2, 7, "deleted", "foreshadowing", "铜铃", "铜铃会响。"),
    ];
    write_versions(&admin, &scope, &versions).await;
    set_cause(&admin, &scope, 2, "author_delete").await;
    set_cause(&admin, &scope, 3, "author_request").await;
    let after = start(&store, &scope, &source, input).await.unwrap();
    run_capture(&store, &admin, &scope, &after.run_id).await;
    let (empty, chapter_id, empty_source) =
        crate::shared_tests::memory_capture::project(&store, &admin, "fea").await;
    let remove = DeleteChapterInput {
        chapter_id: ChapterId::new(chapter_id.as_str()),
        expected_tree_revision: 3,
    };
    let remove = issued(&store, &empty, 0xfe98, &DELETE_CHAPTER, remove).await;
    delete_chapter(&store, &remove).await.unwrap();
    let only_input = start(&store, &empty, &empty_source, input).await.unwrap();
    run_capture(&store, &admin, &empty, &only_input.run_id).await;

    let chapters = json!({"source_class": "chapter_list",
                          "content": "{\"volume\":\"Volume A\"}\n[1,0,\"Chapter A\"]"});
    let input = json!({"source_class": "memory_capture_input", "content": input});
    assert_eq!(
        [
            first_request_items(&admin, &before.run_id).await,
            first_request_items(&admin, &after.run_id).await,
            first_request_items(&admin, &only_input.run_id).await,
        ],
        [
            json!([input, chapters]),
            json!([
                input,
                {"source_class": "memory_index", "content": concat!(
                    "{\"type\":\"character\"}\n",
                    "{\"description\":\"撑船的人。\",\"id\":1,\"title\":\"林舟\"}"
                )},
                chapters,
                {"source_class": "memory_delete_marks", "content": concat!(
                    "{\"description\":\"铜铃会响。\",\"title\":\"铜铃\",\"type\":\"foreshadowing\"}\n",
                    "{\"description\":\"一条旧船。\",\"title\":\"旧船\",\"type\":\"other\"}"
                )},
            ]),
            json!([input]),
        ]
    );
}

/// Each read-only Tool executes in a capture as in a Run of the author: a settled call, a
/// refused call, and a read of an entry that only another Project has. A refusal is a result, and
/// the capture continues. The reads use 7 RunSteps of the limit, and the output call of the last
/// RunStep records the observation. The read record names the capture, and the time of the last
/// read leaves it out.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_read_only_tools_of_a_capture_execute_as_for_the_author() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let project = crate::shared_tests::agent_run_read_chapter::project;
    let (scope, ..) = project(&store, &admin, "fe6", 0xfe90, &["Lin ran.", "Lin swam."]).await;
    let (other, ..) = project(&store, &admin, "fe8", 0xfe94, &["Wei ran."]).await;
    let entry = |number, title| {
        (
            number,
            1,
            number,
            "current",
            "character",
            title,
            "撑船的人。",
        )
    };
    write_versions(&admin, &scope, &[entry(1, "林舟")]).await;
    write_versions(&admin, &other, &[entry(1, "魏"), entry(2, "魏二")]).await;
    let chapter = |chapter| json!({"chapter": chapter, "first_block": null, "last_block": null});
    let item = json!({"type": "character", "title": "林舟", "text": "撑船的人。"});
    let calls = [
        ("search_manuscript", json!({"query": "Lin"})),
        ("search_manuscript", json!({"query": ""})),
        ("read_chapter", chapter(1)),
        ("read_chapter", chapter(9)),
        ("read_memory_entry", json!({"entry": 1})),
        // The entry 2 is an entry of the other Project only.
        ("read_memory_entry", json!({"entry": 2})),
        ("read_memory_entry", json!({"entry": 0})),
        record(json!([item])),
    ];
    let source = conversation_source(&admin, &scope).await;
    let started = start(&store, &scope, &source, &script(&calls))
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &started.run_id).await;

    let states: Vec<Value> =
        crate::shared_tests::agent_run_finish_rule::calls(&admin, &started.run_id)
            .await
            .iter()
            .map(|call| json!([call[0], call[1], call[2], call[3]]))
            .collect();
    let reads: Vec<String> = admin
        .query(
            "SELECT run_id::text FROM storyos.memory_entry_reads WHERE project_id = $1::text::uuid",
            &[&scope.project_id.as_ref()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(/*idx*/ 0))
        .collect();
    let last_reads = crate::memory_entries::last_memory_reads(&admin, &scope)
        .await
        .unwrap();
    assert_eq!(
        states,
        [
            json!([0, "search_manuscript", "settled", null]),
            json!([1, "search_manuscript", "refused", "intake_query"]),
            json!([2, "read_chapter", "settled", null]),
            json!([3, "read_chapter", "refused", "chapter_not_found"]),
            json!([4, "read_memory_entry", "settled", null]),
            json!([5, "read_memory_entry", "refused", "memory_entry_not_found"]),
            json!([6, "read_memory_entry", "refused", "memory_entry_not_found"]),
            json!([7, "record_memory_observations", "settled", null]),
        ]
    );
    let settled = settlement(&admin, &started.run_id).await;
    assert_eq!(
        json!([settled[0], settled[1], settled[2]]),
        json!(["completed", "settled", "observations_recorded"])
    );
    assert_eq!(
        (
            reads,
            last_reads
                .into_iter()
                .map(|read| read.2)
                .collect::<Vec<_>>()
        ),
        (vec![started.run_id.clone()], vec![None])
    );
}

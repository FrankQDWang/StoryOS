//! The two parts of a character file (ADR 0101 section 3): the part of the author that
//! `write_memory` writes beside the part of the background work, the refusal of a change of the
//! part of the background work, the field `"author_part": true` of the Memory Index of a Run of
//! the author and of a capture, and the class of a delete mark. The lead of these tests is `6f`.

use serde_json::{Value, json};
use storyos_core::MemoryWriteRefusal;

use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_write_memory::{first_index, refused, versions};
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::delete_memory_entry::delete;
use crate::shared_tests::memory_capture::{project, run_capture, start};
use crate::shared_tests::memory_capture_loop::{record, script};
use crate::shared_tests::memory_conflict_flag_show::{run, write};
use crate::shared_tests::memory_consolidation::{create, settled, update};
use crate::shared_tests::memory_consolidation_conflict::entry_id;
use crate::shared_tests::memory_consolidation_merge::{pass_on, seed};

fn settled_call(text: &str) -> Value {
    json!(["write_memory", "settled", null, text, true])
}

fn finish() -> Value {
    json!(["finish", "settled", null, "", true])
}

/// The arguments of a `create` of the character 林舟 with `text`.
fn character(text: &str) -> Value {
    json!({"type": "character", "title": "林舟", "description": "船夫。", "text": text})
}

/// A Project has the part of the background work of 林舟. A `create` of `write_memory` gives the
/// part of the author, and a second `create` is refused. A `change` of the part of the author is
/// applied, and a `change` of the part of the background work is refused and writes no version.
/// The next Run of the author and the next capture get the two parts in the Memory Index, and the
/// line of the part of the author has `"author_part": true`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn write_memory_writes_the_part_of_the_author_beside_the_part_of_the_background() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "6f0").await;
    let project = (&scope, chapter_id.as_str());
    seed(&admin, &scope, &[(1, "character", "chapter", "林舟")]).await;
    let message = [
        write("create", character("他会游泳。")),
        write("create", character("他会撑船。")),
        write("change", json!({"entry": 2, "text": "他怕水。"})),
        write("change", json!({"entry": 1, "text": "他怕水。"})),
    ]
    .join("\n");
    let writing = run(&store, &admin, project, "6f1", &message).await;
    let next = run(&store, &admin, project, "6f2", "Help.").await;
    let capture = start(&store, &scope, &source, &script(&[record(json!([]))]))
        .await
        .unwrap();
    run_capture(&store, &admin, &scope, &capture.run_id).await;

    assert_eq!(
        calls(&admin, &writing).await,
        json!([
            "completed",
            [
                settled_call("The Host created the Memory entry 2 with the title \"林舟\"."),
                refused(MemoryWriteRefusal::Exists(2)),
                settled_call("The Host changed the Memory entry 2 with the title \"林舟\"."),
                refused(MemoryWriteRefusal::BackgroundPart(1)),
                finish(),
            ]
        ])
    );
    assert_eq!(
        versions(&admin, &scope).await,
        [
            "1 1 1 current character chapter 林舟 述。 文。 seed",
            "2 1 2 current character author_request 林舟 船夫。 他会游泳。 author_request",
            "2 2 3 current character author_request 林舟 船夫。 他怕水。 author_request",
        ]
    );
    let index = json!([concat!(
        "{\"type\":\"character\"}\n",
        "{\"description\":\"述。\",\"id\":1,\"title\":\"林舟\"}\n",
        "{\"author_part\":true,\"description\":\"船夫。\",\"id\":2,\"title\":\"林舟\"}"
    )]);
    assert_eq!(
        (
            first_index(&admin, &next).await,
            first_index(&admin, &capture.run_id).await
        ),
        (index.clone(), index)
    );
}

/// A delete mark has the class of the entry that the author deleted. After a delete of the part
/// of the author of 林舟, a pass updates the part of the background work of 林舟. After a delete of
/// the part of the background work of 阿青, a `create` of a pass with that title is refused with
/// `deleted_by_author`, and a `create` of `write_memory` with that title is applied.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_delete_mark_stops_only_a_new_entry_of_its_own_class() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "6f3").await;
    let project = (&scope, chapter_id.as_str());
    let rows = [
        (1, "character", "chapter", "林舟"),
        (2, "character", "author_request", "林舟"),
        (3, "character", "chapter", "阿青"),
    ];
    seed(&admin, &scope, &rows).await;
    run(
        &store,
        &admin,
        project,
        "6f4",
        &write("delete", json!({"entry": 2})),
    )
    .await;
    let updated = pass_on(
        (&store, &admin),
        &scope,
        &source,
        &[update(1, "他游过了河。")],
    )
    .await;
    let id = entry_id(&admin, &scope, 3).await;
    assert!(delete(&store, &scope, 0x6f05, &id, 1).await.is_ok());
    let created = create("character", "阿青", json!([1]));
    let refused = pass_on((&store, &admin), &scope, &source, &[created]).await;
    let author = json!({"type": "character", "title": "阿青", "description": "述。",
                        "text": "她是渡口的船主。"});
    let writing = run(&store, &admin, project, "6f6", &write("create", author)).await;

    assert_eq!(
        (
            settled(&admin, &updated.run_id).await[3].clone(),
            settled(&admin, &refused.run_id).await[3].clone(),
            calls(&admin, &writing).await,
        ),
        (
            json!([["applied", null, 1]]),
            json!([["refused", "deleted_by_author", null]]),
            json!([
                "completed",
                [
                    settled_call("The Host created the Memory entry 4 with the title \"阿青\"."),
                    finish()
                ]
            ])
        )
    );
}

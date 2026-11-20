//! The conflict flag in a Run and its removal (ADR 0094 section 7): the Memory Index, the result
//! of `read_memory_entry`, and the list of the settings page with and without a flag, and the
//! removal by a change and by a delete of the author. Each test completes each Run. The lead of
//! these tests is `90`.

use serde_json::{Value, json};
use storyos_application::{MemoryCaptureSource, MemoryEntrySourceItem};
use storyos_core::{DeleteMemoryEntryConflict, TransitionOutcome};

use crate::shared_tests::agent_run_load_skill::calls;
use crate::shared_tests::agent_run_write_memory::first_index;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::delete_memory_entry::delete;
use crate::shared_tests::memory_capture::project;
use crate::shared_tests::memory_conflict_flag_show::{listed_flags, run, write};
use crate::shared_tests::memory_consolidation::{capture, delete as delete_operation, run_pass};
use crate::shared_tests::memory_consolidation_conflict::{conflict, entry_id};
use crate::shared_tests::memory_consolidation_merge::{flag_lines, pass_on, seed};

/// The items of one capture: one fact with no script, and the conflict items `conflicts`. The
/// fact makes the pass request have an observation.
fn with_fact(conflicts: &[(u64, &str)]) -> Value {
    let fact = json!({ "type": "other", "title": "桥", "text": "桥在渡口下游。" });
    let items = conflicts
        .iter()
        .map(|(entry, reason)| conflict(*entry, reason));
    Value::from_iter([fact].into_iter().chain(items))
}

fn conversation_of(source: &MemoryCaptureSource) -> MemoryEntrySourceItem {
    let MemoryCaptureSource::Conversation {
        conversation_id, ..
    } = source
    else {
        panic!("the source of the test is a Project Conversation");
    };
    MemoryEntrySourceItem::Conversation {
        conversation_id: conversation_id.clone(),
        available: true,
    }
}

/// The path of ADR 0094 section 7 on the fake destination. A capture of a Project Conversation
/// gives a conflict item, and the pass records the flag. A Run before the flag has the line of
/// S7-D1a, and the next Run has the flag in its Index and reads the reason and the source. A Run
/// with a change of the author removes the flag from the Index, the read, and the list.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_whole_path_flags_an_entry_of_the_author_and_a_change_removes_the_flag() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "900").await;
    let project = (&scope, chapter_id.as_str());
    seed(
        &admin,
        &scope,
        &[(1, "character", "author_request", "林舟")],
    )
    .await;
    let before = run(&store, &admin, project, "901", "Help.").await;
    let reason = "第二章说他会游泳。";
    capture(&store, &admin, &scope, &source, with_fact(&[(1, reason)])).await;
    run_pass(&store, &admin, &scope).await;
    let read = run(&store, &admin, project, "902", "SCRIPT:read_memory_entry 1").await;
    let flagged = listed_flags(&store, &scope).await;
    let change = write("change", json!({"entry": 1, "text": "他会游泳。"}));
    run(&store, &admin, project, "903", &change).await;
    let after = run(&store, &admin, project, "904", "SCRIPT:read_memory_entry 1").await;
    let result = |calls: Value| calls[1][0][3].as_str().unwrap().to_owned();
    let line = "{\"description\":\"述。\",\"id\":1,\"title\":\"林舟\"}";
    let index = |line: &str| json!([format!("{{\"type\":\"character\"}}\n{line}")]);

    assert_eq!(
        (
            first_index(&admin, &before).await,
            first_index(&admin, &read).await,
            first_index(&admin, &after).await,
        ),
        (
            index(line),
            index(&line.replace("{", "{\"conflict_flag\":true,")),
            index(line)
        )
    );
    let read = result(calls(&admin, &read).await);
    let after = result(calls(&admin, &after).await);
    assert_eq!(
        (
            read.ends_with(&format!(
                "\nconflict_flag: {reason}\nconflict_source: a Project Conversation\n\n文。"
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
            vec![Some((reason.to_owned(), conversation_of(&source)))],
            vec![None],
            json!([
                "1 1 1 current author_request 林舟 述。 文。 seed null null",
                format!(
                    "1 2 2 current author_request 林舟 述。 文。 conflict_flag {reason} conversation"
                ),
                "1 3 3 current author_request 林舟 述。 他会游泳。 author_request null null",
            ])
        )
    );
}

/// A change with the same text is the answer of the author that the entry is correct, and it
/// writes a version with no flag. A delete by `write_memory` and the command `deleteMemoryEntry`
/// write a deleted version with no flag. A delete with the version before the flag, which the
/// page showed, gives `entry_changed`.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_change_with_the_same_text_and_each_delete_remove_the_flag() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "905").await;
    let rows = [
        (1, "character", "author_request", "林舟"),
        (2, "other", "author_request", "渡口"),
        (3, "other", "author_request", "铜铃"),
    ];
    seed(&admin, &scope, &rows).await;
    let conflicts = with_fact(&[(1, "甲"), (2, "乙"), (3, "丙")]);
    capture(&store, &admin, &scope, &source, conflicts).await;
    run_pass(&store, &admin, &scope).await;
    let id = entry_id(&admin, &scope, 3).await;
    let stale = delete(&store, &scope, 0x90a1, &id, 1).await;
    let deleted = delete(&store, &scope, 0x90a2, &id, 2).await;
    let message = [
        write("change", json!({"entry": 1, "text": "文。"})),
        write("delete", json!({"entry": 2})),
    ]
    .join("\n");
    run(&store, &admin, (&scope, &chapter_id), "906", &message).await;

    assert_eq!(
        (
            matches!(
                stale,
                Ok(TransitionOutcome::Conflicted(
                    DeleteMemoryEntryConflict::EntryChanged
                ))
            ),
            matches!(deleted, Ok(TransitionOutcome::Applied(_))),
            flag_lines(&admin, &scope).await,
        ),
        (
            true,
            true,
            json!([
                "1 1 1 current author_request 林舟 述。 文。 seed null null",
                "1 2 4 current author_request 林舟 述。 文。 conflict_flag 甲 conversation",
                "1 3 8 current author_request 林舟 述。 文。 author_request null null",
                "2 1 2 current author_request 渡口 述。 文。 seed null null",
                "2 2 5 current author_request 渡口 述。 文。 conflict_flag 乙 conversation",
                "2 3 9 deleted author_request 渡口 述。  author_request null null",
                "3 1 3 current author_request 铜铃 述。 文。 seed null null",
                "3 2 6 current author_request 铜铃 述。 文。 conflict_flag 丙 conversation",
                "3 3 7 deleted author_request 铜铃 述。  author_delete null null",
            ])
        )
    );
}

/// A pass with no conflict does not change the flag: a background operation cannot remove it.
/// After a change of the author removes the flag, a later pass flags the entry again.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pass_with_no_conflict_keeps_the_flag_and_a_later_pass_flags_again() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, chapter_id, source) = project(&store, &admin, "907").await;
    let rows = [
        (1, "character", "author_request", "林舟"),
        (2, "other", "conversation", "渡口"),
    ];
    seed(&admin, &scope, &rows).await;
    capture(&store, &admin, &scope, &source, with_fact(&[(1, "甲")])).await;
    run_pass(&store, &admin, &scope).await;
    pass_on((&store, &admin), &scope, &source, &[delete_operation(2)]).await;
    let kept = listed_flags(&store, &scope).await;
    let change = write("change", json!({"entry": 1, "text": "他会游泳。"}));
    run(&store, &admin, (&scope, &chapter_id), "908", &change).await;
    capture(&store, &admin, &scope, &source, with_fact(&[(1, "乙")])).await;
    run_pass(&store, &admin, &scope).await;
    let flag = |reason: &str| Some((reason.to_owned(), conversation_of(&source)));

    assert_eq!(
        (
            kept,
            listed_flags(&store, &scope).await,
            flag_lines(&admin, &scope).await
        ),
        (
            vec![flag("甲")],
            vec![flag("乙")],
            json!([
                "1 1 1 current author_request 林舟 述。 文。 seed null null",
                "1 2 3 current author_request 林舟 述。 文。 conflict_flag 甲 conversation",
                "1 3 5 current author_request 林舟 述。 他会游泳。 author_request null null",
                "1 4 6 current author_request 林舟 述。 他会游泳。 conflict_flag 乙 conversation",
                "2 1 2 current conversation 渡口 述。 文。 seed null null",
                "2 2 4 deleted conversation 渡口 述。  consolidation null null",
            ])
        )
    );
}

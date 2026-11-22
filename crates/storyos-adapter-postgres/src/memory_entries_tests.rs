//! The records of Agent Memory (ADR 0094 section 3): the grants of the runtime role, the Scope
//! of a read, and the Project Export Archive.

use serde_json::json;
use storyos_application::ProjectScope;
use storyos_core::{MemoryEntryType, MemoryIndexEntry};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::seed_project;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::memory_entries::{archived, as_runtime, write_versions};

/// The grants of ADR 0094 section 3, and the Scope of a read: two Projects of one User each read
/// only their own entries, and a read in the Scope of one Project sees no row of the other.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_cannot_change_a_version_and_each_project_reads_only_its_entries() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let first = seed_project(&store, "c510").await;
    let second = seed_project(&store, "c520").await;
    let versions = [(1, 1, 1, "current", "character", "林舟", "船夫。")];
    write_versions(&admin, &first, &versions).await;
    let versions = [(1, 1, 1, "current", "other", "渡口", "旧渡口。")];
    write_versions(&admin, &second, &versions).await;
    let index = async |scope: &ProjectScope| {
        let entries = crate::memory_entries::read_memory_index(
            &admin,
            scope,
            1,
            storyos_core::AgentRunKind::Author,
        );
        let entries = entries.await;
        entries.unwrap()
    };
    let entry = |entry_type, title: &str, description: &str| MemoryIndexEntry {
        number: 1,
        entry_type,
        title: title.to_owned(),
        description: description.to_owned(),
        flagged: false,
        author_part: entry_type == MemoryEntryType::Character,
        intent: None,
    };
    let statements = [
        "SELECT 1 FROM storyos.memory_entry_versions
          WHERE project_id <> current_setting('storyos.project_id')::uuid",
        "SELECT 1 FROM storyos.memory_entry_versions",
        "UPDATE storyos.memory_entry_versions SET title = 'x'",
        "DELETE FROM storyos.memory_entry_versions",
        "DELETE FROM storyos.project_memory_heads",
    ];
    let refused = || Err("42501".to_owned());

    assert_eq!(
        (
            index(&first).await,
            index(&second).await,
            as_runtime(&first, &statements).await
        ),
        (
            vec![entry(MemoryEntryType::Character, "林舟", "船夫。")],
            vec![entry(MemoryEntryType::Other, "渡口", "旧渡口。")],
            vec![Ok(0), Ok(1), refused(), refused(), refused()],
        )
    );
}

/// The archive of a Project with entries has each version row, the deleted versions included,
/// and the head of the Project.
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_archive_of_a_project_has_each_version_and_its_memory_head() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let scope = seed_project(&store, "c530").await;
    let versions = [
        (1, 1, 1, "current", "foreshadowing", "铃", "铃声。"),
        (2, 1, 2, "current", "character", "阿青", "妹妹。"),
        (1, 2, 3, "deleted", "foreshadowing", "铃", "铃声。"),
    ];
    write_versions(&admin, &scope, &versions).await;
    let (versions, heads) = archived(&store, &scope, "cf").await;

    assert_eq!(
        (versions, heads),
        (
            vec![
                json!([1, 1, 1, "current"]),
                json!([1, 2, 3, "deleted"]),
                json!([2, 1, 2, "current"]),
            ],
            vec![json!([3, 3])],
        )
    );
}

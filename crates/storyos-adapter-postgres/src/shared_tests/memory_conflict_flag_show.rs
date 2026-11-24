//! The shared test support from `memory_conflict_flag_show_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::agent_run_finish_rule::complete;
use crate::shared_tests::chapter_list_context::admitted;
use serde_json::Value;
use storyos_application::{
    ConversationSelection, MemoryEntryFlagItem, MemoryEntrySourceItem, ProjectScope,
    list_memory_entries,
};

/// The fake script line of one `write_memory` call.
pub(crate) fn write(action: &str, arguments: Value) -> String {
    format!("SCRIPT:write_memory {action} {arguments}")
}

/// Admits a Run of the author with `message` in a new Conversation and completes it.
pub(crate) async fn run(
    store: &PostgresProjectReader,
    admin: &crate::statement_cache::Client,
    (scope, chapter_id): (&ProjectScope, &str),
    prefix: &str,
    message: &str,
) -> String {
    let new = ConversationSelection::New;
    let run_id = admitted(store, scope, chapter_id, prefix, message, new).await;
    complete(store, admin, scope, &run_id).await;
    run_id
}

/// The flag of each entry in the list of the settings page, in the order of the list.
pub(crate) async fn listed_flags(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
) -> Vec<Option<(String, MemoryEntrySourceItem)>> {
    let entries = list_memory_entries(store, scope).await.unwrap().unwrap();
    let flag = |flag: MemoryEntryFlagItem| {
        assert!(flag.flagged_at.ends_with('Z'));
        (flag.reason, flag.source)
    };
    entries
        .into_iter()
        .map(|entry| entry.flag.map(flag))
        .collect()
}

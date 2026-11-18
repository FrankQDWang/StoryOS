//! The rows of the Delete Memory Entry command in the table-driven contract suite (ADR 0094
//! section 6). The lead of these tests is `ad`.

use storyos_application::{DeleteMemoryEntryInput, ProjectScope};
use storyos_contracts::CommandKind;
use tokio_postgres::Client;

use crate::PostgresProjectReader;

use super::support::{CommandCall, Route, issued, seed_project, stores};

pub(crate) const DELETE_MEMORY_ENTRY: Route = Route {
    kind: CommandKind::DeleteMemoryEntry,
    method: storyos_contracts::DELETE_MEMORY_ENTRY_METHOD,
    path: storyos_contracts::DELETE_MEMORY_ENTRY_PATH,
    schema: storyos_contracts::DELETE_MEMORY_ENTRY_REQUEST_SCHEMA_ID,
};

/// Writes the entry 1 of the Project of `scope` with SQL as version 1 at the Memory revision 1,
/// and moves the head. Gives the identity of the entry. The values are test constants.
pub(super) async fn seed_memory_entry(admin: &Client, scope: &ProjectScope) -> String {
    let (owner, project) = (scope.owner_user_id.as_ref(), scope.project_id.as_ref());
    admin
        .execute(
            "INSERT INTO storyos.project_memory_heads VALUES ($1::text::uuid, $2::text::uuid, 1, 2)",
            &[&owner, &project],
        )
        .await
        .unwrap();
    admin
        .query_one(
            "INSERT INTO storyos.memory_entry_versions
                 (owner_user_id, project_id, memory_entry_id, entry_number, version,
                  memory_revision, state, entry_type, origin, title, description, text, cause,
                  source_references)
             VALUES ($1::text::uuid, $2::text::uuid, md5($2 || ':1')::uuid, 1, 1, 1, 'current',
                     'character', 'author_request', '阿青', '渡口的船夫。', '阿青在渡口撑船。',
                     '{\"kind\": \"author_request\"}', '[]')
             RETURNING memory_entry_id::text",
            &[&owner, &project],
        )
        .await
        .unwrap()
        .get(0)
}

/// One applicable delete of the only entry of a new Project.
pub(super) async fn delete_memory_entry_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<DeleteMemoryEntryInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let (_, admin) = stores().await;
    let input = DeleteMemoryEntryInput {
        memory_entry_id: seed_memory_entry(&admin, &scope).await,
        expected_version: 1,
    };
    issued(store, &scope, base + 9, &DELETE_MEMORY_ENTRY, input).await
}

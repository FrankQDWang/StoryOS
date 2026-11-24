//! The shared test support from `grant_scope_tests.rs`.

use crate::statement_cache::Client;
use storyos_application::{ProjectScope, open_project_assistance};

/// The current grant of the Project and its recorded scope.
pub(crate) async fn current_grant(
    store: &crate::PostgresProjectReader,
    admin: &Client,
    scope: &ProjectScope,
) -> (String, Option<String>) {
    let grant_id = open_project_assistance(store, scope)
        .await
        .unwrap()
        .unwrap()
        .grant_id;
    let grant_scope = admin
        .query_one(
            "SELECT grant_scope FROM storyos.project_destination_grants
              WHERE grant_id = $1::text::uuid",
            &[&grant_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    (grant_id, grant_scope)
}

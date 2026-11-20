use storyos_core::{
    ContractDifference, EXA_TOOL_REGISTRATION, ToolRegistration, ToolRegistrationLifecycle,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::tests::stores;

/// Inserts the revision and its tools in one transaction, as the binding of a research offer does.
async fn insert(client: &Client, registration: &ToolRegistration) {
    client.batch_execute("BEGIN").await.unwrap();
    super::insert_registration_rows(client, registration)
        .await
        .unwrap();
    client.batch_execute("COMMIT").await.unwrap();
}

/// The lifecycle of a stored revision. `None` is an unknown revision.
async fn lifecycle(client: &Client, revision: &str) -> Option<ToolRegistrationLifecycle> {
    client
        .query_opt(
            "SELECT lifecycle FROM storyos.tool_registration_revisions
              WHERE tool_registration_revision = $1::text::uuid",
            &[&revision],
        )
        .await
        .unwrap()
        .map(|row| ToolRegistrationLifecycle::parse(row.get(/*idx*/ 0)).unwrap())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_never_reactivates_edits_or_extends_a_registration_revision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, _admin) = stores().await;
    const REVISION: ToolRegistration = ToolRegistration {
        revision: "018f0000-0000-7001-8000-0000000e0f04",
        ..EXA_TOOL_REGISTRATION
    };
    const ACTIVE: ToolRegistration = ToolRegistration {
        revision: "018f0000-0000-7001-8000-0000000e0f05",
        ..EXA_TOOL_REGISTRATION
    };
    let client = store.connect().await.unwrap();
    insert(&client, &REVISION).await;
    insert(&client, &ACTIVE).await;
    let drift = [ContractDifference::ProtocolRevision {
        pinned: "2025-11-25".to_owned(),
        observed: "2026-06-18".to_owned(),
    }];
    // The quarantine of the Tool Gateway on Tool Contract Drift.
    client
        .execute(
            "UPDATE storyos.tool_registration_revisions
                SET lifecycle = 'quarantined', quarantine_differences = $2::text::jsonb,
                    quarantined_at = clock_timestamp()
              WHERE tool_registration_revision = $1::text::uuid AND lifecycle = 'active'",
            &[&REVISION.revision, &serde_json::to_string(&drift).unwrap()],
        )
        .await
        .unwrap();

    let reactivated = client
        .execute(
            "UPDATE storyos.tool_registration_revisions
                SET lifecycle = 'active', quarantine_differences = NULL, quarantined_at = NULL
              WHERE tool_registration_revision = $1::text::uuid",
            &[&REVISION.revision],
        )
        .await
        .unwrap();
    let identity_edit = client
        .execute(
            "UPDATE storyos.tool_registration_revisions SET server_name = 'other-server'
              WHERE tool_registration_revision = $1::text::uuid",
            &[&REVISION.revision],
        )
        .await
        .map_err(|error| crate::sql_state(&error));

    let appended_tool = client
        .execute(
            "INSERT INTO storyos.tool_registration_tools
               (tool_registration_revision, tool_name, tool_spec_revision, input_schema_digest,
                input_contract_digest, provenance_digest)
             SELECT tool_registration_revision, 'agent_run', tool_spec_revision,
                    input_schema_digest, input_contract_digest, provenance_digest
               FROM storyos.tool_registration_tools
              WHERE tool_registration_revision = $1::text::uuid AND tool_name = 'web_fetch_exa'",
            &[&ACTIVE.revision],
        )
        .await
        .map_err(|error| crate::sql_state(&error));

    assert_eq!(appended_tool, Err(Some("42501".to_owned())));
    assert_eq!(reactivated, 0);
    assert_eq!(identity_edit, Err(Some("42501".to_owned())));
    assert_eq!(
        lifecycle(&client, REVISION.revision).await,
        Some(ToolRegistrationLifecycle::Quarantined)
    );
}

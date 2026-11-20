use serde_json::{Value, json};
use storyos_core::{
    ContractDifference, EXA_TOOL_REGISTRATION, ToolCallRefusal, ToolRegistration,
    ToolRegistrationLifecycle,
};

use super::quarantine_revision;
use crate::PostgresProjectReader;
use crate::agent_run_dispatch::tests::stores;

const PINNED: ToolRegistration = ToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0f01",
    ..EXA_TOOL_REGISTRATION
};
const SUCCESSOR: ToolRegistration = ToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0f02",
    ..EXA_TOOL_REGISTRATION
};

/// Inserts the revision and its tools in one transaction, as the binding of a research offer does.
async fn insert(store: &PostgresProjectReader, registration: &ToolRegistration) {
    let client = store.connect().await.unwrap();
    client.batch_execute("BEGIN").await.unwrap();
    super::insert_registration_rows(&client, registration)
        .await
        .unwrap();
    client.batch_execute("COMMIT").await.unwrap();
}

/// The quarantine of the Tool Gateway on Tool Contract Drift. The result is the number of changed
/// rows.
async fn quarantine(
    store: &PostgresProjectReader,
    revision: &str,
    differences: &[ContractDifference],
) -> u64 {
    let client = store.connect().await.unwrap();
    let differences = serde_json::to_value(differences).unwrap();
    quarantine_revision(&client, revision, &differences)
        .await
        .unwrap()
}

/// The lifecycle of a stored revision. `None` is an unknown revision.
async fn lifecycle(
    store: &PostgresProjectReader,
    revision: &str,
) -> Option<ToolRegistrationLifecycle> {
    store
        .connect()
        .await
        .unwrap()
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
async fn drift_quarantines_one_revision_once_and_a_new_revision_carries_no_authority_from_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    insert(&store, &PINNED).await;
    insert(&store, &PINNED).await;
    let tools: Vec<(String, String)> = admin
        .query(
            "SELECT tool_name, input_schema_digest FROM storyos.tool_registration_tools
              WHERE tool_registration_revision = $1::text::uuid ORDER BY tool_name",
            &[&PINNED.revision],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    let drift = [ContractDifference::ToolRemoved {
        tool: "web_fetch_exa".to_owned(),
    }];
    let later = [ContractDifference::ServerName {
        pinned: "exa-search-server".to_owned(),
        observed: "other-server".to_owned(),
    }];

    let first = quarantine(&store, PINNED.revision, &drift).await;
    let second = quarantine(&store, PINNED.revision, &later).await;
    insert(&store, &SUCCESSOR).await;
    let pinned = lifecycle(&store, PINNED.revision).await.unwrap();
    let record: String = admin
        .query_one(
            "SELECT quarantine_differences::text FROM storyos.tool_registration_revisions
              WHERE tool_registration_revision = $1::text::uuid",
            &[&PINNED.revision],
        )
        .await
        .unwrap()
        .get(0);

    assert_eq!(
        tools,
        [
            (
                "web_fetch_exa".to_owned(),
                EXA_TOOL_REGISTRATION.tools[1]
                    .input_schema_digest
                    .to_owned()
            ),
            (
                "web_search_exa".to_owned(),
                EXA_TOOL_REGISTRATION.tools[0]
                    .input_schema_digest
                    .to_owned()
            ),
        ]
    );
    // The first drift quarantines the revision. A later drift does not change it.
    assert_eq!((first, second), (1, 0));
    assert_eq!(
        serde_json::from_str::<Value>(&record).unwrap(),
        json!([{"kind": "tool_removed", "tool": "web_fetch_exa"}])
    );
    assert_eq!(
        pinned.admit_new_tool_call().map_err(ToolCallRefusal::code),
        Err("tool_registration_unavailable")
    );
    assert_eq!(
        lifecycle(&store, SUCCESSOR.revision).await,
        Some(ToolRegistrationLifecycle::Active)
    );
    assert_eq!(
        quarantine(&store, "018f0000-0000-7001-8000-0000000e0f03", &drift).await,
        0
    );
    assert_eq!(
        lifecycle(&store, "018f0000-0000-7001-8000-0000000e0f03").await,
        None
    );
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
    insert(&store, &REVISION).await;
    insert(&store, &ACTIVE).await;
    let drift = [ContractDifference::ProtocolRevision {
        pinned: "2025-11-25".to_owned(),
        observed: "2026-06-18".to_owned(),
    }];
    quarantine(&store, REVISION.revision, &drift).await;
    let client = store.connect().await.unwrap();

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
        lifecycle(&store, REVISION.revision).await,
        Some(ToolRegistrationLifecycle::Quarantined)
    );
}

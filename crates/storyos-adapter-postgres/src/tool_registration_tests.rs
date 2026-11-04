use serde_json::{Value, json};
use storyos_core::{
    ContractDifference, EXA_TOOL_REGISTRATION, ToolCallRefusal, ToolRegistration,
    ToolRegistrationLifecycle,
};

use super::QuarantineOutcome;
use crate::agent_run_dispatch::tests::stores;

const PINNED: ToolRegistration = ToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0f01",
    ..EXA_TOOL_REGISTRATION
};
const SUCCESSOR: ToolRegistration = ToolRegistration {
    revision: "018f0000-0000-7001-8000-0000000e0f02",
    ..EXA_TOOL_REGISTRATION
};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn drift_quarantines_one_revision_once_and_a_new_revision_carries_no_authority_from_it() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    store.insert_tool_registration(&PINNED).await.unwrap();
    store.insert_tool_registration(&PINNED).await.unwrap();
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

    let first = store
        .quarantine_tool_registration(PINNED.revision, &drift)
        .await
        .unwrap();
    let second = store
        .quarantine_tool_registration(PINNED.revision, &later)
        .await
        .unwrap();
    store.insert_tool_registration(&SUCCESSOR).await.unwrap();
    let lifecycle = |revision| store.tool_registration_lifecycle(revision);
    let pinned = lifecycle(PINNED.revision).await.unwrap().unwrap();
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
                EXA_TOOL_REGISTRATION.tools[1].input_schema_digest.to_owned()
            ),
            (
                "web_search_exa".to_owned(),
                EXA_TOOL_REGISTRATION.tools[0].input_schema_digest.to_owned()
            ),
        ]
    );
    assert_eq!(
        (first, second),
        (
            QuarantineOutcome::Quarantined,
            QuarantineOutcome::AlreadyUnavailable
        )
    );
    assert_eq!(
        serde_json::from_str::<Value>(&record).unwrap(),
        json!([{"kind": "tool_removed", "tool": "web_fetch_exa"}])
    );
    assert_eq!(
        pinned.admit_new_tool_call().map_err(ToolCallRefusal::code),
        Err("tool_registration_unavailable")
    );
    assert_eq!(
        lifecycle(SUCCESSOR.revision).await.unwrap(),
        Some(ToolRegistrationLifecycle::Active)
    );
    assert_eq!(
        store
            .quarantine_tool_registration("018f0000-0000-7001-8000-0000000e0f03", &drift)
            .await
            .unwrap(),
        QuarantineOutcome::UnknownRevision
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_runtime_role_never_reactivates_or_edits_a_registration_revision() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, _admin) = stores().await;
    const REVISION: ToolRegistration = ToolRegistration {
        revision: "018f0000-0000-7001-8000-0000000e0f04",
        ..EXA_TOOL_REGISTRATION
    };
    store.insert_tool_registration(&REVISION).await.unwrap();
    let drift = [ContractDifference::ProtocolRevision {
        pinned: "2025-11-25".to_owned(),
        observed: "2026-06-18".to_owned(),
    }];
    store
        .quarantine_tool_registration(REVISION.revision, &drift)
        .await
        .unwrap();
    let client = store.pool.checkout().await.unwrap();

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

    assert_eq!(reactivated, 0);
    assert_eq!(identity_edit, Err(Some("42501".to_owned())));
    assert_eq!(
        store.tool_registration_lifecycle(REVISION.revision).await.unwrap(),
        Some(ToolRegistrationLifecycle::Quarantined)
    );
}

use crate::statement_cache::Client;
use storyos_application::{
    DispatchClaim, ModelDispatchStore, ReferenceRetrieval, ReportedBinding, ResponseReference,
};
use storyos_core::{
    AGENT_PLAN_REGISTRATION, DeploymentDestination, NativeStreamItem, RetrievalBounds,
    StreamItemRole, StreamItemState,
};

use crate::agent_run_observation::encode_items;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_support::stores;
use crate::shared_tests::json_row::json_of;
use crate::shared_tests::model_gateway::{claim_run, queued_run};
use crate::shared_tests::stream_commit_contention::dispatched;

fn item(item_id: &str, text: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: item_id.to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Provisional,
        text: Some(text.to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

async fn stored_payload(admin: &Client, dispatch: &DispatchClaim) -> serde_json::Value {
    json_of(
        admin,
        "SELECT payload::text FROM storyos.model_attempts
              WHERE model_attempt_id = $1::text::uuid",
        &[&dispatch.model_attempt_id],
    )
    .await
}

/// Each stream commit changes only the items and the optional reference of the stored Attempt
/// payload. An absent, null, or non-array item value merges as an empty array (#1247).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stream_commit_keeps_the_attempt_payload_outside_its_items_and_reference() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) =
        queued_run(&store, "9a0", DeploymentDestination::HostFakeToolLoop).await;
    let claim = claim_run(&admin, scope, run_id).await;
    let dispatch = dispatched(&store, &claim).await;
    let reference = ResponseReference {
        reference_id: "resp_9a0".to_owned(),
        retrieval: ReferenceRetrieval::Supported {
            bounds: RetrievalBounds::DeclaredReadOnly,
        },
        reported_binding: ReportedBinding {
            owner_user_id: None,
            conversation_id: None,
            destination_identity: None,
            mapping_revision: AGENT_PLAN_REGISTRATION.mapping_revision.to_owned(),
        },
    };
    let mut expected = stored_payload(&admin, &dispatch).await;
    let batches = [
        (vec![item("1", "Guard"), item("2", "Rail")], None),
        (vec![item("1", "Guard rail")], None),
        (Vec::new(), None),
        (Vec::new(), Some(&reference)),
    ];
    let merged = [
        vec![item("1", "Guard"), item("2", "Rail")],
        vec![item("1", "Guard rail"), item("2", "Rail")],
        vec![item("1", "Guard rail"), item("2", "Rail")],
        vec![item("1", "Guard rail"), item("2", "Rail")],
    ];
    for ((events, reference), items) in batches.iter().zip(&merged) {
        let appended = store
            .append_model_stream_events(&claim, &dispatch, events, *reference)
            .await;

        assert_eq!(appended.unwrap(), None);
        expected["items"] = encode_items(items);
        if let Some(reference) = reference {
            expected["retained_reference"] =
                crate::agent_run_recovery::encode_retained_reference(reference);
            expected["response_reference"] = reference.reference_id.clone().into();
        }
        assert_eq!(stored_payload(&admin, &dispatch).await, expected);
    }

    for damage in [
        "payload - 'items'",
        "jsonb_set(payload, '{items}', 'null')",
        "jsonb_set(payload, '{items}', '{\"item_id\": \"1\"}')",
    ] {
        admin
            .execute(
                &format!(
                    "UPDATE storyos.model_attempts SET payload = {damage}
                      WHERE model_attempt_id = $1::text::uuid"
                ),
                &[&dispatch.model_attempt_id],
            )
            .await
            .unwrap();
        let events = [item("3", "Post")];

        let appended = store
            .append_model_stream_events(&claim, &dispatch, &events, /*reference*/ None)
            .await;

        assert_eq!(appended.unwrap(), None);
        expected["items"] = encode_items(&events);
        assert_eq!(stored_payload(&admin, &dispatch).await, expected);
    }
}

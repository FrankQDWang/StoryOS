use storyos_application::{
    DispatchClaim, ModelDispatchStore, ReferenceRetrieval, ReportedBinding, ResponseReference,
};
use storyos_core::{
    AGENT_PLAN_REGISTRATION, DeploymentDestination, NativeStreamItem, RetrievalBounds,
    StreamItemRole, StreamItemState,
};
use tokio_postgres::Client;

use crate::agent_run_dispatch::stream_commit_contention_tests::dispatched;
use crate::agent_run_dispatch::tests::{claim_run, queued_run, stores};
use crate::agent_run_observation::encode_items;

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
        hosted_report: None,
    }
}

async fn stored_payload(admin: &Client, dispatch: &DispatchClaim) -> serde_json::Value {
    let payload: String = admin
        .query_one(
            "SELECT payload::text FROM storyos.model_attempts
              WHERE model_attempt_id = $1::text::uuid",
            &[&dispatch.model_attempt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    serde_json::from_str(&payload).unwrap()
}

/// Each stream commit changes only the items and the optional reference of the stored Attempt
/// payload. An absent, null, or non-array item value merges as an empty array (#1247).
#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_stream_commit_keeps_the_attempt_payload_outside_its_items_and_reference() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, _, run_id) = queued_run(&store, "9a0", DeploymentDestination::HostFake).await;
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

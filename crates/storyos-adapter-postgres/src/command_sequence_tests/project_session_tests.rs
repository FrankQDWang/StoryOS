use storyos_application::ProjectCommandError;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_damaged_evidence::ReplayError;
use crate::shared_tests::command_sequence_project_session::{
    take_over_project_writer, take_over_project_writer_call, update_project_assistance_call,
};
use crate::shared_tests::command_sequence_support::{stores, with_new_request_ids};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_command_without_a_response_record_replays_without_it_and_damaged_activity_is_a_fault() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = take_over_project_writer_call(&store, /*base*/ 0x6aa0).await;
    let first = take_over_project_writer(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(payload, '{{prior_writer_generation}}', '\"damaged\"')
              WHERE receipt_id = '{}'",
            first.ids.receipt_id
        ))
        .await
        .unwrap();
    let damaged = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:takeOverProjectWriter:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert!(matches!(damaged, Err(ProjectCommandError::Unavailable(_))));
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_assistance_replay_separates_a_project_only_record_from_damaged_assistance() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let call = update_project_assistance_call(&store, /*base*/ 0x6ab0).await;
    store
        .update_project_assistance(&call.envelope, &call.input)
        .await
        .unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    let mut errors = Vec::new();
    for evidence in [
        "acknowledgement_format = 'command_response_project.v1', response_assistance = NULL",
        "acknowledgement_format = 'command_response_project_assistance.v1',
         response_assistance = '{}'::jsonb",
    ] {
        admin
            .batch_execute(&format!(
                "UPDATE storyos.command_idempotency SET {evidence}
                  WHERE idempotency_key = '{key}'"
            ))
            .await
            .unwrap();
        let retry = with_new_request_ids(&call);
        errors.push(
            match store
                .update_project_assistance(&retry.envelope, &retry.input)
                .await
            {
                Err(ProjectCommandError::HistoricalAcknowledgementUnavailable) => {
                    ReplayError::HistoricalAcknowledgementUnavailable
                }
                Err(ProjectCommandError::Unavailable(_)) => ReplayError::Unavailable,
                other => panic!("the replay must fail on assistance evidence, got {other:?}"),
            },
        );
    }
    assert_eq!(
        errors,
        vec![
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ]
    );
}

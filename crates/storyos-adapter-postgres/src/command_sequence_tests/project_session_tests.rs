use storyos_application::ProjectCommandError;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_damaged_evidence::ReplayError;
use crate::shared_tests::command_sequence_project_session::update_project_assistance_call;
use crate::shared_tests::command_sequence_support::{stores, with_new_request_ids};

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

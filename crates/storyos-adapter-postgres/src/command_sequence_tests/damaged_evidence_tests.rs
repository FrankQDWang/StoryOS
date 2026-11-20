use storyos_core::ReceiptResult;
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::accept_proposal_call;
use crate::shared_tests::command_sequence_agent_run::{
    cancel_agent_run_call, create_agent_run_call, park_run, pause_agent_run_call,
};
use crate::shared_tests::command_sequence_damaged_evidence::{
    ReplayError, evidence_replays, replay_past_checks,
};
use crate::shared_tests::command_sequence_memory_entry::delete_memory_entry_call;
use crate::shared_tests::command_sequence_project_session::{
    update_project_assistance_call, update_project_call, update_project_instruction_call,
};
use crate::shared_tests::command_sequence_proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use crate::shared_tests::command_sequence_steer_agent_run::steer_agent_run_call;
use crate::shared_tests::command_sequence_structure::{
    create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    set_current_chapter_call, update_chapter_call, update_volume_call,
};
use crate::shared_tests::command_sequence_support::{CommandCall, SequenceError, stores};

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_replay_separates_pre_capture_from_damaged_evidence() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let create = create_agent_run_call(&store, /*base*/ 0xb350).await;
    let observed = vec![
        evidence_replays(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x6a00).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x6a10).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x6a20).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x6a30).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x6a40).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x6a50).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x6a60).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x6a80).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x6a90).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_project_instruction_call(&store, /*base*/ 0xef30).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &delete_memory_entry_call(&store, /*base*/ 0xad30).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x6ad0).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b50).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7280).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7290).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7490).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &pause_agent_run_call(&store, &admin, /*base*/ 0xb240).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &cancel_agent_run_call(&store, &admin, /*base*/ 0xd540).await,
        )
        .await,
        evidence_replays(&store, &admin, &create).await,
        evidence_replays(
            &store,
            &admin,
            &steer_agent_run_call(&store, &admin, /*base*/ 0xc740).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &accept_proposal_call(&store, &admin, /*base*/ 0xa850).await,
        )
        .await,
    ];
    park_run(&admin, &create).await;
    let separated = |result| {
        (
            result,
            [
                ReplayError::HistoricalAcknowledgementUnavailable,
                ReplayError::Unavailable,
            ],
        )
    };
    let mut expected = vec![separated(ReceiptResult::AuthoritativeApplied); 19];
    expected.push(separated(ReceiptResult::NoEffect));
    expected.push(separated(ReceiptResult::AuthoritativeApplied));
    assert_eq!(observed, expected);
}

/// Settles the call and replays it once with its Receipt payload set to `changed`, an SQL
/// expression over `result_payload`.
pub(super) async fn replay_with_receipt_payload<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    changed: &str,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let receipt_id = &call.envelope.ids.receipt_id;
    let payload: String = admin
        .query_one(
            "SELECT result_payload::text FROM storyos.domain_receipts
              WHERE receipt_id = $1::text::uuid",
            &[receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    replay_past_checks(
        admin,
        store,
        call,
        "domain_receipts",
        "result_payload",
        &format!(
            "UPDATE storyos.domain_receipts SET result_payload = {changed}
              WHERE receipt_id = '{receipt_id}'"
        ),
        &format!(
            "UPDATE storyos.domain_receipts SET result_payload = '{payload}'::jsonb
              WHERE receipt_id = '{receipt_id}'"
        ),
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_replay_requires_the_payload_field_of_its_receipt() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        replay_with_receipt_payload(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x9c00).await,
            "result_payload - 'rejection_reason'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x9c10).await,
            "result_payload - 'transition'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x9c20).await,
            "result_payload - 'transition'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x9c30).await,
            "result_payload - 'transition'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x9c40).await,
            "result_payload - 'transition'",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 5]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_receipt_replay_refuses_a_null_or_zero_historical_order_and_a_zero_shape_value() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        replay_with_receipt_payload(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0xa200).await,
            "jsonb_set(result_payload, '{order}', 'null'::jsonb)",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0xa210).await,
            "jsonb_set(result_payload, '{order}', '\"0\"'::jsonb)",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 2]);
}

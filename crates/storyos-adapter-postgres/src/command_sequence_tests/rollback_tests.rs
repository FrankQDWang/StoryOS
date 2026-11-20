use storyos_core::ReceiptResult;
use uuid::Uuid;

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    accept_proposal_call, invalid_acceptance_call,
};
use crate::shared_tests::command_sequence_agent_run::{
    cancel_agent_run_call, create_agent_run_call, park_run, pause_agent_run_call,
};
use crate::shared_tests::command_sequence_memory_entry::delete_memory_entry_call;
use crate::shared_tests::command_sequence_project_session::{
    take_over_project_writer_call, update_project_assistance_call, update_project_call,
    update_project_instruction_call,
};
use crate::shared_tests::command_sequence_proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use crate::shared_tests::command_sequence_rollback::failed_then_settled;
use crate::shared_tests::command_sequence_steer_agent_run::steer_agent_run_call;
use crate::shared_tests::command_sequence_structure::{
    create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    set_current_chapter_call, update_chapter_call, update_volume_call,
};
use crate::shared_tests::command_sequence_support::stores;

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_failing_step_rolls_back_every_row_and_keeps_the_challenge_unused() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let create = create_agent_run_call(&store, /*base*/ 0xb340).await;
    let observed = vec![
        failed_then_settled(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x5f00).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x5f10).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x5f20).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x5f30).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x5f40).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x5f50).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x5f60).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x5f80).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x5f90).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_instruction_call(&store, /*base*/ 0xef10).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_memory_entry_call(&store, /*base*/ 0xad10).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &take_over_project_writer_call(&store, /*base*/ 0x5fa0).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x5fb0).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b30).await,
        )
        .await,
        failed_then_settled(&store, &admin, &{
            let mut stale = reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b40).await;
            stale.input.proposal_revision_id = Uuid::now_v7().to_string();
            stale
        })
        .await,
        failed_then_settled(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7260).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7270).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7480).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &pause_agent_run_call(&store, &admin, /*base*/ 0xb230).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &cancel_agent_run_call(&store, &admin, /*base*/ 0xd530).await,
        )
        .await,
        failed_then_settled(&store, &admin, &create).await,
        failed_then_settled(
            &store,
            &admin,
            &steer_agent_run_call(&store, &admin, /*base*/ 0xc730).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &accept_proposal_call(&store, &admin, /*base*/ 0xa830).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &invalid_acceptance_call(&store, &admin, /*base*/ 0xa840).await,
        )
        .await,
    ];
    park_run(&admin, &create).await;
    let rolled_back = |result| {
        let after_classify = match result {
            ReceiptResult::AuthoritativeApplied => ["apply", "after authority"],
            ReceiptResult::NoEffect | ReceiptResult::Conflicted | ReceiptResult::Refused => {
                ["zero-authority write"; 2]
            }
        };
        let failures = std::iter::once("classify")
            .chain(after_classify)
            .map(|step| (Some(step), [0; 5]))
            .collect::<Vec<_>>();
        (failures, result)
    };
    let mut expected = vec![rolled_back(ReceiptResult::AuthoritativeApplied); 11];
    expected.push(rolled_back(ReceiptResult::NoEffect));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 2]);
    expected.push(rolled_back(ReceiptResult::Refused));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 6]);
    expected.push(rolled_back(ReceiptResult::NoEffect));
    expected.push(rolled_back(ReceiptResult::AuthoritativeApplied));
    expected.push(rolled_back(ReceiptResult::NoEffect));
    assert_eq!(observed, expected);
}

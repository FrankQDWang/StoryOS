//! An applied replay of a retained Proposal decision refuses an unknown preserved Proposal
//! State Axis value in its effect row.

use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use super::damaged_evidence::{ReplayError, replay_past_checks};
use super::proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use super::support::{CommandCall, SequenceError, stores};

/// Settles the call and replays it once with an unknown value in one preserved column of its
/// effect row, which `receipt_column` binds to the Receipt.
async fn replay_with_unknown_preserved_value<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    table: &str,
    receipt_column: &str,
    column: &str,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let filter = format!(
        "WHERE {receipt_column} = '{}'",
        call.envelope.ids.receipt_id
    );
    let value: String = admin
        .query_one(
            &format!("SELECT {column} FROM storyos.{table} {filter}"),
            &[],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    replay_past_checks(
        admin,
        store,
        call,
        table,
        column,
        &format!("UPDATE storyos.{table} SET {column} = 'damaged' {filter}"),
        &format!("UPDATE storyos.{table} SET {column} = '{value}' {filter}"),
    )
    .await
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_replay_refuses_an_unknown_preserved_proposal_state() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x9d00).await,
            "proposal_rejection_receipts",
            "rejection_receipt_id",
            "preserved_closure",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x9d10).await,
            "proposal_withdrawals",
            "withdrawal_receipt_id",
            "preserved_validation",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x9d20).await,
            "proposal_replans",
            "replan_receipt_id",
            "preserved_generation",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x9d30).await,
            "proposal_withdrawal_reopenings",
            "reopen_receipt_id",
            "preserved_operation_resolution",
        )
        .await,
        replay_with_unknown_preserved_value(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x9d40).await,
            "proposal_operation_reopenings",
            "reopen_receipt_id",
            "preserved_closure",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 5]);
}

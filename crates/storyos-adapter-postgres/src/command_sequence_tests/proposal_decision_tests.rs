use tokio_postgres::Client;
use tokio_postgres::error::SqlState;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_damaged_evidence::{ReplayError, damaged_replay};
use crate::shared_tests::command_sequence_proposal_decision::{
    reject_proposal_operations, reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use crate::shared_tests::command_sequence_support::{
    CommandCall, SequenceError, replayed_outcome, run_without_foreign_keys, stores,
    with_new_request_ids,
};

/// The effect row of one applied Proposal decision and its preserved Proposal State Axes.
struct PreservedEffect {
    table: &'static str,
    receipt_column: &'static str,
    preserved_columns: &'static [&'static str],
}

/// Settles the call and clears one preserved value, which the all-or-none check refuses. Then
/// replays the call once without its preserved values and once without its effect row.
async fn effect_replays<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    effect: PreservedEffect,
) -> (Option<SqlState>, [ReplayError; 2]) {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the Proposal decision must settle");
    };
    let PreservedEffect {
        table,
        receipt_column,
        preserved_columns,
    } = effect;
    let filter = format!(
        "WHERE {receipt_column} = '{}'",
        call.envelope.ids.receipt_id
    );
    let cleared = preserved_columns
        .iter()
        .map(|column| format!("{column} = NULL"))
        .collect::<Vec<_>>()
        .join(", ");
    let partial = admin
        .batch_execute(&format!(
            "UPDATE storyos.{table} SET {} = NULL {filter}",
            preserved_columns[0]
        ))
        .await
        .err()
        .and_then(|error| error.code().cloned());
    let mut errors = Vec::new();
    for statement in [
        format!("UPDATE storyos.{table} SET {cleared} {filter}"),
        format!("DELETE FROM storyos.{table} {filter}"),
    ] {
        run_without_foreign_keys(admin, &statement).await;
        let retry = with_new_request_ids(call);
        let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
            panic!("a replay without its preserved values must fail");
        };
        errors.push(error.sequence().into());
    }
    (partial, errors.try_into().unwrap())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_proposal_decision_replay_separates_pre_capture_from_damaged_effect_rows() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        effect_replays(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7c00).await,
            PreservedEffect {
                table: "proposal_rejection_receipts",
                receipt_column: "rejection_receipt_id",
                preserved_columns: &[
                    "preserved_generation",
                    "preserved_validation",
                    "preserved_closure",
                ],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7c10).await,
            PreservedEffect {
                table: "proposal_withdrawals",
                receipt_column: "withdrawal_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_validation"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7c20).await,
            PreservedEffect {
                table: "proposal_replans",
                receipt_column: "replan_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_closure"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x7c30).await,
            PreservedEffect {
                table: "proposal_withdrawal_reopenings",
                receipt_column: "reopen_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_operation_resolution"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7c40).await,
            PreservedEffect {
                table: "proposal_operation_reopenings",
                receipt_column: "reopen_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_closure"],
            },
        )
        .await,
    ];
    let separated = (
        Some(SqlState::CHECK_VIOLATION),
        [
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ],
    );
    assert_eq!(observed, vec![separated; 5]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_multi_operation_rejection_replays_the_resolution_of_its_first_selected_operation() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let mut call = reject_proposal_operations_call(&store, &admin, /*base*/ 0x7c50).await;
    let first_operation = call.input.selected_pending_operation_ids[0].clone();
    let later_operation = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        &admin,
        &format!(
            "INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             SELECT owner_user_id, project_id, proposal_id, '{later_operation}',
                    gen_random_uuid(), resolution, reservation_state, candidate_text
               FROM storyos.proposal_operations WHERE operation_id = '{first_operation}'"
        ),
    )
    .await;
    call.input.selected_pending_operation_ids = vec![later_operation, first_operation];
    let (result_kind, _rows) =
        replayed_outcome(&store, &admin, &call, reject_proposal_operations).await;
    assert_eq!(result_kind, "proposal_operations_resolved");
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_pre_capture_effect_row_does_not_hide_a_damaged_author_action() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        damaged_replay(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0xa600).await,
            |receipt_id| {
                format!(
                    "UPDATE storyos.proposal_rejection_receipts
                        SET preserved_generation = NULL, preserved_validation = NULL,
                            preserved_closure = NULL
                      WHERE rejection_receipt_id = '{receipt_id}';
                     DELETE FROM storyos.author_action_entries WHERE receipt_id = '{receipt_id}'"
                )
            },
        )
        .await,
        damaged_replay(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0xa610).await,
            |receipt_id| {
                format!(
                    "UPDATE storyos.proposal_withdrawals
                        SET preserved_generation = NULL, preserved_validation = NULL
                      WHERE withdrawal_receipt_id = '{receipt_id}';
                     {}",
                    compensation_action(receipt_id)
                )
            },
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 2]);
}

/// Gives the Forward Author Action of `receipt_id` the compensation disposition.
fn compensation_action(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.author_action_entries
            SET disposition = 'compensation', compensated_source_sequence = author_action_sequence
          WHERE receipt_id = '{receipt_id}'"
    )
}

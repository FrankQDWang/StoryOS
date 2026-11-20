//! The shared test support from `command_sequence_tests/retry_tests.rs`.

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};
use crate::shared_tests::command_sequence_support::{CommandCall, SequenceError, settlement_rows};
use storyos_application::ProjectCommandError;
use tokio_postgres::Client;

/// Marks the Command Challenge consumed and the Command Idempotency Fence in progress, then retries.
///
/// Returns whether the retry is a binding conflict and the rows of the retry Receipt.
pub(crate) async fn in_progress_retry<C: ProjectCommand<Error: SequenceError>>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (bool, [i64; 5]) {
    admin
        .batch_execute(&format!(
            "UPDATE storyos.project_command_challenges SET consumed_at = clock_timestamp()
              WHERE idempotency_key = '{key}';
             UPDATE storyos.command_idempotency SET outcome_kind = 'in_progress'
              WHERE idempotency_key = '{key}';",
            key = call.envelope.challenge_binding.idempotency_key,
        ))
        .await
        .unwrap();
    let retry = settle_project_command(store, &call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence);
    (
        matches!(retry, Err(ProjectCommandError::BindingConflict)),
        settlement_rows(admin, &call.envelope.ids.receipt_id).await,
    )
}

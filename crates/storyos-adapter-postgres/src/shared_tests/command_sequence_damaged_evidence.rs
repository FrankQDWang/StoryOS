//! The shared test support from `command_sequence_tests/damaged_evidence_tests.rs`.

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};
use crate::shared_tests::command_sequence_support::{
    CommandCall, SequenceError, run_without_foreign_keys, with_new_request_ids,
};
use std::fmt::Debug;
use storyos_application::ProjectCommandError;
use storyos_core::ReceiptResult;
use tokio_postgres::Client;

/// The replay error of one exact retry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReplayError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    WriterIneligible,
    Unavailable,
}

/// Settles the call, then replays it with pre-capture and then with damaged acknowledgement evidence.
pub(crate) async fn evidence_replays<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (ReceiptResult, [ReplayError; 2]) {
    let Ok(settled) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let key = &call.envelope.challenge_binding.idempotency_key;
    let mut errors = Vec::new();
    for evidence in [
        "acknowledgement_format = NULL, response_project = NULL, response_assistance = NULL",
        "acknowledgement_format = 'command_response_project.v1',
         response_project = '{\"broken\":true}'::jsonb, response_assistance = NULL",
    ] {
        admin
            .batch_execute(&format!(
                "UPDATE storyos.command_idempotency SET {evidence}
                  WHERE idempotency_key = '{key}'"
            ))
            .await
            .unwrap();
        let retry = with_new_request_ids(call);
        let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
            panic!("a replay without complete evidence must fail");
        };
        errors.push(error.sequence().into());
    }
    let errors: [ReplayError; 2] = errors.try_into().unwrap();
    (settled.outcome.receipt_result(), errors)
}

impl From<ProjectCommandError> for ReplayError {
    fn from(error: ProjectCommandError) -> Self {
        match error {
            ProjectCommandError::BindingConflict => Self::BindingConflict,
            ProjectCommandError::HistoricalAcknowledgementUnavailable => {
                Self::HistoricalAcknowledgementUnavailable
            }
            ProjectCommandError::InvalidChallenge => Self::InvalidChallenge,
            ProjectCommandError::MissingProject => Self::MissingProject,
            ProjectCommandError::WriterIneligible => Self::WriterIneligible,
            ProjectCommandError::Unavailable(_) => Self::Unavailable,
        }
    }
}

/// Settles the call, damages its records with `damage` that takes the Receipt identity, and
/// replays it.
pub(crate) async fn damaged_replay<C: ProjectCommand<Error: SequenceError> + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    damage: fn(&str) -> String,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    run_without_foreign_keys(admin, &damage(&call.envelope.ids.receipt_id)).await;
    let retry = with_new_request_ids(call);
    let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
        panic!("a replay with damaged records must fail");
    };
    error.sequence().into()
}

/// Settles the call and replays it once after `damage`, which the CHECK constraints of `table`
/// whose definition contains `checked` refuse.
///
/// The helper drops those constraints for the replay. Then it runs `repair` and restores them.
pub(crate) async fn replay_past_checks<C: ProjectCommand<Error: SequenceError> + Clone>(
    admin: &Client,
    store: &PostgresProjectReader,
    call: &CommandCall<C>,
    table: &str,
    checked: &str,
    damage: &str,
    repair: &str,
) -> ReplayError {
    let checks = admin
        .query(
            "SELECT conname::text, pg_get_constraintdef(oid) FROM pg_constraint
              WHERE conrelid = $1::text::regclass AND contype = 'c'
                AND strpos(pg_get_constraintdef(oid), $2) > 0",
            &[&format!("storyos.{table}"), &checked],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| {
            (
                row.get::<_, String>(/*idx*/ 0),
                row.get::<_, String>(/*idx*/ 1),
            )
        })
        .collect::<Vec<_>>();
    let dropped = checks
        .iter()
        .map(|(name, _)| format!("ALTER TABLE storyos.{table} DROP CONSTRAINT {name};"))
        .collect::<String>();
    run_without_foreign_keys(admin, &format!("{dropped} {damage}")).await;
    let retry = with_new_request_ids(call);
    let replayed = settle_project_command(store, &retry.envelope, &retry.input).await;
    let restored = checks
        .iter()
        .map(|(name, check)| format!("ALTER TABLE storyos.{table} ADD CONSTRAINT {name} {check};"))
        .collect::<String>();
    run_without_foreign_keys(admin, &format!("{repair}; {restored}")).await;
    let Err(error) = replayed else {
        panic!("a replay with a damaged stored value must fail");
    };
    error.sequence().into()
}

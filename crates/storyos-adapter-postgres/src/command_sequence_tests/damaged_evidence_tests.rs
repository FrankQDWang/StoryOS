use std::fmt::Debug;

use storyos_application::ProjectCommandError;
use storyos_core::ReceiptResult;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{ProjectCommand, settle_project_command};

use super::agent_run::{
    cancel_agent_run_call, create_agent_run_call, park_run, pause_agent_run_call,
};
use super::draft::{
    close_editor_flow_draft_call, discard_call, expand_refused_edit_draft_call, expansion_call,
    refused_edit_draft, refused_edit_expansion,
};
use super::project_session::{
    archive_project_call, update_project_assistance_call, update_project_call,
};
use super::proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use super::proposal_generation::{
    complete_ready_partial_proposal_call, continue_proposal_generation_call,
};
use super::steer_agent_run::steer_agent_run_call;
use super::structure::{
    create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    set_current_chapter_call, update_chapter_call, update_volume_call,
};
use super::support::{
    CommandCall, SequenceError, run_without_foreign_keys, stores, with_new_request_ids,
};

/// The replay error of one exact retry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ReplayError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    WriterIneligible,
    Unavailable,
}

/// Settles the call, then replays it with pre-capture and then with damaged acknowledgement evidence.
async fn evidence_replays<C: ProjectCommand<Error: SequenceError> + Clone>(
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_replay_separates_pre_capture_from_damaged_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
            &archive_project_call(&store, /*base*/ 0x6a70).await,
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
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7550).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7560).await,
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
    let mut expected = vec![separated(ReceiptResult::AuthoritativeApplied); 20];
    expected.push(separated(ReceiptResult::NoEffect));
    assert_eq!(observed, expected);
}

/// Settles the call, damages its records with `damage` that takes the Receipt identity, and
/// replays it.
pub(super) async fn damaged_replay<C: ProjectCommand<Error: SequenceError> + Clone>(
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
pub(super) async fn replay_past_checks<C: ProjectCommand<Error: SequenceError> + Clone>(
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
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
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
        replay_with_receipt_payload(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x9c50).await,
            "result_payload - 'transition'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x9c60).await,
            "result_payload - 'transition'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x9c70).await,
            "result_payload - 'reason'",
        )
        .await,
        replay_with_receipt_payload(
            &store,
            &admin,
            &expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x9c80).await,
            "result_payload - 'reason'",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 9]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_receipt_replay_refuses_a_null_or_zero_historical_order_and_a_zero_shape_value() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = vec![
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
    for (base, changed) in [
        (
            0xa220,
            format!(
                "jsonb_set(result_payload, '{{event_id}}', '\"{}\"'::jsonb)",
                Uuid::now_v7()
            ),
        ),
        (0xa230, "result_payload - 'event_id'".to_owned()),
    ] {
        let (scope, tombstoned) = refused_edit_draft(&store, &admin, base, "tombstoned").await;
        let refused = discard_call(&store, &scope, base + 9, tombstoned).await;
        observed.push(replay_with_receipt_payload(&store, &admin, &refused, &changed).await);
    }
    let (scope, tombstoned) =
        refused_edit_expansion(&store, &admin, /*base*/ 0xa240, "tombstoned").await;
    let refused = expansion_call(&store, &scope, /*suffix*/ 0xa249, tombstoned).await;
    observed.push(
        replay_with_receipt_payload(
            &store,
            &admin,
            &refused,
            "jsonb_set(result_payload, '{proposal_id}', '7'::jsonb)",
        )
        .await,
    );
    assert_eq!(observed, vec![ReplayError::Unavailable; 5]);
}

//! The admit step and the settle step of a direct editor action (ADR 0044).
//!
//! The admit step consumes the Command Challenge and commits the Admission. The fence stays
//! `in_progress`. The settle step settles the admitted command in a new transaction.

use storyos_application::{
    ProjectCommandChallengeTransaction, ProjectCommandChallengeUse, ProjectCommandEnvelope,
    ProjectCommandError,
};
use tokio_postgres::Client;

use super::admission::insert_admission;
use super::records::{lock_project, read_project};
use super::{
    Admission, CommandIsolation, MissingAdmission, ProjectCommand, SettledCommand,
    challenge_problem, replay_command, replay_problem, settle_classified, unavailable,
};
use crate::PostgresProjectReader;
use crate::command_replay::read_command_replay;

/// The result reference of a fence that the outcome query settled as `RequiresReconfirmation`.
const REQUIRES_RECONFIRMATION: &str = "requires_reconfirmation";

/// An Admission that the settle step cannot settle.
pub(crate) enum AdmittedRefusal {
    /// The Admission expired before its settlement.
    Expired,
    /// The Editor Session of the Admission is no longer the current writer.
    StaleWriter,
}

/// One project command that the admit step admits and the settle step settles.
pub(crate) trait AdmittedCommand: ProjectCommand {
    /// The Admission that the admit step records.
    fn admission(&self, envelope: &ProjectCommandEnvelope) -> Admission;

    /// The error of an Admission that the settle step cannot settle.
    fn admitted_refusal(refusal: AdmittedRefusal) -> Self::Error;

    /// Runs after the writes of the settle step and before its commit. An error rolls back the
    /// settlement.
    fn before_commit(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// The settlement state of one Admission, which the settle step reads under its lock.
enum AdmittedState {
    Open,
    Settled { receipt_id: String },
    Refused(AdmittedRefusal),
    BindingConflict,
}

/// Runs the admit step and then the settle step. `after_admission` runs between the steps.
pub(crate) async fn admit_and_settle<C: AdmittedCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    after_admission: impl FnOnce() -> Result<(), C::Error>,
) -> Result<SettledCommand<C>, C::Error> {
    let challenge_error = |error| challenge_problem(C::SPEC.rate_limited, error);
    let mut transaction = match C::SPEC.isolation {
        CommandIsolation::Serializable => {
            store
                .begin_serializable_project_command_transaction(&envelope.project_scope)
                .await
        }
    }
    .map_err(challenge_error)?;
    let challenge_use = transaction
        .consume(&envelope.challenge_binding, &envelope.nonce_digest)
        .await
        .map_err(challenge_error)?;
    match challenge_use {
        ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
            transaction.rollback().await.map_err(challenge_error)?;
            if result_reference == REQUIRES_RECONFIRMATION {
                return Err(ProjectCommandError::BindingConflict.into());
            }
            return replay_settled(store, envelope, command, &result_reference).await;
        }
        ProjectCommandChallengeUse::ExactRetryInProgress => {
            transaction.rollback().await.map_err(challenge_error)?;
            return Err(ProjectCommandError::BindingConflict.into());
        }
        ProjectCommandChallengeUse::FirstUse => {
            match admit(&transaction.client, envelope, command).await {
                Ok(()) => transaction.commit().await.map_err(challenge_error)?,
                Err(error) => {
                    transaction.rollback().await.map_err(challenge_error)?;
                    return Err(error);
                }
            }
        }
    }
    after_admission()?;
    settle_admitted_command(store, envelope, command).await
}

async fn admit<C: AdmittedCommand>(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<(), C::Error> {
    lock_project(client, envelope).await?;
    if insert_admission(client, envelope, C::SPEC.kind, &command.admission(envelope)).await? {
        return Ok(());
    }
    Err(match C::SPEC.missing_admission {
        MissingAdmission::Diagnosed => command.diagnose_missing_admission(client, envelope).await,
        MissingAdmission::InvalidChallenge
        | MissingAdmission::BindingConflict
        | MissingAdmission::InvalidWriter => C::SPEC.missing_admission.error().into(),
    })
}

/// Settles one admitted command in a new transaction. It reads the Project row without a lock, and
/// the command locks its own facts in `classify`. It requires an Admission without settlement that is not expired and whose Editor Session is
/// the current writer. A settled Admission replays its Receipt.
pub(crate) async fn settle_admitted_command<C: AdmittedCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, C::Error> {
    let challenge_error = |error| challenge_problem(C::SPEC.rate_limited, error);
    let transaction = match C::SPEC.isolation {
        CommandIsolation::Serializable => {
            store
                .begin_serializable_project_command_transaction(&envelope.project_scope)
                .await
        }
    }
    .map_err(challenge_error)?;
    let client = &transaction.client;
    let settled = async {
        let project = read_project(client, envelope).await?;
        match admitted_state(client, envelope).await? {
            AdmittedState::Open => {}
            AdmittedState::Settled { receipt_id } => return Ok(Err(receipt_id)),
            AdmittedState::Refused(refusal) => return Err(C::admitted_refusal(refusal)),
            AdmittedState::BindingConflict => {
                return Err(ProjectCommandError::BindingConflict.into());
            }
        }
        let classification = command.classify(client, envelope, &project).await?;
        let settlement =
            settle_classified(client, envelope, command, &project, classification).await?;
        command.before_commit()?;
        Ok(Ok(settlement))
    }
    .await;
    match settled {
        Ok(Ok(settlement)) => {
            transaction.commit().await.map_err(challenge_error)?;
            Ok(settlement)
        }
        Ok(Err(receipt_id)) => {
            transaction.rollback().await.map_err(challenge_error)?;
            replay_settled(store, envelope, command, &receipt_id).await
        }
        Err(error) => {
            transaction.rollback().await.map_err(challenge_error)?;
            // A concurrent settle step can settle the same Admission first.
            match settled_receipt(store, envelope).await? {
                Some(receipt_id) => replay_settled(store, envelope, command, &receipt_id).await,
                None => Err(error),
            }
        }
    }
}

/// Replays the Domain Receipt `receipt_id` of one settled command.
pub(crate) async fn replay_settled<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    receipt_id: &str,
) -> Result<SettledCommand<C>, C::Error> {
    read_command_replay(
        store,
        &envelope.challenge_binding,
        receipt_id,
        &envelope.canonical_command_bytes,
        &C::SPEC.replay_effect,
    )
    .await
    .and_then(|replay| replay_command(command, &replay))
    .map_err(|fault| replay_problem(fault).into())
}

/// Reads the settlement, expiry, and writer facts of the Admission of `envelope`. The settlement
/// row is unique, so two settle steps cannot both settle one Admission.
async fn admitted_state(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<AdmittedState, ProjectCommandError> {
    let Some(row) = client
        .query_opt(
            "SELECT settlement.settlement_kind, settlement.receipt_id::text,
                    admission.challenge_expires_at > clock_timestamp(),
                    admission.writer_generation < (
                      SELECT max(current_writer.writer_generation)
                        FROM storyos.project_writer_generations AS current_writer
                       WHERE (current_writer.owner_user_id, current_writer.project_id) =
                             (admission.owner_user_id, admission.project_id)),
                    EXISTS (
                      SELECT 1 FROM storyos.project_writer_generations AS writer
                       WHERE (writer.owner_user_id, writer.project_id,
                              writer.current_editor_session_id, writer.writer_generation) =
                             (admission.owner_user_id, admission.project_id,
                              admission.editor_session_id, admission.writer_generation)
                         AND writer.writer_generation = (
                           SELECT max(current_writer.writer_generation)
                             FROM storyos.project_writer_generations AS current_writer
                            WHERE (current_writer.owner_user_id, current_writer.project_id) =
                                  (admission.owner_user_id, admission.project_id)))
               FROM storyos.author_command_admissions AS admission
          LEFT JOIN storyos.author_command_admission_settlements AS settlement
                 ON (settlement.owner_user_id, settlement.project_id,
                     settlement.author_command_admission_id) =
                    (admission.owner_user_id, admission.project_id,
                     admission.author_command_admission_id)
              WHERE admission.owner_user_id = $1::text::uuid
                AND admission.project_id = $2::text::uuid
                AND admission.author_command_admission_id = $3::text::uuid",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.author_command_admission_id,
            ],
        )
        .await
        .map_err(unavailable)?
    else {
        return Ok(AdmittedState::BindingConflict);
    };
    let settlement_kind = row.get::<_, Option<String>>(/*idx*/ 0);
    let receipt_id = row.get::<_, Option<String>>(/*idx*/ 1);
    if let (Some("receipt_settled"), Some(receipt_id)) = (settlement_kind.as_deref(), receipt_id) {
        return Ok(AdmittedState::Settled { receipt_id });
    }
    let unexpired = row.get::<_, bool>(/*idx*/ 2);
    let stale_writer = row.get::<_, Option<bool>>(/*idx*/ 3) == Some(true);
    let current_writer = row.get::<_, bool>(/*idx*/ 4);
    Ok(
        if settlement_kind.is_none() && unexpired && current_writer {
            AdmittedState::Open
        } else if !unexpired {
            AdmittedState::Refused(AdmittedRefusal::Expired)
        } else if stale_writer {
            AdmittedState::Refused(AdmittedRefusal::StaleWriter)
        } else {
            AdmittedState::BindingConflict
        },
    )
}

/// The Receipt of the settled Admission of `envelope`, read in a new connection.
async fn settled_receipt(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
) -> Result<Option<String>, ProjectCommandError> {
    let client = store.connect_challenge().await.map_err(unavailable)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(unavailable)?;
    let receipt = async {
        crate::set_challenge_scope_on_client(&client, &envelope.project_scope)
            .await
            .map_err(unavailable)?;
        client
            .query_opt(
                "SELECT receipt_id::text FROM storyos.author_command_admission_settlements
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND author_command_admission_id = $3::text::uuid
                    AND settlement_kind = 'receipt_settled'",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &envelope.ids.author_command_admission_id,
                ],
            )
            .await
            .map_err(unavailable)
    }
    .await;
    let _end = client.batch_execute("COMMIT").await;
    Ok(receipt?.map(|row| row.get(/*idx*/ 0)))
}

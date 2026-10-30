//! The admit step and the settle step of a direct editor action (ADR 0076).
//!
//! The admit step consumes the Command Challenge and commits the Admission. The fence stays
//! `in_progress`. The settle step settles the admitted command in a new transaction.

use storyos_application::{
    DiagnosticField as _, DiagnosticId, ProjectCommandEnvelope, ProjectCommandError,
};
use tokio_postgres::Client;

use super::admission::insert_admission;
use super::records::read_project;
use super::skeleton::{
    ChallengeUse, FirstUse, begin_command_transaction, challenge_problem, consume_challenge,
    record_ids, record_result,
};
use super::{
    Admission, Classification, ProjectCommand, RateLimitedChallenge, SettledCommand,
    replay_receipt, settle_classified, unavailable,
};
use crate::PostgresProjectReader;

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
#[tracing::instrument(skip_all, fields(
    command_kind = C::SPEC.kind.diagnostic(),
    project_id = envelope.project_scope.project_id.diagnostic(),
    command_id = tracing::field::Empty,
    author_command_admission_id = tracing::field::Empty,
    correlation_id = DiagnosticId(&envelope.correlation_id).diagnostic(),
    outcome = tracing::field::Empty,
))]
pub(crate) async fn admit_and_settle<C: AdmittedCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    after_admission: impl FnOnce() -> Result<(), C::Error>,
) -> Result<SettledCommand<C>, C::Error> {
    let settled = async {
        match consume_challenge(store, envelope, &Admit(command)).await? {
            ChallengeUse::Committed(()) => {
                tracing::Span::current().record("outcome", "admitted");
            }
            ChallengeUse::ExactRetrySettled(result_reference) => {
                if result_reference == REQUIRES_RECONFIRMATION {
                    return Err(ProjectCommandError::BindingConflict.into());
                }
                return replay_receipt(store, envelope, command, &result_reference).await;
            }
            ChallengeUse::ExactRetryInProgress => {
                return Err(ProjectCommandError::BindingConflict.into());
            }
        }
        after_admission()?;
        // The settle step records its outcome on this span, so that one command has one span.
        settle_admitted_steps(store, envelope, command).await
    }
    .await;
    record_result(&settled);
    settled
}

/// The admit step of a direct editor action: its first use commits only the Admission.
struct Admit<'c, C>(&'c C);

impl<C: AdmittedCommand> FirstUse for Admit<'_, C> {
    type Output = ();
    type Error = C::Error;
    const RATE_LIMITED: RateLimitedChallenge = C::SPEC.rate_limited;

    async fn write(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> Result<(), C::Error> {
        record_ids(
            &envelope.ids.command_id,
            &envelope.ids.author_command_admission_id,
        );
        let command = self.0;
        // The Admission insert requires an active Project. As on `main`, the admit step locks no
        // Project row, so a concurrent transaction that holds a key share of it does not stop it.
        read_project(client, envelope).await?;
        if insert_admission(client, envelope, C::SPEC.kind, &command.admission(envelope)).await? {
            return Ok(());
        }
        Err(command.diagnose_missing_admission(client, envelope).await)
    }
}

/// Settles one admitted command in a new transaction. It reads the Project row without a lock, and
/// the command locks its own facts in `classify`. It requires an Admission without settlement
/// that is not expired and whose Editor Session is the current writer. A settled Admission
/// replays its Receipt.
#[tracing::instrument(skip_all, fields(
    command_kind = C::SPEC.kind.diagnostic(),
    project_id = envelope.project_scope.project_id.diagnostic(),
    command_id = tracing::field::Empty,
    author_command_admission_id = tracing::field::Empty,
    correlation_id = DiagnosticId(&envelope.correlation_id).diagnostic(),
    outcome = tracing::field::Empty,
))]
pub(crate) async fn settle_admitted_command<C: AdmittedCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, C::Error> {
    record_ids(
        &envelope.ids.command_id,
        &envelope.ids.author_command_admission_id,
    );
    let settled = settle_admitted_steps(store, envelope, command).await;
    record_result(&settled);
    settled
}

async fn settle_admitted_steps<C: AdmittedCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, C::Error> {
    let challenge_error = |error| challenge_problem(C::SPEC.rate_limited, error);
    let transaction = begin_command_transaction(store, envelope, C::SPEC.rate_limited).await?;
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
        // The admit step inserted the Admission, so the settle step does not read it.
        let Classification {
            outcome,
            admission: _,
            heads,
            zero_receipt,
        } = command.classify(client, envelope, &project).await?;
        let settlement = settle_classified(
            client,
            envelope,
            command,
            &project,
            outcome,
            heads,
            zero_receipt,
        )
        .await?;
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
            replay_recorded(store, envelope, command, &receipt_id).await
        }
        Err(error) => {
            transaction.rollback().await.map_err(challenge_error)?;
            // A concurrent settle step can settle the same Admission first.
            match settled_receipt(store, envelope).await? {
                Some(receipt_id) => replay_recorded(store, envelope, command, &receipt_id).await,
                None => Err(error),
            }
        }
    }
}

/// Replays the Domain Receipt `receipt_id` of one settled command.
#[tracing::instrument(skip_all, fields(
    command_kind = C::SPEC.kind.diagnostic(),
    project_id = envelope.project_scope.project_id.diagnostic(),
    command_id = tracing::field::Empty,
    author_command_admission_id = tracing::field::Empty,
    correlation_id = DiagnosticId(&envelope.correlation_id).diagnostic(),
    outcome = tracing::field::Empty,
))]
pub(crate) async fn replay_settled<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    receipt_id: &str,
) -> Result<SettledCommand<C>, C::Error> {
    let replayed = replay_recorded(store, envelope, command, receipt_id).await;
    record_result(&replayed);
    replayed
}

/// Replays one Domain Receipt and records `replayed` on the current span.
async fn replay_recorded<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    receipt_id: &str,
) -> Result<SettledCommand<C>, C::Error> {
    tracing::Span::current().record("outcome", "replayed");
    replay_receipt(store, envelope, command, receipt_id).await
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
    // The read is read-only, so a failed COMMIT does not change the Receipt that it read.
    let _end = client.batch_execute("COMMIT").await;
    Ok(receipt?.map(|row| row.get(/*idx*/ 0)))
}

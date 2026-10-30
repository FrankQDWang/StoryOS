//! The one skeleton of the entry points that consume a Command Challenge (ADR 0076).

use std::future::Future;

use storyos_application::{
    AdmittedProjectCommand, DiagnosticField as _, DiagnosticId, ProjectCommandChallengeError,
    ProjectCommandChallengeTransaction, ProjectCommandChallengeUse, ProjectCommandEnvelope,
    ProjectCommandError, ProjectCommandSettlement,
};
use tokio_postgres::Client;

use super::contention::{CommandError, contended};
use super::unavailable;
use crate::command_replay::ReplayFault;
use crate::{PostgresProjectCommandTransaction, PostgresProjectReader};

/// The first use of one entry point: the rows that it writes in the Challenge transaction.
///
/// `consume_challenge` runs it after the Challenge use and before the commit. An implementation
/// never runs transaction control.
pub(super) trait FirstUse: Sync {
    type Output: Send;
    type Error: CommandError;
    /// The refusal of a serialization failure, a unique violation, or a deadlock of the first use,
    /// including its commit. `None` keeps such a failure a store fault.
    const CONTENTION_REFUSAL: Option<Self::Error> = None;

    fn write(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// The result of one Command Challenge use.
pub(super) enum ChallengeUse<T> {
    /// The committed first use.
    Committed(T),
    /// An exact retry of a settled command, with the result reference of its fence.
    ExactRetrySettled(String),
    /// An exact retry whose fence is still `in_progress`.
    ExactRetryInProgress,
}

/// Begins the Serializable transaction of one command, consumes its Command Challenge, and runs
/// its first use. Commits a first use that succeeded, and rolls back every other path. An exact
/// retry of a settled command records `replayed` on the command span.
pub(super) async fn consume_challenge<F: FirstUse>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    first_use: &F,
) -> Result<ChallengeUse<F::Output>, F::Error> {
    let mut transaction = begin_command_transaction(store, envelope).await?;
    let challenge_use = transaction
        .consume(&envelope.challenge_binding, &envelope.nonce_digest)
        .await
        .map_err(challenge_problem)?;
    match challenge_use {
        ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
            transaction.rollback().await.map_err(challenge_problem)?;
            tracing::Span::current().record("outcome", "replayed");
            Ok(ChallengeUse::ExactRetrySettled(result_reference))
        }
        ProjectCommandChallengeUse::ExactRetryInProgress => {
            transaction.rollback().await.map_err(challenge_problem)?;
            Ok(ChallengeUse::ExactRetryInProgress)
        }
        ProjectCommandChallengeUse::FirstUse => {
            match first_use.write(&transaction.client, envelope).await {
                Ok(output) => transaction
                    .commit()
                    .await
                    .map(|()| ChallengeUse::Committed(output))
                    .map_err(|error| challenge_problem(error).into()),
                Err(error) => {
                    transaction.rollback().await.map_err(challenge_problem)?;
                    Err(error)
                }
            }
            .map_err(|error| match F::CONTENTION_REFUSAL {
                Some(refusal) if error.sequence_error().is_some_and(contended) => refusal,
                Some(_) | None => error,
            })
        }
    }
}

/// Begins the Serializable transaction of one command and sets its Project Scope.
pub(super) async fn begin_command_transaction(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
) -> Result<PostgresProjectCommandTransaction, ProjectCommandError> {
    store
        .begin_serializable_project_command_transaction(&envelope.project_scope)
        .await
        .map_err(challenge_problem)
}

/// The command and Admission identities of one sequence result.
pub(super) trait CommandIds {
    fn command_ids(&self) -> (&str, &str);
}

impl<A, N, F, R, P, Z> CommandIds for ProjectCommandSettlement<A, N, F, R, P, Z> {
    fn command_ids(&self) -> (&str, &str) {
        (&self.ids.command_id, &self.ids.author_command_admission_id)
    }
}

impl<W, P> CommandIds for AdmittedProjectCommand<W, P> {
    fn command_ids(&self) -> (&str, &str) {
        (&self.command_id, &self.author_command_admission_id)
    }
}

/// Records the identifiers that a result returns, so that a replay records the stored
/// identifiers, or the reason code of a failed or refused command.
pub(super) fn record_result(result: &Result<impl CommandIds, impl CommandError>) {
    match result {
        Ok(output) => {
            let (command_id, author_command_admission_id) = output.command_ids();
            record_ids(command_id, author_command_admission_id);
        }
        Err(error) => {
            tracing::Span::current().record("outcome", error.reason_code().diagnostic());
        }
    }
}

/// Records the command identifiers on the current command span.
pub(super) fn record_ids(command_id: &str, author_command_admission_id: &str) {
    tracing::Span::current().record("command_id", DiagnosticId(command_id).diagnostic());
    tracing::Span::current().record(
        "author_command_admission_id",
        DiagnosticId(author_command_admission_id).diagnostic(),
    );
}

pub(super) fn replay_problem(fault: ReplayFault) -> ProjectCommandError {
    match fault {
        ReplayFault::BindingConflict => ProjectCommandError::BindingConflict,
        ReplayFault::HistoricalAcknowledgementUnavailable => {
            ProjectCommandError::HistoricalAcknowledgementUnavailable
        }
        ReplayFault::Unavailable(source) => ProjectCommandError::Unavailable(source),
    }
}

/// Maps an error of the Challenge transaction. Only a Challenge issue applies the rate limit, so a
/// `RateLimited` error here is a store fault.
pub(super) fn challenge_problem(error: ProjectCommandChallengeError) -> ProjectCommandError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ProjectCommandError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => ProjectCommandError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => unavailable(error),
    }
}

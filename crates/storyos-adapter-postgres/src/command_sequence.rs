//! The fixed Author Command Admission, Core Transition, and Command Acknowledgement sequence
//! of the project commands (ADR 0041, ADR 0043).

use std::future::Future;

use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeTransaction, ProjectCommandChallengeUse,
    ProjectCommandEnvelope, ProjectCommandError, ProjectCommandSettlement, ProjectScope,
};
use storyos_core::{ProjectLifecycle, ReasonCode, TransitionOutcome};
use tokio_postgres::Client;

use crate::PostgresProjectReader;

mod activity_only;
mod admission;
mod chapter_selection;
mod records;
mod response;
mod structural;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
pub(crate) use activity_only::{ActivityOnly, ActivitySequences, ActivityWrite};
use admission::insert_admission;
pub(crate) use chapter_selection::{ChapterSelection, ChapterSelectionWrite};
use records::{ReceiptRecord, insert_receipt, lock_project};
pub(crate) use response::{ProjectAssistanceResponse, ProjectResponse, ResponseRecord};
pub(crate) use structural::{
    CurrentChapterChange, Structural, StructureIdentity, StructureWrite, WriterBase,
};

pub(crate) enum CommandIsolation {
    Serializable,
}

/// The action class and Editor Session binding that the Author Command Admission records.
pub(crate) enum Admission {
    ExplicitProjectCommand,
    /// A command of the current writer Editor Session; the insert requires its writer generation.
    ExplicitEditorCommand(EditorAdmission),
}

pub(crate) struct EditorAdmission {
    pub(crate) editor_session_id: String,
    pub(crate) chapter_object_id: Option<String>,
    pub(crate) expected_authoritative_revision_id: Option<String>,
}

/// The head arrays that the Domain Receipt of every outcome records.
#[derive(Default)]
pub(crate) struct ReceiptHeads {
    pub(crate) expected: Vec<String>,
    pub(crate) prior: Vec<String>,
    pub(crate) resulting: Vec<String>,
}

/// The error of an Admission insert that inserts no row.
pub(crate) enum MissingAdmission {
    InvalidChallenge,
}

pub(crate) struct CommandSpec {
    pub(crate) kind: &'static str,
    pub(crate) isolation: CommandIsolation,
    pub(crate) missing_admission: MissingAdmission,
    pub(crate) activity_kind: &'static str,
}

/// The Project row that the sequence locks before a command loads its own facts.
pub(crate) struct LockedProject {
    pub(crate) lifecycle: ProjectLifecycle,
    pub(crate) tree_revision: u64,
    pub(crate) current_chapter_id: Option<String>,
}

/// The authority records that an applied project command allocates and writes (ADR 0043).
///
/// A command names one profile. The sequence calls `allocate` before the Domain Receipt and
/// `persist` after the command effect rows; on an exact retry it calls `replay` instead.
/// Implementations never run transaction control and never write the Admission, Receipt,
/// settlement link, or idempotency rows.
pub(crate) trait SettlementProfile {
    type Sequences: Send + Sync;
    /// The applied writes that a command of this profile returns.
    type Write<E: Send>: Send;
    /// The applied value of the settlement, equal on first delivery and replay.
    type Applied<E: Send>: Send;

    fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Self::Sequences, ProjectCommandError>> + Send;

    /// The Authoritative Commit identities that the Domain Receipt binds.
    fn commit_ids(sequences: &Self::Sequences) -> Vec<String>;

    fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity_kind: &'static str,
        sequences: Self::Sequences,
        write: Self::Write<E>,
    ) -> impl Future<Output = Result<Self::Applied<E>, ProjectCommandError>> + Send;

    fn replay<E: Send>(effect: E, replay: &CommandReplay) -> Result<Self::Applied<E>, ReplayFault>;
}

/// A Core Transition Outcome whose applied value carries the locked facts that `apply` reuses.
pub(crate) type Classified<C> = TransitionOutcome<
    (<C as ProjectCommand>::Applied, <C as ProjectCommand>::Plan),
    <C as ProjectCommand>::NoEffect,
    <C as ProjectCommand>::Conflict,
    <C as ProjectCommand>::Refusal,
>;

/// The Core Transition Outcome of one command and the facts that the sequence records with it.
pub(crate) struct Classification<C: ProjectCommand + ?Sized> {
    pub(crate) outcome: Classified<C>,
    pub(crate) admission: Admission,
    pub(crate) heads: ReceiptHeads,
}

impl<C: ProjectCommand + ?Sized> Classification<C> {
    /// An explicit project command whose Receipt has empty head arrays.
    pub(crate) fn project_command(outcome: Classified<C>) -> Self {
        Self {
            outcome,
            admission: Admission::ExplicitProjectCommand,
            heads: ReceiptHeads::default(),
        }
    }
}

/// The scope sequences that one command's settlement profile allocates.
pub(crate) type ProfileSequences<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Sequences;

/// The applied writes that one command returns for its settlement profile.
pub(crate) type ProfileWrite<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Write<<C as ProjectCommand>::Effect>;

/// One project command: its declared profiles, its locked facts, its own effect rows, and its
/// applied effect decoder.
///
/// Implementations never run transaction control (ADR 0031) and never write the Admission,
/// Receipt, settlement link, authority, or idempotency rows. `settle_project_command` writes
/// those rows in one fixed order.
pub(crate) trait ProjectCommand: Sync {
    const SPEC: CommandSpec;
    type Profile: SettlementProfile;
    type Response: ResponseRecord;
    type Applied: Send;
    type Plan: Send;
    type Effect: Send;
    type NoEffect: ReasonCode + Send;
    type Conflict: ReasonCode + Send;
    type Refusal: ReasonCode + Send;

    /// Locks the command facts and classifies them through Core.
    fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> impl Future<Output = Result<Classification<Self>, ProjectCommandError>> + Send;

    /// The Domain Receipt payload of an applied outcome.
    fn applied_receipt_payload(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> String {
        "{}".to_owned()
    }

    /// Writes the effect rows of an applied outcome after its profile sequences are allocated.
    fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> impl Future<Output = Result<ProfileWrite<Self>, ProjectCommandError>> + Send;

    /// Decodes the applied effect from the stored acknowledgement evidence.
    fn decode(&self, replay: &CommandReplay) -> Result<Self::Effect, ReplayFault>;
}

pub(crate) type SettledCommand<C> = ProjectCommandSettlement<
    <<C as ProjectCommand>::Profile as SettlementProfile>::Applied<<C as ProjectCommand>::Effect>,
    <C as ProjectCommand>::NoEffect,
    <C as ProjectCommand>::Conflict,
    <C as ProjectCommand>::Refusal,
    <<C as ProjectCommand>::Response as ResponseRecord>::Response,
>;

pub(crate) async fn settle_project_command<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, ProjectCommandError> {
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
            read_command_replay(store, &envelope.challenge_binding, &result_reference)
                .await
                .and_then(|replay| replay_command(command, &replay))
                .map_err(|fault| match fault {
                    ReplayFault::BindingConflict => ProjectCommandError::BindingConflict,
                    ReplayFault::HistoricalAcknowledgementUnavailable => {
                        ProjectCommandError::HistoricalAcknowledgementUnavailable
                    }
                    ReplayFault::Unavailable(source) => ProjectCommandError::Unavailable(source),
                })
        }
        ProjectCommandChallengeUse::ExactRetryInProgress => {
            transaction.rollback().await.map_err(challenge_error)?;
            Err(ProjectCommandError::BindingConflict)
        }
        ProjectCommandChallengeUse::FirstUse => {
            match first_use(&transaction.client, envelope, command).await {
                Ok(settlement) => {
                    transaction.commit().await.map_err(challenge_error)?;
                    Ok(settlement)
                }
                Err(error) => {
                    let _rollback = transaction.rollback().await;
                    Err(error)
                }
            }
        }
    }
}

async fn first_use<C: ProjectCommand>(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, ProjectCommandError> {
    let project = lock_project(client, envelope).await?;
    let Classification {
        outcome: classified,
        admission,
        heads,
    } = command.classify(client, envelope, &project).await?;
    insert_admission(client, envelope, &C::SPEC, &admission).await?;
    let receipt = ReceiptRecord {
        result: classified.receipt_result().code(),
        payload: match classified.reason_code() {
            Some(code) => serde_json::json!({ "reason": code }).to_string(),
            None => "{}".to_owned(),
        },
        command_kind: C::SPEC.kind,
        heads,
    };
    let (receipt_created_at, outcome) = match classified {
        TransitionOutcome::Applied((applied, plan)) => {
            let sequences = C::Profile::allocate(client, &envelope.project_scope).await?;
            let receipt = ReceiptRecord {
                payload: command.applied_receipt_payload(&applied, &plan),
                ..receipt
            };
            let receipt_created_at = insert_receipt(
                client,
                envelope,
                &receipt,
                &C::Profile::commit_ids(&sequences),
            )
            .await?;
            let write = command
                .apply(client, envelope, &project, &sequences, plan, applied)
                .await?;
            let applied = C::Profile::persist(
                client,
                envelope,
                &project,
                C::SPEC.activity_kind,
                sequences,
                write,
            )
            .await?;
            (receipt_created_at, TransitionOutcome::Applied(applied))
        }
        TransitionOutcome::NoEffect(reason) => (
            insert_receipt(client, envelope, &receipt, &[]).await?,
            TransitionOutcome::NoEffect(reason),
        ),
        TransitionOutcome::Conflicted(reason) => (
            insert_receipt(client, envelope, &receipt, &[]).await?,
            TransitionOutcome::Conflicted(reason),
        ),
        TransitionOutcome::Refused(reason) => (
            insert_receipt(client, envelope, &receipt, &[]).await?,
            TransitionOutcome::Refused(reason),
        ),
    };
    let response = C::Response::settle(client, envelope, C::SPEC.kind).await?;
    Ok(ProjectCommandSettlement {
        ids: envelope.ids.clone(),
        receipt_created_at,
        outcome,
        response,
    })
}

fn replay_command<C: ProjectCommand>(
    command: &C,
    replay: &CommandReplay,
) -> Result<SettledCommand<C>, ReplayFault> {
    let outcome = match replay.outcome::<C::NoEffect, C::Conflict, C::Refusal>()? {
        TransitionOutcome::Applied(()) => {
            TransitionOutcome::Applied(C::Profile::replay(command.decode(replay)?, replay)?)
        }
        TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
        TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
        TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
    };
    Ok(ProjectCommandSettlement {
        ids: replay.ids.clone(),
        receipt_created_at: replay.receipt_created_at.clone(),
        response: C::Response::replay(replay)?,
        outcome,
    })
}

pub(crate) fn unavailable(
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> ProjectCommandError {
    ProjectCommandError::Unavailable(error.into())
}

fn challenge_error(error: ProjectCommandChallengeError) -> ProjectCommandError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ProjectCommandError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => ProjectCommandError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => unavailable(error),
    }
}

#[cfg(test)]
#[path = "command_sequence_tests.rs"]
pub(crate) mod tests;

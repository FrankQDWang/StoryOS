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

mod action_only;
mod activity_only;
mod admission;
mod chapter_selection;
mod records;
mod response;
mod structural;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
pub(crate) use action_only::{ActionOnly, ActionSequence};
pub(crate) use activity_only::{ActivityOnly, ActivitySequences, ActivityWrite};
use admission::insert_admission;
pub(crate) use chapter_selection::{ChapterSelection, ChapterSelectionWrite};
use records::{ReceiptRecord, insert_activity_payload, insert_receipt, lock_project};
pub(crate) use response::{NoResponse, ProjectAssistanceResponse, ProjectResponse, ResponseRecord};
pub(crate) use structural::{
    CurrentChapterChange, Structural, StructureIdentity, StructureWrite, WriterBase,
};

pub(crate) enum CommandIsolation {
    Serializable,
}

/// The action class and Editor Session binding that the Author Command Admission records.
pub(crate) enum Admission {
    ExplicitProjectCommand,
    /// A command of the current writer Editor Session. The insert requires its writer generation.
    ExplicitEditorCommand(EditorAdmission),
    /// A writer takeover. The insert requires the observed writer generation of another session.
    WriterTakeover(TakeoverAdmission),
}

pub(crate) struct TakeoverAdmission {
    pub(crate) editor_session_id: String,
    pub(crate) observed_writer_generation: u64,
    pub(crate) editor_contract_revision: String,
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

/// The Proposal Revision, Draft, and lifecycle references that one Domain Receipt records.
#[derive(Default)]
pub(crate) struct ReceiptRefs {
    pub(crate) proposal_revision_ids: Vec<String>,
    pub(crate) draft_artifact_refs: Vec<String>,
    pub(crate) artifact_lifecycle_event_refs: Vec<String>,
    /// The JSON text of the source Draft disposition, when the Receipt records one.
    pub(crate) source_draft_disposition: Option<String>,
}

/// The error of an Admission insert that inserts no row.
pub(crate) enum MissingAdmission {
    InvalidChallenge,
    BindingConflict,
}

/// The error of a Command Challenge that the rate limit refuses.
#[derive(Clone, Copy)]
pub(crate) enum RateLimitedChallenge {
    InvalidChallenge,
    Unavailable,
}

/// The Domain Receipt result kind that an applied outcome of the command records.
pub(crate) struct AppliedResult(&'static str);

impl AppliedResult {
    /// The result kind of a change to Authoritative State or Project state.
    pub(crate) const AUTHORITATIVE_APPLIED: Self = Self("authoritative_applied");

    /// A result kind that the command declares for itself, for example `proposal_revised`.
    pub(crate) const fn command(code: &'static str) -> Self {
        Self(code)
    }

    pub(crate) fn code(&self) -> &'static str {
        self.0
    }
}

/// The effect rows of the command that an exact retry reads in the replay transaction.
pub(crate) enum ReplayEffect {
    NoQuery,
    /// One query that takes the owner, Project, and Receipt identities as `$1`, `$2`, and `$3`.
    /// Its optional row holds one JSON object text.
    Query(&'static str),
}

pub(crate) struct CommandSpec {
    pub(crate) kind: &'static str,
    pub(crate) applied_result: AppliedResult,
    pub(crate) isolation: CommandIsolation,
    pub(crate) missing_admission: MissingAdmission,
    pub(crate) rate_limited: RateLimitedChallenge,
    pub(crate) activity_kind: &'static str,
    pub(crate) replay_effect: ReplayEffect,
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
/// `persist` after the command effect rows. On an exact retry, it calls `replay` instead.
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
        spec: &CommandSpec,
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

/// A zero-authority outcome, which a command can settle with effect rows (ADR 0043).
pub(crate) enum ZeroOutcome<'a, C: ProjectCommand + ?Sized> {
    NoEffect(&'a C::NoEffect),
    Conflicted(&'a C::Conflict),
    Refused(&'a C::Refusal),
}

/// The rows that one zero-authority outcome writes besides its Admission, Receipt, and fence.
pub(crate) enum ZeroAuthorityRows {
    None,
    /// Effect rows without a Project Activity record.
    Effect,
    /// Effect rows and one Activity record at a position that the sequence allocates.
    EffectWithActivity,
}

/// The effect and the whole Activity payload of a zero-authority outcome that writes effect rows.
pub(crate) struct ZeroAuthorityWrite<Z> {
    pub(crate) effect: Z,
    /// The complete payload, including its `kind`. The event kind is the command Activity kind.
    pub(crate) activity: serde_json::Value,
}

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
    /// The effect of a zero-authority outcome that writes effect rows. Other commands use `()`.
    type ZeroEffect: Send;
    type Applied: Send;
    type Plan: Send;
    type Effect: Send;
    type NoEffect: ReasonCode + Send + Sync;
    type Conflict: ReasonCode + Send + Sync;
    type Refusal: ReasonCode + Send + Sync;

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

    /// The Domain Receipt references of an applied outcome.
    fn applied_receipt_refs(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> ReceiptRefs {
        ReceiptRefs::default()
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

    /// The rows that this zero-authority outcome writes (ADR 0043).
    fn zero_authority_rows(&self, _outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        ZeroAuthorityRows::None
    }

    /// Writes the effect rows of a zero-authority outcome that declares `Effect`.
    fn write_zero_authority_effect(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _outcome: &ZeroOutcome<'_, Self>,
    ) -> impl Future<Output = Result<Self::ZeroEffect, ProjectCommandError>> + Send {
        async { Err(unavailable("the command writes no zero-authority effect")) }
    }

    /// Writes the effect rows of a zero-authority outcome that declares `EffectWithActivity`, at
    /// its allocated Activity position.
    fn write_zero_authority_activity(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _activity: &ActivitySequences,
    ) -> impl Future<Output = Result<ZeroAuthorityWrite<Self::ZeroEffect>, ProjectCommandError>> + Send
    {
        async { Err(unavailable("the command writes no zero-authority Activity")) }
    }

    /// Decodes the effect of a zero-authority outcome from the stored effect rows.
    fn decode_zero_authority_effect(
        &self,
        _replay: &CommandReplay,
    ) -> Result<Option<Self::ZeroEffect>, ReplayFault> {
        Ok(None)
    }
}

pub(crate) type SettledCommand<C> = ProjectCommandSettlement<
    <<C as ProjectCommand>::Profile as SettlementProfile>::Applied<<C as ProjectCommand>::Effect>,
    <C as ProjectCommand>::NoEffect,
    <C as ProjectCommand>::Conflict,
    <C as ProjectCommand>::Refusal,
    <<C as ProjectCommand>::Response as ResponseRecord>::Response,
    <C as ProjectCommand>::ZeroEffect,
>;

pub(crate) async fn settle_project_command<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, ProjectCommandError> {
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
            read_command_replay(
                store,
                &envelope.challenge_binding,
                &result_reference,
                &C::SPEC.replay_effect,
            )
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
        result: match &classified {
            TransitionOutcome::Applied(_) => C::SPEC.applied_result.code(),
            TransitionOutcome::NoEffect(_)
            | TransitionOutcome::Conflicted(_)
            | TransitionOutcome::Refused(_) => classified.receipt_result().code(),
        },
        payload: match classified.reason_code() {
            Some(code) => serde_json::json!({ "reason": code }).to_string(),
            None => "{}".to_owned(),
        },
        command_kind: C::SPEC.kind,
        heads,
        refs: ReceiptRefs::default(),
    };
    let (receipt_created_at, outcome) = match classified {
        TransitionOutcome::Applied((applied, plan)) => {
            let sequences = C::Profile::allocate(client, &envelope.project_scope).await?;
            let receipt = ReceiptRecord {
                payload: command.applied_receipt_payload(&applied, &plan),
                refs: command.applied_receipt_refs(&applied, &plan),
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
            let applied =
                C::Profile::persist(client, envelope, &project, &C::SPEC, sequences, write).await?;
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
    let zero_outcome = match &outcome {
        TransitionOutcome::Applied(_) => None,
        TransitionOutcome::NoEffect(reason) => Some(ZeroOutcome::NoEffect(reason)),
        TransitionOutcome::Conflicted(reason) => Some(ZeroOutcome::Conflicted(reason)),
        TransitionOutcome::Refused(reason) => Some(ZeroOutcome::Refused(reason)),
    };
    let zero_authority_effect = match zero_outcome {
        None => None,
        Some(zero_outcome) => match command.zero_authority_rows(&zero_outcome) {
            ZeroAuthorityRows::None => None,
            ZeroAuthorityRows::Effect => Some(
                command
                    .write_zero_authority_effect(client, envelope, &zero_outcome)
                    .await?,
            ),
            ZeroAuthorityRows::EffectWithActivity => {
                let activity = ActivityOnly::allocate(client, &envelope.project_scope).await?;
                let write = command
                    .write_zero_authority_activity(client, envelope, &project, &activity)
                    .await?;
                insert_activity_payload(
                    client,
                    envelope,
                    outcome.receipt_result().code(),
                    C::SPEC.activity_kind,
                    activity.project_activity_position,
                    &activity.project_activity_event_id,
                    write.activity,
                )
                .await?;
                Some(write.effect)
            }
        },
    };
    let response = C::Response::settle(client, envelope, C::SPEC.kind).await?;
    Ok(ProjectCommandSettlement {
        ids: envelope.ids.clone(),
        receipt_created_at,
        outcome,
        response,
        zero_authority_effect,
    })
}

fn replay_command<C: ProjectCommand>(
    command: &C,
    replay: &CommandReplay,
) -> Result<SettledCommand<C>, ReplayFault> {
    let mut zero_authority_effect = None;
    let outcome = match replay
        .outcome::<C::NoEffect, C::Conflict, C::Refusal>(C::SPEC.applied_result.code())?
    {
        TransitionOutcome::Applied(()) => {
            TransitionOutcome::Applied(C::Profile::replay(command.decode(replay)?, replay)?)
        }
        TransitionOutcome::NoEffect(reason) => {
            zero_authority_effect = command.decode_zero_authority_effect(replay)?;
            TransitionOutcome::NoEffect(reason)
        }
        TransitionOutcome::Conflicted(reason) => {
            zero_authority_effect = command.decode_zero_authority_effect(replay)?;
            TransitionOutcome::Conflicted(reason)
        }
        TransitionOutcome::Refused(reason) => {
            zero_authority_effect = command.decode_zero_authority_effect(replay)?;
            TransitionOutcome::Refused(reason)
        }
    };
    Ok(ProjectCommandSettlement {
        zero_authority_effect,
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

fn challenge_problem(
    rate_limited: RateLimitedChallenge,
    error: ProjectCommandChallengeError,
) -> ProjectCommandError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ProjectCommandError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => ProjectCommandError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. } => match rate_limited {
            RateLimitedChallenge::InvalidChallenge => ProjectCommandError::InvalidChallenge,
            RateLimitedChallenge::Unavailable => unavailable(error),
        },
        ProjectCommandChallengeError::Unavailable(_) => unavailable(error),
    }
}

#[cfg(test)]
#[path = "command_sequence_tests.rs"]
pub(crate) mod tests;

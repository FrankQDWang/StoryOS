//! The fixed Author Command Admission, Core Transition, and Command Acknowledgement sequence
//! of the Manuscript Structure Transition commands (ADR 0041).

use std::future::Future;

use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeTransaction, ProjectCommandChallengeUse,
    ProjectCommandEnvelope, ProjectCommandError, StructureApplied, StructureAuthority,
    StructureAuthorityEvidence, StructureSettlement,
};
use storyos_core::{ProjectLifecycle, ReasonCode, TransitionOutcome};
use tokio_postgres::Client;

use crate::PostgresProjectReader;

mod records;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use crate::structural_authority_settlement::{
    StructureAffectedIdentity, StructureCommitBinding, allocate_structure_transition_sequences,
    persist_forward_author_action, persist_structure_commit, rebind_writer_base,
    receipt_reason_payload,
};
use records::{ReceiptRecord, insert_admission, insert_receipt, lock_project, settle_idempotency};

pub(crate) enum CommandIsolation {
    Serializable,
}

pub(crate) struct CommandSpec {
    pub(crate) kind: &'static str,
    pub(crate) isolation: CommandIsolation,
    pub(crate) activity_kind: &'static str,
}

/// The Project row that the sequence locks before a command loads its own facts.
pub(crate) struct LockedProject {
    pub(crate) lifecycle: ProjectLifecycle,
    pub(crate) tree_revision: u64,
    pub(crate) current_chapter_id: Option<String>,
}

pub(crate) enum StructureIdentity {
    Volume(String),
    Chapter(String),
    ChapterInitialRevision {
        chapter_id: String,
        revision_id: String,
    },
}

/// The Current Chapter that the transition leaves on the Project.
pub(crate) enum CurrentChapterChange {
    Preserve,
    Select(String),
    Clear,
}

/// Whether the current writer base Snapshot moves to the new canonical Snapshot.
pub(crate) enum WriterBase {
    Keep,
    RebindToCurrentChapter,
}

/// The applied writes that one command returns; the sequence writes every authority record.
pub(crate) struct StructureWrite<E> {
    pub(crate) effect: E,
    pub(crate) resulting_tree_revision: u64,
    pub(crate) identity: StructureIdentity,
    pub(crate) current_chapter: CurrentChapterChange,
    pub(crate) writer_base: WriterBase,
    /// The command fields of the Activity payload; the sequence adds `kind` and `tree_revision`.
    pub(crate) activity: serde_json::Value,
}

pub(crate) type Classified<C> = TransitionOutcome<
    <C as StructureCommand>::Applied,
    <C as StructureCommand>::NoEffect,
    <C as StructureCommand>::Conflict,
    <C as StructureCommand>::Refusal,
>;

/// One Manuscript Structure Transition command: its locked facts, its own effect rows,
/// and its applied effect decoder.
///
/// Implementations never run transaction control (ADR 0031) and never write the Admission,
/// Receipt, settlement link, Activity, Commit, Author Action, Snapshot, tree revision, or
/// idempotency rows. `settle_structure_command` writes those rows in one fixed order.
pub(crate) trait StructureCommand: Sync {
    const SPEC: CommandSpec;
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
    ) -> impl Future<Output = Result<(Classified<Self>, Self::Plan), ProjectCommandError>> + Send;

    /// The Domain Receipt payload of an applied outcome.
    fn applied_receipt_payload(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> String {
        "{}".to_owned()
    }

    /// Writes the effect rows of an applied outcome.
    fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> impl Future<Output = Result<StructureWrite<Self::Effect>, ProjectCommandError>> + Send;

    /// Decodes the applied effect from the stored acknowledgement evidence.
    fn decode(&self, replay: &CommandReplay) -> Result<Self::Effect, ReplayFault>;
}

pub(crate) type SettledStructure<C> = StructureSettlement<
    <C as StructureCommand>::Effect,
    <C as StructureCommand>::NoEffect,
    <C as StructureCommand>::Conflict,
    <C as StructureCommand>::Refusal,
>;

pub(crate) async fn settle_structure_command<C: StructureCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledStructure<C>, ProjectCommandError> {
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
                .and_then(|replay| replay_structure(command, &replay))
                .map_err(replay_error)
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

async fn first_use<C: StructureCommand>(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledStructure<C>, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let ids = &envelope.ids;
    let project = lock_project(client, envelope).await?;
    let (classified, plan) = command.classify(client, envelope, &project).await?;
    insert_admission(client, envelope, C::SPEC.kind).await?;
    let receipt = ReceiptRecord {
        result: classified.receipt_result().code(),
        payload: receipt_reason_payload(classified.reason_code()),
        command_kind: C::SPEC.kind,
    };
    let (receipt_created_at, outcome) = match classified {
        TransitionOutcome::Applied(applied) => {
            let sequences = allocate_structure_transition_sequences(client, scope)
                .await
                .map_err(ProjectCommandError::Unavailable)?;
            let receipt = ReceiptRecord {
                payload: command.applied_receipt_payload(&applied, &plan),
                ..receipt
            };
            let receipt_created_at = insert_receipt(
                client,
                envelope,
                &receipt,
                std::slice::from_ref(&sequences.authoritative_commit_id),
            )
            .await?;
            let write = command
                .apply(client, envelope, &project, plan, applied)
                .await?;
            let resulting_current = match &write.current_chapter {
                CurrentChapterChange::Preserve => project.current_chapter_id.clone(),
                CurrentChapterChange::Select(chapter_id) => Some(chapter_id.clone()),
                CurrentChapterChange::Clear => None,
            };
            let bumped = client
                .execute(
                    "UPDATE storyos.projects
                        SET tree_revision = $3::text::bigint, current_chapter_id = $5::text::uuid
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'
                        AND current_chapter_id IS NOT DISTINCT FROM $6::text::uuid",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &write.resulting_tree_revision.to_string(),
                        &project.tree_revision.to_string(),
                        &resulting_current,
                        &project.current_chapter_id,
                    ],
                )
                .await
                .map_err(unavailable)?;
            if bumped != 1 {
                return Err(unavailable(
                    "tree revision or Current Chapter changed under FOR UPDATE",
                ));
            }
            let serde_json::Value::Object(mut activity) = write.activity else {
                return Err(unavailable("the Activity payload is not an object"));
            };
            activity.insert("kind".to_owned(), C::SPEC.activity_kind.into());
            activity.insert(
                "tree_revision".to_owned(),
                write.resulting_tree_revision.to_string().into(),
            );
            client
                .execute(
                    "INSERT INTO storyos.project_activity_event_payloads
                       (owner_user_id, project_id, project_activity_position,
                        project_activity_event_id, event_kind, receipt_id, receipt_result_kind,
                        payload)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid,
                             $5, $6::text::uuid, 'authoritative_applied', $7::text::jsonb)",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &sequences.project_activity_position.to_string(),
                        &sequences.project_activity_event_id,
                        &C::SPEC.activity_kind,
                        &ids.receipt_id,
                        &serde_json::Value::Object(activity).to_string(),
                    ],
                )
                .await
                .map_err(unavailable)?;
            let (identity, resulting_revision_id) = match &write.identity {
                StructureIdentity::Volume(volume_id) => {
                    (StructureAffectedIdentity::Volume { volume_id }, None)
                }
                StructureIdentity::Chapter(chapter_id) => {
                    (StructureAffectedIdentity::Chapter { chapter_id }, None)
                }
                StructureIdentity::ChapterInitialRevision {
                    chapter_id,
                    revision_id,
                } => (
                    StructureAffectedIdentity::ChapterInitialRevision {
                        chapter_id,
                        resulting_revision_id: revision_id,
                    },
                    Some(revision_id.clone()),
                ),
            };
            persist_structure_commit(
                client,
                scope,
                &sequences,
                &ids.author_command_admission_id,
                &ids.receipt_id,
                StructureCommitBinding {
                    prior_manuscript_tree_revision: project.tree_revision,
                    resulting_manuscript_tree_revision: write.resulting_tree_revision,
                    identity,
                },
            )
            .await
            .map_err(unavailable)?;
            persist_forward_author_action(client, scope, &sequences, &ids.receipt_id)
                .await
                .map_err(unavailable)?;
            crate::snapshot::persist_canonical_snapshot(
                client,
                scope,
                &sequences.snapshot_id,
                sequences.project_activity_position,
            )
            .await
            .map_err(unavailable)?;
            if let (WriterBase::RebindToCurrentChapter, Some(chapter_id)) =
                (&write.writer_base, &resulting_current)
            {
                rebind_writer_base(
                    client,
                    scope,
                    chapter_id,
                    &sequences.snapshot_id,
                    sequences.project_activity_position,
                )
                .await
                .map_err(ProjectCommandError::Unavailable)?;
            }
            let applied = TransitionOutcome::Applied(StructureApplied {
                effect: write.effect,
                project_activity_position: sequences.project_activity_position,
                project_activity_event_id: sequences.project_activity_event_id,
                authority: StructureAuthorityEvidence::Settled(StructureAuthority {
                    authoritative_commit_id: sequences.authoritative_commit_id,
                    author_action_sequence: sequences.author_action_sequence,
                    snapshot_id: sequences.snapshot_id,
                    prior_manuscript_tree_revision: project.tree_revision,
                    resulting_manuscript_tree_revision: write.resulting_tree_revision,
                    resulting_revision_id,
                }),
            });
            (receipt_created_at, applied)
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
    let response_project = settle_idempotency(client, envelope, C::SPEC.kind).await?;
    Ok(StructureSettlement {
        ids: ids.clone(),
        receipt_created_at,
        outcome,
        response_project,
    })
}

fn replay_structure<C: StructureCommand>(
    command: &C,
    replay: &CommandReplay,
) -> Result<SettledStructure<C>, ReplayFault> {
    let outcome = match replay.outcome::<C::NoEffect, C::Conflict, C::Refusal>()? {
        TransitionOutcome::Applied(()) => TransitionOutcome::Applied(StructureApplied {
            effect: command.decode(replay)?,
            project_activity_position: replay.project_activity_position,
            project_activity_event_id: replay.project_activity_event_id.clone(),
            authority: match &replay.authority {
                Some(authority) => StructureAuthorityEvidence::Settled(StructureAuthority {
                    authoritative_commit_id: authority.authoritative_commit_id.clone(),
                    author_action_sequence: authority.author_action_sequence,
                    snapshot_id: authority.snapshot_id.clone(),
                    prior_manuscript_tree_revision: authority.prior_manuscript_tree_revision,
                    resulting_manuscript_tree_revision: authority
                        .resulting_manuscript_tree_revision,
                    resulting_revision_id: authority.resulting_revision_id.clone(),
                }),
                None => StructureAuthorityEvidence::BeforeAuthorityHistoryFloor,
            },
        }),
        TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
        TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
        TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
    };
    Ok(StructureSettlement {
        ids: replay.ids.clone(),
        receipt_created_at: replay.receipt_created_at.clone(),
        response_project: replay.response_project()?,
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

fn replay_error(fault: ReplayFault) -> ProjectCommandError {
    match fault {
        ReplayFault::BindingConflict => ProjectCommandError::BindingConflict,
        ReplayFault::HistoricalAcknowledgementUnavailable => {
            ProjectCommandError::HistoricalAcknowledgementUnavailable
        }
        ReplayFault::Unavailable(source) => ProjectCommandError::Unavailable(source),
    }
}

#[cfg(test)]
#[path = "structure_command_tests.rs"]
pub(crate) mod tests;

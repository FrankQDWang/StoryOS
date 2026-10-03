//! The fixed Author Command Admission, Core Transition, and Command Acknowledgement sequence
//! of the Manuscript Structure Transition commands (ADR 0041).

use std::future::Future;

use storyos_application::{
    ChapterId, Project, ProjectCommandChallengeError, ProjectCommandChallengeTransaction,
    ProjectCommandChallengeUse, ProjectCommandEnvelope, ProjectCommandError, StructureApplied,
    StructureAuthority, StructureAuthorityEvidence, StructureSettlement,
};
use storyos_core::{ProjectLifecycle, ReasonCode, TransitionOutcome};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};
use crate::structural_authority_settlement::{
    StructureAffectedIdentity, StructureCommitBinding, allocate_structure_transition_sequences,
    persist_forward_author_action, persist_structure_commit, receipt_reason_payload,
};

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
}

pub(crate) enum StructureIdentity {
    Volume(String),
}

/// The applied writes that one command returns; the sequence writes every authority record.
pub(crate) struct StructureWrite<E> {
    pub(crate) effect: E,
    pub(crate) resulting_tree_revision: u64,
    pub(crate) identity: StructureIdentity,
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

    /// Writes the effect rows of an applied outcome.
    fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> impl Future<Output = Result<StructureWrite<Self::Effect>, ProjectCommandError>> + Send;

    /// Decodes the applied effect from the stored acknowledgement evidence.
    fn decode(replay: &CommandReplay) -> Result<Self::Effect, ReplayFault>;
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
                .and_then(|replay| replay_structure::<C>(&replay))
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
            let receipt_created_at = insert_receipt(
                client,
                envelope,
                &receipt,
                std::slice::from_ref(&sequences.authoritative_commit_id),
            )
            .await?;
            let write = command.apply(client, envelope, plan, applied).await?;
            let bumped = client
                .execute(
                    "UPDATE storyos.projects
                        SET tree_revision = $3::text::bigint
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND tree_revision = $4::text::bigint AND lifecycle_state = 'active'",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &write.resulting_tree_revision.to_string(),
                        &project.tree_revision.to_string(),
                    ],
                )
                .await
                .map_err(unavailable)?;
            if bumped != 1 {
                return Err(unavailable("tree revision changed under FOR UPDATE"));
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
            let identity = match &write.identity {
                StructureIdentity::Volume(volume_id) => {
                    StructureAffectedIdentity::Volume { volume_id }
                }
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

struct ReceiptRecord {
    result: &'static str,
    payload: String,
    command_kind: &'static str,
}

/// Inserts the Domain Receipt and its Admission settlement link.
async fn insert_receipt(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    receipt: &ReceiptRecord,
    commit_ids: &[String],
) -> Result<String, ProjectCommandError> {
    let receipt_created_at = client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $11, $6, $7::text::uuid,
                     'author_command_admission', '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::uuid[], $10::text[]::uuid[], '{}'::text[], '{}'::text[],
                     '{}'::text[], $8, $9::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.ids.author_command_admission_id,
                &envelope.ids.command_id,
                &envelope.challenge_binding.canonical_command_digest,
                &envelope.challenge_binding.idempotency_key,
                &receipt.result,
                &receipt.payload,
                &commit_ids,
                &receipt.command_kind,
            ],
        )
        .await
        .map_err(unavailable)?
        .get::<_, String>(0);
    client
        .execute(
            "INSERT INTO storyos.author_command_admission_settlements
               (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid,
                     'receipt_settled', $4::text::uuid)",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.author_command_admission_id,
                &envelope.ids.receipt_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(receipt_created_at)
}

fn replay_structure<C: StructureCommand>(
    replay: &CommandReplay,
) -> Result<SettledStructure<C>, ReplayFault> {
    let outcome = match replay.outcome::<C::NoEffect, C::Conflict, C::Refusal>()? {
        TransitionOutcome::Applied(()) => TransitionOutcome::Applied(StructureApplied {
            effect: C::decode(replay)?,
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

async fn lock_project(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<LockedProject, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT lifecycle_state, tree_revision::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
              FOR UPDATE",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(unavailable)?
        .ok_or(ProjectCommandError::MissingProject)?;
    let lifecycle = match row.get::<_, String>(0).as_str() {
        "active" => ProjectLifecycle::Active,
        "archived" => ProjectLifecycle::Archived,
        other => {
            return Err(unavailable(format!(
                "unsupported Project lifecycle {other}"
            )));
        }
    };
    Ok(LockedProject {
        lifecycle,
        tree_revision: row.get::<_, String>(1).parse().map_err(unavailable)?,
    })
}

async fn insert_admission(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command_kind: &str,
) -> Result<(), ProjectCommandError> {
    let binding = &envelope.client_binding;
    let challenge = &envelope.challenge_binding;
    let inserted = client
        .execute(
            "INSERT INTO storyos.author_command_admissions
               (owner_user_id, project_id, author_command_admission_id, command_id,
                editor_session_id, writer_generation, client_session_binding_ref,
                client_session_generation, client_contract_revision, security_policy_revision,
                action_class, method, route_template, command_schema, command_kind,
                canonical_command_digest, idempotency_key, challenge_consumed_at,
                challenge_expires_at, correlation_id, chapter_object_id,
                expected_authoritative_revision_id, expected_proposal_head_revision_ids,
                target_refs, observed_ownership_partition, editor_contract_revision,
                undo_group_id, completed_intent_record_id, local_intent_sequence, command_payload)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                    NULL, NULL, $5, $6::text::numeric, $7, $8,
                    'explicit_project_command', $9, $10, $11, $16,
                    $12, $13::text::uuid, challenge.consumed_at, challenge.expires_at,
                    $14::text::uuid, NULL, NULL, '{}'::uuid[], '{}'::text[], NULL, $7,
                    NULL, NULL, NULL, convert_from($15::bytea, 'UTF8')::jsonb
               FROM storyos.project_command_challenges AS challenge
              WHERE challenge.owner_user_id = $1::text::uuid
                AND challenge.project_id = $2::text::uuid
                AND challenge.command_kind = $16
                AND challenge.idempotency_key = $13::text::uuid",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.author_command_admission_id,
                &envelope.ids.command_id,
                &binding.binding_ref,
                &binding.session_generation.to_string(),
                &binding.client_contract_revision,
                &binding.security_policy_revision,
                &challenge.method,
                &challenge.route_template,
                &challenge.command_schema,
                &challenge.canonical_command_digest,
                &challenge.idempotency_key,
                &envelope.correlation_id,
                &envelope.canonical_command_bytes.as_slice(),
                &command_kind,
            ],
        )
        .await
        .map_err(unavailable)?;
    if inserted != 1 {
        return Err(ProjectCommandError::InvalidChallenge);
    }
    Ok(())
}

/// Reads the Command-response Project after the writes and settles the Command Idempotency Fence.
async fn settle_idempotency(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command_kind: &str,
) -> Result<Project, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let row = client
        .query_one(
            "SELECT title, current_chapter_id::text FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?;
    let project = Project {
        project_id: scope.project_id.clone(),
        title: row.get(0),
        current_chapter_id: row.get::<_, Option<String>>(1).map(ChapterId::new),
    };
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled',
                    result_reference = $3,
                    acknowledgement_format = $5,
                    response_project = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = $7 AND idempotency_key = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.challenge_binding.idempotency_key,
                &COMMAND_RESPONSE_PROJECT_FORMAT,
                &encode_command_response_project(&project),
                &command_kind,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(project)
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
mod tests;

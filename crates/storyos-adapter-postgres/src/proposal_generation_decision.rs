use storyos_application::{
    ChapterId, CompleteReadyPartialProposalCommand, CompleteReadyPartialProposalEffect,
    ContinueProposalGenerationCommand, ContinueProposalGenerationEffect, Project,
    ProjectCommandChallengeError, ProjectCommandChallengeUse, ProposalGenerationDecisionError,
    ProposalGenerationDecisionStore, ProposalGenerationSettlement,
};
use storyos_core::{
    CompleteReadyPartialProposal, CompleteReadyPartialProposalResult, ContinueProposalGeneration,
    ContinueProposalGenerationResult, complete_ready_partial_proposal as classify_complete,
    continue_proposal_generation as classify_continue, hex_sha256,
};
use uuid::Uuid;

use super::*;
use crate::author_edit::parse_u64;
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};

pub(super) struct LoadedGeneration {
    revision_id: String,
    generation_state: String,
    validation: String,
    closure: String,
    operation_resolution: String,
    candidate_text: String,
    generation_id: String,
    last_seq: u64,
    source_run_id: String,
    run_status: String,
    chapter_id: String,
    head_revision_id: Option<String>,
    run_agent_id: String,
    conversation_id: String,
    memory_settings_revision: String,
    grant_id: String,
    binding_revision: String,
    author_message: String,
}

impl ProposalGenerationDecisionStore for PostgresProjectReader {
    async fn complete_ready_partial_proposal(
        &self,
        command: &CompleteReadyPartialProposalCommand,
    ) -> Result<
        ProposalGenerationSettlement<CompleteReadyPartialProposalEffect>,
        ProposalGenerationDecisionError,
    > {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(challenge_error)?;
        match transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(challenge_error)?
        {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction.rollback().await.map_err(challenge_error)?;
                read::read_complete_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction.rollback().await.map_err(challenge_error)?;
                Err(ProposalGenerationDecisionError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_complete(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction.rollback().await.map_err(challenge_error)?;
                        Err(error)
                    }
                }
            }
        }
    }

    async fn continue_proposal_generation(
        &self,
        command: &ContinueProposalGenerationCommand,
    ) -> Result<
        ProposalGenerationSettlement<ContinueProposalGenerationEffect>,
        ProposalGenerationDecisionError,
    > {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(challenge_error)?;
        match transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(challenge_error)?
        {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction.rollback().await.map_err(challenge_error)?;
                read::read_continue_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction.rollback().await.map_err(challenge_error)?;
                Err(ProposalGenerationDecisionError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_continue(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction.rollback().await.map_err(challenge_error)?;
                        Err(error)
                    }
                }
            }
        }
    }
}

async fn persist_complete(
    client: &tokio_postgres::Client,
    command: &CompleteReadyPartialProposalCommand,
) -> Result<
    ProposalGenerationSettlement<CompleteReadyPartialProposalEffect>,
    ProposalGenerationDecisionError,
> {
    let Some(loaded) =
        load_generation(client, &command.project_scope, &command.proposal_id).await?
    else {
        return Err(ProposalGenerationDecisionError::MissingProject);
    };
    write::insert_admission(
        client,
        &command.project_scope,
        &command.client_binding,
        &command.challenge_binding,
        &command.ids,
        &command.editor_session_id,
        &command.correlation_id,
        &loaded.chapter_id,
        &command.expected_authoritative_revision_id,
        &command.canonical_command_bytes,
        "completeReadyPartialProposal",
    )
    .await?;
    let classified = classify_complete(&CompleteReadyPartialProposal {
        revision_current: loaded.revision_id == command.proposal_revision_id,
        closure_open: loaded.closure == "open",
        generation_state: loaded.generation_state.clone(),
        generation_id_matches: loaded.generation_id == command.generation_id,
        candidate_digest_matches: hex_sha256(loaded.candidate_text.as_bytes())
            == command.expected_candidate_digest,
        stream_seq_matches: loaded.last_seq == command.last_applied_stream_seq,
        expected_target_matches_head: loaded.head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        CompleteReadyPartialProposalResult::Completed => {
            let sequence = next_author_action(client, &command.project_scope).await?;
            let updated = client
                .execute(
                    "UPDATE storyos.proposal_revisions
                        SET generation = 'ready'
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND proposal_id = $3::text::uuid AND revision_id = $4::text::uuid
                        AND generation = 'ready_partial'",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &command.proposal_id,
                        &loaded.revision_id,
                    ],
                )
                .await
                .map_err(database_error)?;
            if updated != 1 {
                return Err(ProposalGenerationDecisionError::BindingConflict);
            }
            let event_id = Uuid::now_v7().to_string();
            let receipt = command_receipt(
                &command.project_scope,
                &command.challenge_binding,
                &command.ids,
                &command.expected_authoritative_revision_id,
            );
            let created_at = write::insert_receipt(
                client,
                &receipt,
                "completeReadyPartialProposal",
                "proposal_generation_completed",
                r#"{"transition":"generation_completed"}"#,
                Some(sequence),
            )
            .await?;
            write::insert_transition(
                client,
                &command.project_scope,
                &event_id,
                &command.proposal_id,
                &loaded,
                &loaded.generation_id,
                "ready",
                &loaded.source_run_id,
                &command.ids.receipt_id,
                sequence,
            )
            .await?;
            let project = settle_idempotency(
                client,
                &command.project_scope,
                "completeReadyPartialProposal",
                &command.challenge_binding.idempotency_key,
                &command.ids.receipt_id,
            )
            .await?;
            Ok(ProposalGenerationSettlement {
                ids: command.ids.clone(),
                effect: CompleteReadyPartialProposalEffect::Completed {
                    author_action_sequence: sequence,
                    generation_id: loaded.generation_id,
                    preserved_validation: loaded.validation,
                    preserved_closure: loaded.closure,
                    preserved_operation_resolution: loaded.operation_resolution,
                    generation_event_id: event_id,
                },
                receipt_created_at: created_at,
                response_project: project,
            })
        }
        CompleteReadyPartialProposalResult::Conflicted { reason } => {
            zero_effect(
                client,
                command_receipt(
                    &command.project_scope,
                    &command.challenge_binding,
                    &command.ids,
                    &command.expected_authoritative_revision_id,
                ),
                "completeReadyPartialProposal",
                "conflicted",
                "changed_head",
                CompleteReadyPartialProposalEffect::Conflicted { reason },
            )
            .await
        }
        CompleteReadyPartialProposalResult::Refused { reason } => {
            zero_effect(
                client,
                command_receipt(
                    &command.project_scope,
                    &command.challenge_binding,
                    &command.ids,
                    &command.expected_authoritative_revision_id,
                ),
                "completeReadyPartialProposal",
                "refused",
                complete_reason(&reason),
                CompleteReadyPartialProposalEffect::Refused { reason },
            )
            .await
        }
    }
}

async fn persist_continue(
    client: &tokio_postgres::Client,
    command: &ContinueProposalGenerationCommand,
) -> Result<
    ProposalGenerationSettlement<ContinueProposalGenerationEffect>,
    ProposalGenerationDecisionError,
> {
    let Some(loaded) =
        load_generation(client, &command.project_scope, &command.proposal_id).await?
    else {
        return Err(ProposalGenerationDecisionError::MissingProject);
    };
    write::insert_admission(
        client,
        &command.project_scope,
        &command.client_binding,
        &command.challenge_binding,
        &command.ids,
        &command.editor_session_id,
        &command.correlation_id,
        &loaded.chapter_id,
        &command.expected_authoritative_revision_id,
        &command.canonical_command_bytes,
        "continueProposalGeneration",
    )
    .await?;
    let pending = selected_pending(
        client,
        &command.project_scope,
        &command.proposal_id,
        &command.selected_pending_operation_ids,
    )
    .await?;
    let classified = classify_continue(&ContinueProposalGeneration {
        revision_current: loaded.revision_id == command.proposal_revision_id,
        closure_open: loaded.closure == "open",
        generation_state: loaded.generation_state.clone(),
        expected_generation_state: command.expected_generation_state.clone(),
        generation_id_matches: loaded.generation_id == command.prior_generation_id,
        candidate_digest_matches: hex_sha256(loaded.candidate_text.as_bytes())
            == command.expected_candidate_digest,
        selected_operations_pending: pending,
        selection_duplicate_free: command
            .selected_pending_operation_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == command.selected_pending_operation_ids.len(),
        expected_target_matches_head: loaded.head_revision_id.as_deref()
            == Some(command.expected_authoritative_revision_id.as_str()),
    });
    match classified {
        ContinueProposalGenerationResult::Started => {
            let sequence = next_author_action(client, &command.project_scope).await?;
            let terminal = matches!(
                loaded.run_status.as_str(),
                "completed" | "refused" | "cancelled"
            );
            let resulting_run_id = if terminal {
                Uuid::now_v7().to_string()
            } else {
                loaded.source_run_id.clone()
            };
            let new_generation_id = Uuid::now_v7().to_string();
            let event_id = Uuid::now_v7().to_string();
            let receipt = command_receipt(
                &command.project_scope,
                &command.challenge_binding,
                &command.ids,
                &command.expected_authoritative_revision_id,
            );
            let created_at = write::insert_receipt(
                client,
                &receipt,
                "continueProposalGeneration",
                "proposal_generation_started",
                r#"{"transition":"generation_started"}"#,
                Some(sequence),
            )
            .await?;
            if terminal {
                write::insert_successor_run(
                    client,
                    &command.project_scope,
                    &loaded,
                    &resulting_run_id,
                    &command.ids.receipt_id,
                )
                .await?;
            }
            client
                .execute(
                    "INSERT INTO storyos.proposal_generations
                       (owner_user_id, project_id, generation_id, proposal_id,
                        last_applied_stream_seq, run_id)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                             0, $5::text::uuid)",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &new_generation_id,
                        &command.proposal_id,
                        &resulting_run_id,
                    ],
                )
                .await
                .map_err(database_error)?;
            client
                .execute(
                    "UPDATE storyos.proposal_generation_heads
                        SET generation_id = $4::text::uuid
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND proposal_id = $3::text::uuid",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &command.proposal_id,
                        &new_generation_id,
                    ],
                )
                .await
                .map_err(database_error)?;
            let updated = client
                .execute(
                    "UPDATE storyos.proposal_revisions
                        SET generation = 'generating'
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND proposal_id = $3::text::uuid AND revision_id = $4::text::uuid
                        AND generation = $5",
                    &[
                        &command.project_scope.owner_user_id.as_ref(),
                        &command.project_scope.project_id.as_ref(),
                        &command.proposal_id,
                        &loaded.revision_id,
                        &loaded.generation_state,
                    ],
                )
                .await
                .map_err(database_error)?;
            if updated != 1 {
                return Err(ProposalGenerationDecisionError::BindingConflict);
            }
            write::insert_transition(
                client,
                &command.project_scope,
                &event_id,
                &command.proposal_id,
                &loaded,
                &new_generation_id,
                "generating",
                &resulting_run_id,
                &command.ids.receipt_id,
                sequence,
            )
            .await?;
            let project = settle_idempotency(
                client,
                &command.project_scope,
                "continueProposalGeneration",
                &command.challenge_binding.idempotency_key,
                &command.ids.receipt_id,
            )
            .await?;
            Ok(ProposalGenerationSettlement {
                ids: command.ids.clone(),
                effect: ContinueProposalGenerationEffect::Started {
                    author_action_sequence: sequence,
                    prior_generation_id: loaded.generation_id,
                    new_generation_id,
                    prior_generation_state: loaded.generation_state,
                    prior_run_id: loaded.source_run_id,
                    resulting_run_id,
                    preserved_validation: loaded.validation,
                    preserved_closure: loaded.closure,
                    preserved_operation_resolution: loaded.operation_resolution,
                    generation_event_id: event_id,
                },
                receipt_created_at: created_at,
                response_project: project,
            })
        }
        ContinueProposalGenerationResult::Conflicted { reason } => {
            zero_effect(
                client,
                command_receipt(
                    &command.project_scope,
                    &command.challenge_binding,
                    &command.ids,
                    &command.expected_authoritative_revision_id,
                ),
                "continueProposalGeneration",
                "conflicted",
                "changed_head",
                ContinueProposalGenerationEffect::Conflicted { reason },
            )
            .await
        }
        ContinueProposalGenerationResult::Refused { reason } => {
            zero_effect(
                client,
                command_receipt(
                    &command.project_scope,
                    &command.challenge_binding,
                    &command.ids,
                    &command.expected_authoritative_revision_id,
                ),
                "continueProposalGeneration",
                "refused",
                continue_reason(&reason),
                ContinueProposalGenerationEffect::Refused { reason },
            )
            .await
        }
    }
}

pub(super) struct ReceiptBinding<'a> {
    scope: &'a storyos_application::ProjectScope,
    challenge: &'a storyos_application::ProjectCommandChallengeBinding,
    ids: &'a storyos_application::AuthorCommandAdmissionIds,
    head: &'a str,
}

fn command_receipt<'a>(
    scope: &'a storyos_application::ProjectScope,
    challenge: &'a storyos_application::ProjectCommandChallengeBinding,
    ids: &'a storyos_application::AuthorCommandAdmissionIds,
    head: &'a str,
) -> ReceiptBinding<'a> {
    ReceiptBinding {
        scope,
        challenge,
        ids,
        head,
    }
}

async fn zero_effect<T>(
    client: &tokio_postgres::Client,
    binding: ReceiptBinding<'_>,
    command_kind: &str,
    result_kind: &str,
    reason: &str,
    effect: T,
) -> Result<ProposalGenerationSettlement<T>, ProposalGenerationDecisionError> {
    let payload = serde_json::json!({ "reason": reason }).to_string();
    let created_at =
        write::insert_receipt(client, &binding, command_kind, result_kind, &payload, None).await?;
    let project = settle_idempotency(
        client,
        binding.scope,
        command_kind,
        &binding.challenge.idempotency_key,
        &binding.ids.receipt_id,
    )
    .await?;
    Ok(ProposalGenerationSettlement {
        ids: binding.ids.clone(),
        effect,
        receipt_created_at: created_at,
        response_project: project,
    })
}

#[path = "proposal_generation_decision_read.rs"]
mod read;

#[path = "proposal_generation_decision_write.rs"]
mod write;

async fn load_generation(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    proposal_id: &str,
) -> Result<Option<LoadedGeneration>, ProposalGenerationDecisionError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                    revision.closure, operation.resolution,
                    revision.candidate_text, generation.generation_id::text,
                    generation.last_applied_stream_seq, proposal.source_run_id::text,
                    run.status, proposal.chapter_id::text, chapter_head.current_revision_id::text,
                    run.project_agent_id::text, run.conversation_id::text,
                    run.memory_settings_revision::text, run.grant_id::text,
                    run.project_model_use_binding_revision::text, run.author_message
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               JOIN storyos.proposal_generation_heads AS generation_head
                 ON (generation_head.owner_user_id, generation_head.project_id,
                     generation_head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_generations AS generation
                 ON (generation.owner_user_id, generation.project_id, generation.generation_id) =
                    (generation_head.owner_user_id, generation_head.project_id,
                     generation_head.generation_id)
               JOIN storyos.proposal_operations AS operation
                 ON (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id, run.run_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.source_run_id)
               LEFT JOIN storyos.authoritative_heads AS chapter_head
                 ON (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.proposal_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(row.map(|row| LoadedGeneration {
        revision_id: row.get(0),
        generation_state: row.get(1),
        validation: row.get(2),
        closure: row.get(3),
        operation_resolution: row.get(4),
        candidate_text: row.get(5),
        generation_id: row.get(6),
        last_seq: u64::try_from(row.get::<_, i64>(7)).unwrap_or(0),
        source_run_id: row.get(8),
        run_status: row.get(9),
        chapter_id: row.get(10),
        head_revision_id: row.get(11),
        run_agent_id: row.get(12),
        conversation_id: row.get(13),
        memory_settings_revision: row.get(14),
        grant_id: row.get(15),
        binding_revision: row.get(16),
        author_message: row.get(17),
    }))
}

async fn selected_pending(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    proposal_id: &str,
    selected: &[String],
) -> Result<bool, ProposalGenerationDecisionError> {
    let rows = client
        .query(
            "SELECT operation_id::text
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND resolution = 'pending'
                AND operation_id = ANY($4::text[]::uuid[])",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
                &selected,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(rows.len() == selected.len())
}

async fn next_author_action(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
) -> Result<u64, ProposalGenerationDecisionError> {
    let row = client
        .query_one(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = author_action_sequence + 1
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
          RETURNING author_action_sequence::text",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(database_error)?;
    parse_u64(row.get(0)).map_err(parse_error)
}

async fn settle_idempotency(
    client: &tokio_postgres::Client,
    scope: &storyos_application::ProjectScope,
    command_kind: &str,
    idempotency_key: &str,
    receipt_id: &str,
) -> Result<Project, ProposalGenerationDecisionError> {
    let row = client
        .query_opt(
            "SELECT title, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Err(ProposalGenerationDecisionError::MissingProject);
    };
    let response_project = Project {
        project_id: scope.project_id.clone(),
        title: row.get(0),
        current_chapter_id: row.get::<_, Option<String>>(1).map(ChapterId::new),
    };
    client
        .execute(
            "UPDATE storyos.command_idempotency
                SET outcome_kind = 'settled', result_reference = $3,
                    acknowledgement_format = $5, response_project = $6::text::jsonb
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND command_kind = $7 AND idempotency_key = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &receipt_id,
                &idempotency_key,
                &COMMAND_RESPONSE_PROJECT_FORMAT,
                &encode_command_response_project(&response_project),
                &command_kind,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(response_project)
}

fn complete_reason(reason: &storyos_core::CompleteReadyPartialProposalRefusal) -> &'static str {
    match reason {
        storyos_core::CompleteReadyPartialProposalRefusal::StaleProposalRevision => {
            "stale_proposal_revision"
        }
        storyos_core::CompleteReadyPartialProposalRefusal::NotEligible => "not_eligible",
        storyos_core::CompleteReadyPartialProposalRefusal::NotReadyPartial => "not_ready_partial",
        storyos_core::CompleteReadyPartialProposalRefusal::StaleGeneration => "stale_generation",
        storyos_core::CompleteReadyPartialProposalRefusal::StaleCandidate => "stale_candidate",
    }
}

fn continue_reason(reason: &storyos_core::ContinueProposalGenerationRefusal) -> &'static str {
    match reason {
        storyos_core::ContinueProposalGenerationRefusal::StaleProposalRevision => {
            "stale_proposal_revision"
        }
        storyos_core::ContinueProposalGenerationRefusal::NotEligible => "not_eligible",
        storyos_core::ContinueProposalGenerationRefusal::NotContinuable => "not_continuable",
        storyos_core::ContinueProposalGenerationRefusal::StaleGeneration => "stale_generation",
        storyos_core::ContinueProposalGenerationRefusal::StaleCandidate => "stale_candidate",
        storyos_core::ContinueProposalGenerationRefusal::OperationNotPending => {
            "operation_not_pending"
        }
        storyos_core::ContinueProposalGenerationRefusal::DuplicateIdentities => {
            "duplicate_identities"
        }
    }
}

pub(super) fn challenge_error(
    error: ProjectCommandChallengeError,
) -> ProposalGenerationDecisionError {
    match error {
        ProjectCommandChallengeError::BindingConflict => {
            ProposalGenerationDecisionError::BindingConflict
        }
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => {
            ProposalGenerationDecisionError::InvalidChallenge
        }
        ProjectCommandChallengeError::Unavailable(source) => {
            ProposalGenerationDecisionError::Unavailable(source)
        }
    }
}

pub(super) fn database_error(error: tokio_postgres::Error) -> ProposalGenerationDecisionError {
    ProposalGenerationDecisionError::Unavailable(Box::new(error))
}

pub(super) fn parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> ProposalGenerationDecisionError {
    ProposalGenerationDecisionError::Unavailable(Box::new(error))
}

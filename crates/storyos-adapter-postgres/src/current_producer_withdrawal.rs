//! The current-producer form of Withdraw Proposal. It is an AgentRun decision without a Command
//! Challenge or an Admission, so it stays outside the command sequence (ADR 0043).

use storyos_application::{
    AuthorCommandAdmissionIds, ChapterId, CurrentProducerWithdrawal,
    CurrentProducerWithdrawalSettlement, Project, ProjectCommandChallengeError,
    ProjectCommandEnvelope, ProjectCommandError, ProposalWithdrawn,
};
use storyos_core::{
    TransitionOutcome, WithdrawProposal as CoreWithdraw, WithdrawProposalConflict,
    WithdrawProposalNoEffect, WithdrawProposalRefusal, WithdrawalCause, withdraw_proposal,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::unavailable;

impl PostgresProjectReader {
    /// Settles one Withdrawal by the AgentRun decision that produced the Proposal.
    ///
    /// An exact retry with the same idempotency key returns the first settlement.
    pub async fn withdraw_proposal_as_current_producer(
        &self,
        envelope: &ProjectCommandEnvelope,
        withdrawal: &CurrentProducerWithdrawal,
    ) -> Result<CurrentProducerWithdrawalSettlement, ProjectCommandError> {
        let transaction = self
            .begin_serializable_project_command_transaction(&envelope.project_scope)
            .await
            .map_err(challenge_error)?;
        let outcome = async {
            if let Some(existing) = read_producer_receipt(&transaction.client, envelope).await? {
                return Ok(existing);
            }
            persist_withdrawal(&transaction.client, envelope, withdrawal).await
        }
        .await;
        match outcome {
            Ok(settlement) => {
                transaction.commit().await.map_err(challenge_error)?;
                Ok(settlement)
            }
            Err(ProjectCommandError::BindingConflict) => {
                transaction.rollback().await.map_err(challenge_error)?;
                let replay = self
                    .begin_serializable_project_command_transaction(&envelope.project_scope)
                    .await
                    .map_err(challenge_error)?;
                let replayed = read_producer_receipt(&replay.client, envelope).await;
                replay.rollback().await.map_err(challenge_error)?;
                match replayed {
                    Ok(Some(settlement)) => Ok(settlement),
                    Ok(None) => Err(ProjectCommandError::BindingConflict),
                    Err(error) => Err(error),
                }
            }
            Err(error) => {
                transaction.rollback().await.map_err(challenge_error)?;
                Err(error)
            }
        }
    }
}

async fn persist_withdrawal(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    withdrawal: &CurrentProducerWithdrawal,
) -> Result<CurrentProducerWithdrawalSettlement, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                    revision.closure, proposal.chapter_id::text,
                    chapter_head.current_revision_id::text,
                    proposal.source_run_id::text, proposal.source_decision_id::text
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
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
                &withdrawal.proposal_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .ok_or(ProjectCommandError::MissingProject)?;
    if row.get::<_, Option<String>>(/*idx*/ 4).is_none() {
        return Err(ProjectCommandError::MissingProject);
    }
    let closure = row.get::<_, String>(/*idx*/ 3);
    let classified = withdraw_proposal(&CoreWithdraw {
        scope_matches: true,
        cause: WithdrawalCause::CurrentProducer,
        admission_valid: false,
        producer_matches: row.get::<_, Option<String>>(/*idx*/ 6).as_deref()
            == Some(withdrawal.run_id.as_str())
            && row.get::<_, Option<String>>(/*idx*/ 7).as_deref()
                == Some(withdrawal.decision_id.as_str()),
        proposal_revision_current: row.get::<_, String>(/*idx*/ 0)
            == withdrawal.proposal_revision_id,
        closure_open: closure == "open",
        terminal_supersession: closure == "superseded",
        expected_target_matches_head: row.get::<_, Option<String>>(/*idx*/ 5).as_deref()
            == Some(withdrawal.expected_authoritative_revision_id.as_str()),
    });
    let zero_result_kind = classified.receipt_result().code();
    let zero_payload = serde_json::json!({ "reason": classified.reason_code() }).to_string();
    let (receipt_created_at, outcome) = match classified {
        TransitionOutcome::Applied(()) => {
            let updated = client
                .execute(
                    "UPDATE storyos.proposal_revisions
                        SET closure = 'withdrawn'
                      WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                        AND proposal_id = $3::text::uuid AND revision_id = $4::text::uuid
                        AND closure = 'open'",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &withdrawal.proposal_id,
                        &withdrawal.proposal_revision_id,
                    ],
                )
                .await
                .map_err(unavailable)?;
            if updated != 1 {
                return Err(ProjectCommandError::BindingConflict);
            }
            let withdrawal_event_id = Uuid::now_v7().to_string();
            let receipt_created_at = insert_receipt(
                client,
                envelope,
                withdrawal,
                "proposal_closure_changed",
                r#"{"transition":"withdraw"}"#,
            )
            .await?;
            client
                .execute(
                    "INSERT INTO storyos.proposal_withdrawals
                       (owner_user_id, project_id, withdrawal_event_id, proposal_id,
                        proposal_revision_id, withdrawal_reason, author_note,
                        withdrawal_receipt_id, author_action_sequence)
                     VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                             $5::text::uuid, 'current_producer_withdrew', NULL, $6::text::uuid,
                             NULL)",
                    &[
                        &scope.owner_user_id.as_ref(),
                        &scope.project_id.as_ref(),
                        &withdrawal_event_id,
                        &withdrawal.proposal_id,
                        &withdrawal.proposal_revision_id,
                        &envelope.ids.receipt_id,
                    ],
                )
                .await
                .map_err(unavailable)?;
            (
                receipt_created_at,
                TransitionOutcome::Applied(ProposalWithdrawn {
                    preserved_generation: row.get(/*idx*/ 1),
                    preserved_validation: row.get(/*idx*/ 2),
                    withdrawal_event_id,
                }),
            )
        }
        TransitionOutcome::NoEffect(reason) => (
            insert_receipt(
                client,
                envelope,
                withdrawal,
                zero_result_kind,
                &zero_payload,
            )
            .await?,
            TransitionOutcome::NoEffect(reason),
        ),
        TransitionOutcome::Conflicted(reason) => (
            insert_receipt(
                client,
                envelope,
                withdrawal,
                zero_result_kind,
                &zero_payload,
            )
            .await?,
            TransitionOutcome::Conflicted(reason),
        ),
        TransitionOutcome::Refused(reason) => (
            insert_receipt(
                client,
                envelope,
                withdrawal,
                zero_result_kind,
                &zero_payload,
            )
            .await?,
            TransitionOutcome::Refused(reason),
        ),
    };
    let project = client
        .query_opt(
            "SELECT title, current_chapter_id::text
               FROM storyos.projects
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?
        .ok_or(ProjectCommandError::MissingProject)?;
    Ok(CurrentProducerWithdrawalSettlement {
        ids: envelope.ids.clone(),
        receipt_created_at,
        outcome,
        response: Project {
            project_id: scope.project_id.clone(),
            title: project.get(/*idx*/ 0),
            current_chapter_id: project
                .get::<_, Option<String>>(/*idx*/ 1)
                .map(ChapterId::new),
        },
        zero_authority_effect: None,
    })
}

/// Inserts the Domain Receipt of one current-producer Withdrawal. It has no Admission.
async fn insert_receipt(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    withdrawal: &CurrentProducerWithdrawal,
    result_kind: &str,
    result_payload: &str,
) -> Result<String, ProjectCommandError> {
    Ok(client
        .query_one(
            "INSERT INTO storyos.domain_receipts
               (owner_user_id, project_id, receipt_id, author_command_admission_id,
                command_id, command_kind, command_digest, idempotency_key, producer_cause,
                expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
                proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
                artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, NULL,
                     $4::text::uuid, 'withdrawProposal', $5, $6::text::uuid,
                     'agent_run_decision', ARRAY[$7::text::uuid], ARRAY[$7::text::uuid],
                     ARRAY[$7::text::uuid], '{}'::uuid[], '{}'::uuid[],
                     '{}'::uuid[], '{}'::text[], '{}'::text[], '{}'::text[],
                     $8, $9::text::jsonb)
          RETURNING to_char(created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &envelope.ids.receipt_id,
                &envelope.ids.command_id,
                &envelope.challenge_binding.canonical_command_digest,
                &envelope.challenge_binding.idempotency_key,
                &withdrawal.expected_authoritative_revision_id,
                &result_kind,
                &result_payload,
            ],
        )
        .await
        .map_err(|error| {
            if error.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                ProjectCommandError::BindingConflict
            } else {
                unavailable(error)
            }
        })?
        .get::<_, String>(/*idx*/ 0))
}

/// Reads the settlement of an earlier current-producer Withdrawal with the same idempotency key.
async fn read_producer_receipt(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<Option<CurrentProducerWithdrawalSettlement>, ProjectCommandError> {
    let scope = &envelope.project_scope;
    let Some(row) = client
        .query_opt(
            "SELECT receipt.command_id::text, receipt.receipt_id::text, receipt.command_digest,
                    receipt.result_kind, receipt.result_payload->>'reason',
                    to_char(receipt.created_at AT TIME ZONE 'UTC',
                            'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                    withdrawal.withdrawal_event_id::text, revision.generation, revision.validation,
                    project.title, project.current_chapter_id::text
               FROM storyos.domain_receipts AS receipt
               JOIN storyos.projects AS project
                 ON (project.owner_user_id, project.project_id) =
                    (receipt.owner_user_id, receipt.project_id)
          LEFT JOIN storyos.proposal_withdrawals AS withdrawal
                 ON (withdrawal.owner_user_id, withdrawal.project_id,
                     withdrawal.withdrawal_receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
          LEFT JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (withdrawal.owner_user_id, withdrawal.project_id,
                     withdrawal.proposal_id, withdrawal.proposal_revision_id)
              WHERE receipt.owner_user_id = $1::text::uuid
                AND receipt.project_id = $2::text::uuid
                AND receipt.command_kind = 'withdrawProposal'
                AND receipt.producer_cause = 'agent_run_decision'
                AND receipt.idempotency_key = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &envelope.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(unavailable)?
    else {
        return Ok(None);
    };
    if row.get::<_, String>(/*idx*/ 2) != envelope.challenge_binding.canonical_command_digest {
        return Err(ProjectCommandError::BindingConflict);
    }
    let result_kind = row.get::<_, String>(/*idx*/ 3);
    let outcome = if result_kind == "proposal_closure_changed" {
        match (
            row.get::<_, Option<String>>(/*idx*/ 6),
            row.get::<_, Option<String>>(/*idx*/ 7),
            row.get::<_, Option<String>>(/*idx*/ 8),
        ) {
            (Some(withdrawal_event_id), Some(preserved_generation), Some(preserved_validation)) => {
                TransitionOutcome::Applied(ProposalWithdrawn {
                    preserved_generation,
                    preserved_validation,
                    withdrawal_event_id,
                })
            }
            _ => return Err(ProjectCommandError::HistoricalAcknowledgementUnavailable),
        }
    } else {
        row.get::<_, Option<String>>(/*idx*/ 4)
            .and_then(|reason| {
                TransitionOutcome::<
                    ProposalWithdrawn,
                    WithdrawProposalNoEffect,
                    WithdrawProposalConflict,
                    WithdrawProposalRefusal,
                >::from_zero_authority_codes(&result_kind, &reason)
            })
            .ok_or(ProjectCommandError::HistoricalAcknowledgementUnavailable)?
    };
    Ok(Some(CurrentProducerWithdrawalSettlement {
        ids: AuthorCommandAdmissionIds {
            command_id: row.get(/*idx*/ 0),
            author_command_admission_id: String::new(),
            receipt_id: row.get(/*idx*/ 1),
        },
        receipt_created_at: row.get(/*idx*/ 5),
        outcome,
        response: Project {
            project_id: scope.project_id.clone(),
            title: row.get(/*idx*/ 9),
            current_chapter_id: row.get::<_, Option<String>>(/*idx*/ 10).map(ChapterId::new),
        },
        zero_authority_effect: None,
    }))
}

fn challenge_error(error: ProjectCommandChallengeError) -> ProjectCommandError {
    match error {
        ProjectCommandChallengeError::BindingConflict => ProjectCommandError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => ProjectCommandError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(source) => {
            ProjectCommandError::Unavailable(source)
        }
    }
}

use crate::command_response_project::{
    CommandResponseProjectEvidence, read_command_response_project,
};
use storyos_application::{
    AuthorCommandAdmissionIds, ProjectCommandChallengeError, ProjectCommandChallengeUse,
    UpdateChapterAuthority, UpdateChapterCommand, UpdateChapterError, UpdateChapterSettlement,
    UpdateChapterSettlementEffect, UpdateChapterStore,
};

use super::*;

mod persist;
use persist::persist_update_chapter;

impl UpdateChapterStore for PostgresProjectReader {
    async fn update_chapter(
        &self,
        command: &UpdateChapterCommand,
    ) -> Result<UpdateChapterSettlement, UpdateChapterError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(update_chapter_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(update_chapter_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(update_chapter_challenge_error)?;
                read_update_chapter_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(update_chapter_challenge_error)?;
                Err(UpdateChapterError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_update_chapter(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(update_chapter_challenge_error)?;
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
}

async fn read_update_chapter_settlement(
    store: &PostgresProjectReader,
    command: &UpdateChapterCommand,
    receipt_id: &str,
) -> Result<UpdateChapterSettlement, UpdateChapterError> {
    let client = store
        .connect_challenge()
        .await
        .map_err(update_chapter_challenge_error)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(update_chapter_database_error)?;
    let result = async {
        set_challenge_scope_on_client(&client, &command.project_scope)
            .await
            .map_err(update_chapter_challenge_error)?;
        let row = client
            .query_opt(
                "SELECT receipt.command_id::text,
                        receipt.author_command_admission_id::text,
                        receipt.receipt_id::text,
                        to_char(receipt.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        receipt.result_kind,
                        receipt.result_payload->>'reason',
                        payload.payload->>'tree_revision',
                        payload.payload->>'title',
                        payload.payload->>'order',
                        payload.project_activity_position::text,
                        payload.project_activity_event_id::text,
                        authoritative_commit.authoritative_commit_id::text,
                        action.author_action_sequence::text,
                        snapshot.snapshot_id::text,
                        authoritative_commit.prior_manuscript_tree_revision::text,
                        authoritative_commit.resulting_manuscript_tree_revision::text,
                        idempotency.acknowledgement_format,
                        idempotency.response_project::text
                   FROM storyos.domain_receipts AS receipt
                   JOIN storyos.author_command_admission_settlements AS settlement
                     ON (settlement.owner_user_id, settlement.project_id,
                         settlement.author_command_admission_id, settlement.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id,
                         receipt.author_command_admission_id, receipt.receipt_id)
                   JOIN storyos.command_idempotency AS idempotency
                     ON (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key,
                         idempotency.result_reference) =
                        (receipt.owner_user_id, receipt.project_id, receipt.command_kind,
                         receipt.idempotency_key, receipt.receipt_id::text)
              LEFT JOIN storyos.project_activity_event_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.authoritative_commits AS authoritative_commit
                     ON (authoritative_commit.owner_user_id, authoritative_commit.project_id,
                         authoritative_commit.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.author_action_entries AS action
                     ON (action.owner_user_id, action.project_id, action.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
              LEFT JOIN storyos.project_snapshots AS snapshot
                     ON (snapshot.owner_user_id, snapshot.project_id,
                         snapshot.project_activity_position) =
                        (payload.owner_user_id, payload.project_id,
                         payload.project_activity_position)
                    AND snapshot.snapshot_kind = 'canonical'
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $3::text::uuid
                    AND receipt.command_kind = 'updateChapter'
                    AND receipt.command_digest = $4
                    AND receipt.idempotency_key = $5::text::uuid
                    AND settlement.settlement_kind = 'receipt_settled'
                    AND idempotency.outcome_kind = 'settled'",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &receipt_id,
                    &command.challenge_binding.canonical_command_digest,
                    &command.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(update_chapter_database_error)?
            .ok_or(UpdateChapterError::BindingConflict)?;
        let result_kind = row.get::<_, String>(4);
        let reason = row.get::<_, Option<String>>(5);
        let effect = match (result_kind.as_str(), reason.as_deref()) {
            ("authoritative_applied", None) => {
                let tree_revision = row
                    .get::<_, Option<String>>(6)
                    .ok_or(UpdateChapterError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(update_chapter_parse_error)?;
                let title = row
                    .get::<_, Option<String>>(7)
                    .ok_or(UpdateChapterError::BindingConflict)?;
                let order = row
                    .get::<_, Option<String>>(8)
                    .ok_or(UpdateChapterError::BindingConflict)?
                    .parse::<u64>()
                    .map_err(update_chapter_parse_error)?;
                UpdateChapterSettlementEffect::Applied {
                    title,
                    order,
                    tree_revision,
                }
            }
            ("no_effect", Some("unchanged")) => UpdateChapterSettlementEffect::NoEffect {
                reason: storyos_core::UpdateChapterNoEffect::Unchanged,
            },
            ("conflicted", Some("stale_tree_revision")) => {
                UpdateChapterSettlementEffect::Conflicted {
                    reason: storyos_core::UpdateChapterConflict::StaleTreeRevision,
                }
            }
            ("refused", Some("archived_project")) => UpdateChapterSettlementEffect::Refused {
                reason: storyos_core::UpdateChapterRefusal::ArchivedProject,
            },
            ("refused", Some("invalid_title")) => UpdateChapterSettlementEffect::Refused {
                reason: storyos_core::UpdateChapterRefusal::InvalidTitle,
            },
            ("refused", Some("invalid_order")) => UpdateChapterSettlementEffect::Refused {
                reason: storyos_core::UpdateChapterRefusal::InvalidOrder,
            },
            ("refused", Some("invalid_chapter_join")) => UpdateChapterSettlementEffect::Refused {
                reason: storyos_core::UpdateChapterRefusal::InvalidChapterJoin,
            },
            _ => return Err(UpdateChapterError::BindingConflict),
        };
        let authority = match (
            row.get::<_, Option<String>>(11),
            row.get::<_, Option<String>>(12),
            row.get::<_, Option<String>>(13),
            row.get::<_, Option<String>>(14),
            row.get::<_, Option<String>>(15),
        ) {
            (
                Some(authoritative_commit_id),
                Some(author_action_sequence),
                Some(snapshot_id),
                Some(prior_manuscript_tree_revision),
                Some(resulting_manuscript_tree_revision),
            ) => Some(UpdateChapterAuthority {
                authoritative_commit_id,
                author_action_sequence: author_action_sequence
                    .parse()
                    .map_err(update_chapter_parse_error)?,
                snapshot_id,
                prior_manuscript_tree_revision: prior_manuscript_tree_revision
                    .parse()
                    .map_err(update_chapter_parse_error)?,
                resulting_manuscript_tree_revision: resulting_manuscript_tree_revision
                    .parse()
                    .map_err(update_chapter_parse_error)?,
            }),
            _ => None,
        };
        let response_project = match read_command_response_project(
            row.get::<_, Option<String>>(16).as_deref(),
            row.get::<_, Option<String>>(17).as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => project,
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                return Err(UpdateChapterError::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => {
                return Err(UpdateChapterError::Unavailable(Box::new(
                    std::io::Error::other("Update Chapter acknowledgement evidence is damaged"),
                )));
            }
        };
        Ok(UpdateChapterSettlement {
            ids: AuthorCommandAdmissionIds {
                command_id: row.get(0),
                author_command_admission_id: row.get(1),
                receipt_id: row.get(2),
            },
            receipt_created_at: row.get(3),
            effect,
            project_activity_position: row
                .get::<_, Option<String>>(9)
                .unwrap_or_else(|| "0".to_owned())
                .parse::<u64>()
                .map_err(update_chapter_parse_error)?,
            project_activity_event_id: row.get::<_, Option<String>>(10).unwrap_or_default(),
            authority,
            response_project,
        })
    }
    .await;
    match &result {
        Ok(_) => client
            .batch_execute("COMMIT")
            .await
            .map_err(update_chapter_database_error)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    result
}

pub(super) fn update_chapter_challenge_error(
    error: ProjectCommandChallengeError,
) -> UpdateChapterError {
    match error {
        ProjectCommandChallengeError::BindingConflict => UpdateChapterError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => UpdateChapterError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            UpdateChapterError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn update_chapter_database_error(error: tokio_postgres::Error) -> UpdateChapterError {
    UpdateChapterError::Unavailable(Box::new(error))
}

pub(super) fn update_chapter_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> UpdateChapterError {
    UpdateChapterError::Unavailable(Box::new(error))
}

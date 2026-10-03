use storyos_application::{
    DeleteChapterAuthority, DeleteChapterCommand, DeleteChapterError, DeleteChapterSettlement,
    DeleteChapterSettlementEffect, DeleteChapterStore, ProjectCommandChallengeError,
    ProjectCommandChallengeUse,
};
use storyos_core::DeleteChapterCurrent;

use super::*;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use storyos_core::TransitionOutcome;

mod persist;
use persist::persist_delete_chapter;

impl DeleteChapterStore for PostgresProjectReader {
    async fn delete_chapter(
        &self,
        command: &DeleteChapterCommand,
    ) -> Result<DeleteChapterSettlement, DeleteChapterError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(delete_chapter_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(delete_chapter_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(delete_chapter_challenge_error)?;
                read_delete_chapter_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(delete_chapter_challenge_error)?;
                Err(DeleteChapterError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_delete_chapter(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(delete_chapter_challenge_error)?;
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

async fn read_delete_chapter_settlement(
    store: &PostgresProjectReader,
    command: &DeleteChapterCommand,
    receipt_id: &str,
) -> Result<DeleteChapterSettlement, DeleteChapterError> {
    read_command_replay(store, &command.challenge_binding, receipt_id)
        .await
        .and_then(|replay| delete_chapter_replay(replay, command))
        .map_err(|fault| match fault {
            ReplayFault::BindingConflict => DeleteChapterError::BindingConflict,
            ReplayFault::HistoricalAcknowledgementUnavailable => {
                DeleteChapterError::HistoricalAcknowledgementUnavailable
            }
            ReplayFault::Unavailable(source) => DeleteChapterError::Unavailable(source),
        })
}

fn delete_chapter_replay(
    replay: CommandReplay,
    command: &DeleteChapterCommand,
) -> Result<DeleteChapterSettlement, ReplayFault> {
    let effect = match replay.outcome()? {
        TransitionOutcome::Applied(()) => DeleteChapterSettlementEffect::Applied {
            tree_revision: replay.activity_u64("tree_revision")?,
            volume_id: replay.activity_text("volume_id")?,
            current: match (
                replay
                    .activity_optional_text("prior_current_chapter_id")
                    .as_deref(),
                replay.activity_optional_text("current_chapter_id"),
            ) {
                // Prior Current that is not the removed Chapter stayed
                // in place. Resulting Current alone cannot name that.
                (Some(prior), _) if prior != command.chapter_id.as_ref() => {
                    DeleteChapterCurrent::PreserveExisting
                }
                (_, None) => DeleteChapterCurrent::Empty,
                (_, Some(chapter_id)) => DeleteChapterCurrent::SelectSuccessor { chapter_id },
            },
        },
        TransitionOutcome::NoEffect(reason) => DeleteChapterSettlementEffect::NoEffect { reason },
        TransitionOutcome::Conflicted(reason) => {
            DeleteChapterSettlementEffect::Conflicted { reason }
        }
        TransitionOutcome::Refused(reason) => DeleteChapterSettlementEffect::Refused { reason },
    };
    let response_project = replay.response_project()?;
    Ok(DeleteChapterSettlement {
        authority: replay.authority.map(|authority| DeleteChapterAuthority {
            authoritative_commit_id: authority.authoritative_commit_id,
            author_action_sequence: authority.author_action_sequence,
            snapshot_id: authority.snapshot_id,
            prior_manuscript_tree_revision: authority.prior_manuscript_tree_revision,
            resulting_manuscript_tree_revision: authority.resulting_manuscript_tree_revision,
        }),
        ids: replay.ids,
        receipt_created_at: replay.receipt_created_at,
        effect,
        project_activity_position: replay.project_activity_position,
        project_activity_event_id: replay.project_activity_event_id,
        response_project,
    })
}

pub(super) fn delete_chapter_challenge_error(
    error: ProjectCommandChallengeError,
) -> DeleteChapterError {
    match error {
        ProjectCommandChallengeError::BindingConflict => DeleteChapterError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => DeleteChapterError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            DeleteChapterError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn delete_chapter_database_error(error: tokio_postgres::Error) -> DeleteChapterError {
    DeleteChapterError::Unavailable(Box::new(error))
}

pub(super) fn delete_chapter_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> DeleteChapterError {
    DeleteChapterError::Unavailable(Box::new(error))
}

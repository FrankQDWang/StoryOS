use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, UpdateChapterAuthority,
    UpdateChapterCommand, UpdateChapterError, UpdateChapterSettlement,
    UpdateChapterSettlementEffect, UpdateChapterStore,
};

use super::*;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use storyos_core::TransitionOutcome;

mod persist;
pub(super) mod sibling_order;
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
    read_command_replay(store, &command.challenge_binding, receipt_id)
        .await
        .and_then(update_chapter_replay)
        .map_err(|fault| match fault {
            ReplayFault::BindingConflict => UpdateChapterError::BindingConflict,
            ReplayFault::HistoricalAcknowledgementUnavailable => {
                UpdateChapterError::HistoricalAcknowledgementUnavailable
            }
            ReplayFault::Unavailable(source) => UpdateChapterError::Unavailable(source),
        })
}

fn update_chapter_replay(replay: CommandReplay) -> Result<UpdateChapterSettlement, ReplayFault> {
    let effect = match replay.outcome()? {
        TransitionOutcome::Applied(()) => UpdateChapterSettlementEffect::Applied {
            title: replay.activity_text("title")?,
            order: replay.activity_u64("order")?,
            tree_revision: replay.activity_u64("tree_revision")?,
        },
        TransitionOutcome::NoEffect(reason) => UpdateChapterSettlementEffect::NoEffect { reason },
        TransitionOutcome::Conflicted(reason) => {
            UpdateChapterSettlementEffect::Conflicted { reason }
        }
        TransitionOutcome::Refused(reason) => UpdateChapterSettlementEffect::Refused { reason },
    };
    let response_project = replay.response_project()?;
    Ok(UpdateChapterSettlement {
        authority: replay.authority.map(|authority| UpdateChapterAuthority {
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

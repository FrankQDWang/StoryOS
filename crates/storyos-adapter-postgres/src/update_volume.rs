use storyos_application::{
    ProjectCommandChallengeError, ProjectCommandChallengeUse, UpdateVolumeAuthority,
    UpdateVolumeCommand, UpdateVolumeError, UpdateVolumeSettlement, UpdateVolumeSettlementEffect,
    UpdateVolumeStore,
};

use super::*;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use storyos_core::TransitionOutcome;

mod persist;
use persist::persist_update_volume;

impl UpdateVolumeStore for PostgresProjectReader {
    async fn update_volume(
        &self,
        command: &UpdateVolumeCommand,
    ) -> Result<UpdateVolumeSettlement, UpdateVolumeError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(update_volume_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(update_volume_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(update_volume_challenge_error)?;
                read_update_volume_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(update_volume_challenge_error)?;
                Err(UpdateVolumeError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_update_volume(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(update_volume_challenge_error)?;
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

async fn read_update_volume_settlement(
    store: &PostgresProjectReader,
    command: &UpdateVolumeCommand,
    receipt_id: &str,
) -> Result<UpdateVolumeSettlement, UpdateVolumeError> {
    read_command_replay(store, &command.challenge_binding, receipt_id)
        .await
        .and_then(update_volume_replay)
        .map_err(|fault| match fault {
            ReplayFault::BindingConflict => UpdateVolumeError::BindingConflict,
            ReplayFault::HistoricalAcknowledgementUnavailable => {
                UpdateVolumeError::HistoricalAcknowledgementUnavailable
            }
            ReplayFault::Unavailable(source) => UpdateVolumeError::Unavailable(source),
        })
}

fn update_volume_replay(replay: CommandReplay) -> Result<UpdateVolumeSettlement, ReplayFault> {
    let effect = match replay.outcome()? {
        TransitionOutcome::Applied(()) => UpdateVolumeSettlementEffect::Applied {
            title: replay.activity_text("title")?,
            order: replay.activity_u64("order")?,
            tree_revision: replay.activity_u64("tree_revision")?,
        },
        TransitionOutcome::NoEffect(reason) => UpdateVolumeSettlementEffect::NoEffect { reason },
        TransitionOutcome::Conflicted(reason) => {
            UpdateVolumeSettlementEffect::Conflicted { reason }
        }
        TransitionOutcome::Refused(reason) => UpdateVolumeSettlementEffect::Refused { reason },
    };
    let response_project = replay.response_project()?;
    Ok(UpdateVolumeSettlement {
        authority: replay.authority.map(|authority| UpdateVolumeAuthority {
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

pub(super) fn update_volume_challenge_error(
    error: ProjectCommandChallengeError,
) -> UpdateVolumeError {
    match error {
        ProjectCommandChallengeError::BindingConflict => UpdateVolumeError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => UpdateVolumeError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            UpdateVolumeError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn update_volume_database_error(error: tokio_postgres::Error) -> UpdateVolumeError {
    UpdateVolumeError::Unavailable(Box::new(error))
}

pub(super) fn update_volume_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> UpdateVolumeError {
    UpdateVolumeError::Unavailable(Box::new(error))
}

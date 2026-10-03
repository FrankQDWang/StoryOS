use super::*;
use crate::command_replay::{CommandReplay, ReplayFault, read_command_replay};
use storyos_application::{
    DeleteVolumeAuthority, DeleteVolumeCommand, DeleteVolumeError, DeleteVolumeSettlement,
    DeleteVolumeSettlementEffect, DeleteVolumeStore, ProjectCommandChallengeError,
    ProjectCommandChallengeUse,
};
use storyos_core::TransitionOutcome;

mod persist;
use persist::persist_delete_volume;

impl DeleteVolumeStore for PostgresProjectReader {
    async fn delete_volume(
        &self,
        command: &DeleteVolumeCommand,
    ) -> Result<DeleteVolumeSettlement, DeleteVolumeError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(delete_volume_challenge_error)?;
        let challenge_use = transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
            .map_err(delete_volume_challenge_error)?;
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(delete_volume_challenge_error)?;
                read_delete_volume_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(delete_volume_challenge_error)?;
                Err(DeleteVolumeError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_delete_volume(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction
                            .commit()
                            .await
                            .map_err(delete_volume_challenge_error)?;
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

async fn read_delete_volume_settlement(
    store: &PostgresProjectReader,
    command: &DeleteVolumeCommand,
    receipt_id: &str,
) -> Result<DeleteVolumeSettlement, DeleteVolumeError> {
    read_command_replay(store, &command.challenge_binding, receipt_id)
        .await
        .and_then(delete_volume_replay)
        .map_err(|fault| match fault {
            ReplayFault::BindingConflict => DeleteVolumeError::BindingConflict,
            ReplayFault::HistoricalAcknowledgementUnavailable => {
                DeleteVolumeError::HistoricalAcknowledgementUnavailable
            }
            ReplayFault::Unavailable(source) => DeleteVolumeError::Unavailable(source),
        })
}

fn delete_volume_replay(replay: CommandReplay) -> Result<DeleteVolumeSettlement, ReplayFault> {
    let effect = match replay.outcome()? {
        TransitionOutcome::Applied(()) => DeleteVolumeSettlementEffect::Applied {
            tree_revision: replay.activity_u64("tree_revision")?,
            volume_id: replay.activity_text("volume_id")?,
        },
        TransitionOutcome::NoEffect(reason) => DeleteVolumeSettlementEffect::NoEffect { reason },
        TransitionOutcome::Conflicted(reason) => {
            DeleteVolumeSettlementEffect::Conflicted { reason }
        }
        TransitionOutcome::Refused(reason) => DeleteVolumeSettlementEffect::Refused { reason },
    };
    let response_project = replay.response_project()?;
    Ok(DeleteVolumeSettlement {
        authority: replay.authority.map(|authority| DeleteVolumeAuthority {
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

pub(super) fn delete_volume_challenge_error(
    error: ProjectCommandChallengeError,
) -> DeleteVolumeError {
    match error {
        ProjectCommandChallengeError::BindingConflict => DeleteVolumeError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired => DeleteVolumeError::InvalidChallenge,
        ProjectCommandChallengeError::RateLimited { .. }
        | ProjectCommandChallengeError::Unavailable(_) => {
            DeleteVolumeError::Unavailable(Box::new(error))
        }
    }
}

pub(super) fn delete_volume_database_error(error: tokio_postgres::Error) -> DeleteVolumeError {
    DeleteVolumeError::Unavailable(Box::new(error))
}

pub(super) fn delete_volume_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> DeleteVolumeError {
    DeleteVolumeError::Unavailable(Box::new(error))
}

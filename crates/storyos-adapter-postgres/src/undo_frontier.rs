use storyos_application::ProjectCommandError;
use storyos_core::AuthorUndoFrontierKind;

use crate::accept_proposal::{AcceptanceCompensation, LoadedAcceptance};
use crate::author_edit::{ObservedProseFrontier, ProseCompensation};
use crate::author_edit_proposal::{ObservedProposalFrontier, ProposalEditCompensation};
use crate::close_editor_flow_draft::{DraftCompensation, ObservedDraftClose};
use crate::proposal_decision_compensation::ObservedProposalDecision;
use crate::reopen_rejected_operations::{ObservedOperationReopening, ReopenRejectedCompensation};
use crate::reopen_withdrawn_proposal::ReopenWithdrawnCompensation;
use crate::replan_proposal::ReplanCompensation;
use crate::set_current_chapter::{CurrentChapterCompensation, ObservedCurrentChapterFrontier};
use crate::structural_authority_settlement::{ObservedStructureFrontier, StructureCompensation};
use crate::undo_compensation::{CompensationAdapter, ForwardCommand, UndoDisposition, UndoRequest};
use crate::withdraw_proposal::{AuthorWithdrawalCompensation, ObservedAuthorWithdrawal};

pub(crate) enum ObservedFrontier {
    Acceptance(LoadedAcceptance),
    Prose(ObservedProseFrontier),
    Structure(ObservedStructureFrontier),
    CurrentChapter(ObservedCurrentChapterFrontier),
    Proposal(ObservedProposalFrontier),
    Replan(ObservedProposalDecision),
    ReopenRejectedOperations(ObservedOperationReopening),
    ReopenWithdrawnProposal(ObservedProposalDecision),
    AuthorWithdrawal(ObservedAuthorWithdrawal),
    DraftClose(ObservedDraftClose),
    Barrier { sequence: u64 },
}

pub(crate) async fn load_observed_frontier(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
) -> Result<Option<ObservedFrontier>, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT frontier.author_action_sequence::text,
                    frontier.command_kind,
                    frontier.result_kind
               FROM storyos.projects AS project
          LEFT JOIN LATERAL (
                SELECT action.author_action_sequence,
                       receipt.command_kind,
                       receipt.result_kind
                  FROM storyos.author_action_entries AS action
                  JOIN storyos.domain_receipts AS receipt
                    ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id)
             LEFT JOIN storyos.author_action_entries AS compensation
                    ON compensation.owner_user_id = action.owner_user_id
                   AND compensation.project_id = action.project_id
                   AND compensation.disposition = 'compensation'
                   AND compensation.compensated_source_sequence = action.author_action_sequence
                 WHERE action.owner_user_id = project.owner_user_id
                   AND action.project_id = project.project_id
                   AND action.disposition = 'forward'
                   AND compensation.author_action_sequence IS NULL
              ORDER BY action.author_action_sequence DESC
                 LIMIT 1
          ) AS frontier ON true
              WHERE project.owner_user_id = $1::text::uuid
                AND project.project_id = $2::text::uuid
              FOR UPDATE OF project",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let Some(row) = row else {
        return Err(ProjectCommandError::MissingProject);
    };
    let Some(sequence) = row.get::<_, Option<String>>(/*idx*/ 0) else {
        return Ok(None);
    };
    let sequence = sequence.parse().map_err(undo_parse_error)?;
    let forward = match (
        row.get::<_, Option<String>>(/*idx*/ 1),
        row.get::<_, Option<String>>(/*idx*/ 2),
    ) {
        (Some(command_kind), Some(result_kind)) => {
            ForwardCommand::from_receipt(&command_kind, &result_kind)
        }
        (None, _) | (_, None) => None,
    };
    let disposition = forward.map_or(UndoDisposition::Barrier, ForwardCommand::disposition);
    let observed = match disposition {
        UndoDisposition::Prose => ProseCompensation::load(client, command, (), sequence)
            .await?
            .map(ObservedFrontier::Prose),
        UndoDisposition::Acceptance => AcceptanceCompensation::load(client, command, (), sequence)
            .await?
            .map(ObservedFrontier::Acceptance),
        UndoDisposition::ProposalEdit => {
            ProposalEditCompensation::load(client, command, (), sequence)
                .await?
                .map(ObservedFrontier::Proposal)
        }
        UndoDisposition::Replan => ReplanCompensation::load(client, command, (), sequence)
            .await?
            .map(ObservedFrontier::Replan),
        UndoDisposition::ReopenRejectedOperations => {
            ReopenRejectedCompensation::load(client, command, (), sequence)
                .await?
                .map(ObservedFrontier::ReopenRejectedOperations)
        }
        UndoDisposition::ReopenWithdrawnProposal => {
            ReopenWithdrawnCompensation::load(client, command, (), sequence)
                .await?
                .map(ObservedFrontier::ReopenWithdrawnProposal)
        }
        UndoDisposition::Structure(forward) => {
            StructureCompensation::load(client, command, forward, sequence)
                .await?
                .map(ObservedFrontier::Structure)
        }
        UndoDisposition::CurrentChapter => {
            CurrentChapterCompensation::load(client, command, (), sequence)
                .await?
                .map(ObservedFrontier::CurrentChapter)
        }
        UndoDisposition::AuthorWithdrawal => {
            AuthorWithdrawalCompensation::load(client, command, (), sequence)
                .await?
                .map(ObservedFrontier::AuthorWithdrawal)
        }
        UndoDisposition::Draft => DraftCompensation::load(client, command, (), sequence)
            .await?
            .map(ObservedFrontier::DraftClose),
        UndoDisposition::Barrier => None,
    };
    Ok(Some(
        observed.unwrap_or(ObservedFrontier::Barrier { sequence }),
    ))
}

impl ObservedFrontier {
    pub(crate) fn sequence(&self) -> u64 {
        match self {
            Self::Acceptance(frontier) => frontier.sequence,
            Self::Prose(frontier) => frontier.sequence,
            Self::Structure(frontier) => frontier.sequence,
            Self::CurrentChapter(frontier) => frontier.sequence,
            Self::Proposal(frontier) => frontier.sequence,
            Self::Replan(frontier) | Self::ReopenWithdrawnProposal(frontier) => frontier.sequence,
            Self::ReopenRejectedOperations(frontier) => frontier.decision.sequence,
            Self::AuthorWithdrawal(frontier) => frontier.sequence,
            Self::DraftClose(frontier) => frontier.sequence,
            Self::Barrier { sequence } => *sequence,
        }
    }

    pub(crate) fn kind(&self) -> AuthorUndoFrontierKind {
        match self {
            Self::Acceptance(frontier) => AcceptanceCompensation::frontier_kind(frontier),
            Self::Prose(frontier) => ProseCompensation::frontier_kind(frontier),
            Self::Structure(frontier) => StructureCompensation::frontier_kind(frontier),
            Self::CurrentChapter(frontier) => CurrentChapterCompensation::frontier_kind(frontier),
            Self::Proposal(frontier) => ProposalEditCompensation::frontier_kind(frontier),
            Self::Replan(frontier) => ReplanCompensation::frontier_kind(frontier),
            Self::ReopenRejectedOperations(frontier) => {
                ReopenRejectedCompensation::frontier_kind(frontier)
            }
            Self::ReopenWithdrawnProposal(frontier) => {
                ReopenWithdrawnCompensation::frontier_kind(frontier)
            }
            Self::AuthorWithdrawal(frontier) => {
                AuthorWithdrawalCompensation::frontier_kind(frontier)
            }
            Self::DraftClose(frontier) => DraftCompensation::frontier_kind(frontier),
            Self::Barrier { .. } => AuthorUndoFrontierKind::Barrier,
        }
    }

    pub(crate) fn prose_head(&self) -> Option<&str> {
        match self {
            Self::Acceptance(frontier) => Some(frontier.current_head_revision_id.as_str()),
            Self::Prose(frontier) => Some(frontier.current_head_revision_id.as_str()),
            Self::Structure(_)
            | Self::CurrentChapter(_)
            | Self::Proposal(_)
            | Self::Replan(_)
            | Self::ReopenRejectedOperations(_)
            | Self::ReopenWithdrawnProposal(_)
            | Self::AuthorWithdrawal(_)
            | Self::Barrier { .. } => None,
            Self::DraftClose(frontier) => Some(frontier.current_head_revision_id.as_str()),
        }
    }
}

fn undo_database_error(error: tokio_postgres::Error) -> ProjectCommandError {
    ProjectCommandError::Unavailable(Box::new(error))
}

fn undo_parse_error(error: impl std::error::Error + Send + Sync + 'static) -> ProjectCommandError {
    ProjectCommandError::Unavailable(Box::new(error))
}

pub(crate) async fn editor_session_chapter(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
) -> Result<Option<String>, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT snapshot.chapter_object_id::text
               FROM storyos.editor_session_base_snapshots AS snapshot
              WHERE snapshot.owner_user_id = $1::text::uuid
                AND snapshot.project_id = $2::text::uuid
                AND snapshot.editor_session_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.editor_session_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    Ok(row.map(|row| row.get(/*idx*/ 0)))
}

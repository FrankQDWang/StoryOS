use storyos_application::{UndoLatestAuthorActionCommand, UndoLatestAuthorActionError};
use storyos_core::AuthorUndoFrontierKind;

use crate::author_edit_proposal::{ObservedProposalFrontier, ProposalEditCompensation};
use crate::close_editor_flow_draft::{DraftCompensation, ObservedDraftClose};
use crate::set_current_chapter::{CurrentChapterCompensation, ObservedCurrentChapterFrontier};
use crate::structural_authority_settlement::{ObservedStructureFrontier, StructureCompensation};
use crate::undo_compensation::{CompensationAdapter, ForwardCommand, UndoDisposition};
use crate::withdraw_proposal::{AuthorWithdrawalCompensation, ObservedAuthorWithdrawal};

pub(super) enum ObservedFrontier {
    Acceptance(crate::undo_acceptance::LoadedAcceptance),
    Prose(ObservedProseFrontier),
    Structure(ObservedStructureFrontier),
    CurrentChapter(ObservedCurrentChapterFrontier),
    Proposal(ObservedProposalFrontier),
    AuthorWithdrawal(ObservedAuthorWithdrawal),
    DraftClose(ObservedDraftClose),
    Barrier { sequence: u64 },
}

pub(super) struct ObservedProseFrontier {
    pub sequence: u64,
    pub chapter_id: String,
    pub resulting_revision_id: String,
    pub prior_revision_id: String,
    pub prior_payload: String,
    pub current_head_revision_id: String,
}

pub(super) struct LoadedUndoFrontier {
    pub lifecycle_state: String,
    pub observed: Option<ObservedFrontier>,
}

pub(super) async fn load_observed_frontier(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<LoadedUndoFrontier, UndoLatestAuthorActionError> {
    let row = client
        .query_opt(
            "SELECT project.lifecycle_state,
                    frontier.author_action_sequence::text,
                    frontier.manuscript_object_id::text,
                    frontier.resulting_revision_id::text,
                    frontier.prior_revision_id::text,
                    convert_from(prior_payload.canonical_bytes, 'UTF8'),
                    head.current_revision_id::text,
                    frontier.command_kind,
                    frontier.result_kind
               FROM storyos.projects AS project
          LEFT JOIN LATERAL (
                SELECT action.author_action_sequence,
                       commit.manuscript_object_id,
                       commit.resulting_revision_id,
                       commit.prior_revision_id,
                       receipt.command_kind,
                       receipt.result_kind
                  FROM storyos.author_action_entries AS action
                  JOIN storyos.domain_receipts AS receipt
                    ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id)
             LEFT JOIN storyos.authoritative_commits AS commit
                    ON (commit.owner_user_id, commit.project_id, commit.receipt_id,
                        commit.receipt_result_kind, commit.authoritative_commit_id) =
                       (action.owner_user_id, action.project_id, action.receipt_id,
                        action.receipt_result_kind, action.authoritative_commit_id)
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
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (project.owner_user_id, project.project_id, frontier.manuscript_object_id)
          LEFT JOIN storyos.authoritative_revisions AS prior_revision
                 ON (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.manuscript_object_id, prior_revision.revision_id) =
                    (project.owner_user_id, project.project_id,
                     frontier.manuscript_object_id, frontier.prior_revision_id)
          LEFT JOIN storyos.authoritative_payloads AS prior_payload
                 ON (prior_payload.owner_user_id, prior_payload.project_id,
                     prior_payload.payload_id) =
                    (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.payload_id)
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
        return Err(UndoLatestAuthorActionError::MissingProject);
    };
    let lifecycle_state = row.get(/*idx*/ 0);
    let Some(sequence) = row.get::<_, Option<String>>(/*idx*/ 1) else {
        return Ok(LoadedUndoFrontier {
            lifecycle_state,
            observed: None,
        });
    };
    let sequence = sequence.parse().map_err(undo_parse_error)?;
    let forward = match (
        row.get::<_, Option<String>>(/*idx*/ 7),
        row.get::<_, Option<String>>(/*idx*/ 8),
    ) {
        (Some(command_kind), Some(result_kind)) => {
            ForwardCommand::from_receipt(&command_kind, &result_kind)
        }
        (None, _) | (_, None) => None,
    };
    let disposition = forward.map_or(UndoDisposition::Barrier, ForwardCommand::disposition);
    let observed = match disposition {
        UndoDisposition::Prose => prose_frontier(&row, sequence).map(ObservedFrontier::Prose),
        UndoDisposition::Acceptance => match acceptance_frontier(&row, sequence) {
            Some(pending) => Some(ObservedFrontier::Acceptance(
                crate::undo_acceptance::enrich(client, command, pending).await?,
            )),
            None => None,
        },
        UndoDisposition::ProposalEdit => {
            ProposalEditCompensation::load(client, command, (), sequence)
                .await?
                .map(ObservedFrontier::Proposal)
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
    Ok(LoadedUndoFrontier {
        lifecycle_state,
        observed: Some(observed.unwrap_or(ObservedFrontier::Barrier { sequence })),
    })
}

fn prose_frontier(row: &tokio_postgres::Row, sequence: u64) -> Option<ObservedProseFrontier> {
    match (
        row.get(/*idx*/ 2),
        row.get(/*idx*/ 3),
        row.get(/*idx*/ 4),
        row.get(/*idx*/ 5),
        row.get(/*idx*/ 6),
    ) {
        (
            Some(chapter_id),
            Some(resulting_revision_id),
            Some(prior_revision_id),
            Some(prior_payload),
            Some(current_head_revision_id),
        ) => Some(ObservedProseFrontier {
            sequence,
            chapter_id,
            resulting_revision_id,
            prior_revision_id,
            prior_payload,
            current_head_revision_id,
        }),
        _ => None,
    }
}

fn acceptance_frontier(
    row: &tokio_postgres::Row,
    sequence: u64,
) -> Option<crate::undo_acceptance::LoadedAcceptance> {
    match (
        row.get(/*idx*/ 2),
        row.get(/*idx*/ 3),
        row.get(/*idx*/ 4),
        row.get(/*idx*/ 6),
    ) {
        (
            Some(chapter_id),
            Some(resulting_revision_id),
            Some(prior_revision_id),
            Some(current_head_revision_id),
        ) => Some(crate::undo_acceptance::LoadedAcceptance::pending(
            sequence,
            chapter_id,
            resulting_revision_id,
            prior_revision_id,
            row.get(/*idx*/ 5),
            current_head_revision_id,
        )),
        _ => None,
    }
}

impl ObservedFrontier {
    pub(super) fn sequence(&self) -> u64 {
        match self {
            Self::Acceptance(frontier) => frontier.sequence,
            Self::Prose(frontier) => frontier.sequence,
            Self::Structure(frontier) => frontier.sequence,
            Self::CurrentChapter(frontier) => frontier.sequence,
            Self::Proposal(frontier) => frontier.sequence,
            Self::AuthorWithdrawal(frontier) => frontier.sequence,
            Self::DraftClose(frontier) => frontier.sequence,
            Self::Barrier { sequence } => *sequence,
        }
    }

    pub(super) fn kind(&self) -> AuthorUndoFrontierKind {
        match self {
            Self::Acceptance(frontier) => AuthorUndoFrontierKind::ReversibleAcceptance {
                resulting_revision_id: frontier.resulting_revision_id.clone(),
                prior_evidence_usable: frontier.prior_evidence_usable,
            },
            Self::Prose(frontier) => AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
                resulting_revision_id: frontier.resulting_revision_id.clone(),
            },
            Self::Structure(frontier) => StructureCompensation::frontier_kind(frontier),
            Self::CurrentChapter(frontier) => CurrentChapterCompensation::frontier_kind(frontier),
            Self::Proposal(frontier) => ProposalEditCompensation::frontier_kind(frontier),
            Self::AuthorWithdrawal(frontier) => {
                AuthorWithdrawalCompensation::frontier_kind(frontier)
            }
            Self::DraftClose(frontier) => DraftCompensation::frontier_kind(frontier),
            Self::Barrier { .. } => AuthorUndoFrontierKind::Barrier,
        }
    }

    pub(super) fn prose_head(&self) -> Option<&str> {
        match self {
            Self::Acceptance(frontier) => Some(frontier.current_head_revision_id.as_str()),
            Self::Prose(frontier) => Some(frontier.current_head_revision_id.as_str()),
            Self::Structure(_)
            | Self::CurrentChapter(_)
            | Self::Proposal(_)
            | Self::AuthorWithdrawal(_)
            | Self::Barrier { .. } => None,
            Self::DraftClose(frontier) => Some(frontier.current_head_revision_id.as_str()),
        }
    }
}

fn undo_database_error(error: tokio_postgres::Error) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn undo_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

pub(super) async fn editor_session_chapter(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
) -> Result<Option<String>, UndoLatestAuthorActionError> {
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

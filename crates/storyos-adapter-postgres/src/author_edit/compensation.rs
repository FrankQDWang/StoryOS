//! The prose Compensation of Author Undo: it restores the Revision before one applied Author
//! Edit through the `AuthoritativeRevision` profile (ADR 0044).

use storyos_application::{
    ProjectCommandError, UndoLatestAuthorActionCommand, UndoLatestAuthorActionError,
    UndoLatestAuthorActionSettlement, UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::command_sequence::{
    ActionDisposition, AuthoritativeRevision, RevisionBase, RevisionMembers, RevisionWrite,
    SettlementProfile, write_revision,
};
use crate::undo_compensation::{CompensationAdapter, CompensationReplay};
use crate::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_session,
};

/// The Forward evidence of one applied Author Edit or Acceptance whose Revision the Compensation
/// reverses.
pub(crate) struct ObservedProseFrontier {
    pub sequence: u64,
    pub chapter_id: String,
    pub resulting_revision_id: String,
    pub prior_revision_id: String,
    pub prior_payload: String,
    pub current_head_revision_id: String,
}

/// Restores the Revision before one applied Author Edit.
pub(crate) struct ProseCompensation;

impl CompensationAdapter for ProseCompensation {
    type Forward = ();
    type Evidence = ObservedProseFrontier;

    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        (): (),
        sequence: u64,
    ) -> Result<Option<ObservedProseFrontier>, UndoLatestAuthorActionError> {
        let row = load_revision_evidence(client, command, sequence).await?;
        Ok(row.and_then(|row| {
            match (
                row.get(/*idx*/ 0),
                row.get(/*idx*/ 1),
                row.get(/*idx*/ 2),
                row.get(/*idx*/ 3),
                row.get(/*idx*/ 4),
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
        }))
    }

    fn frontier_kind(evidence: &ObservedProseFrontier) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleDirectAuthorAction {
            resulting_revision_id: evidence.resulting_revision_id.clone(),
        }
    }

    async fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &ObservedProseFrontier,
        source_sequence: u64,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        compensate_revision(client, command, evidence, source_sequence).await
    }

    async fn decode(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
        decode_revision_compensation(client, command, replay).await
    }
}

/// Reads the Commit, the prior payload, and the current Head of the Forward Author Action at
/// `sequence`. Each column is null when its record is absent.
pub(crate) async fn load_revision_evidence(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    sequence: u64,
) -> Result<Option<tokio_postgres::Row>, UndoLatestAuthorActionError> {
    client
        .query_opt(
            "SELECT commit.manuscript_object_id::text,
                    commit.resulting_revision_id::text,
                    commit.prior_revision_id::text,
                    convert_from(prior_payload.canonical_bytes, 'UTF8'),
                    head.current_revision_id::text
               FROM storyos.author_action_entries AS action
          LEFT JOIN storyos.authoritative_commits AS commit
                 ON (commit.owner_user_id, commit.project_id, commit.receipt_id,
                     commit.receipt_result_kind, commit.authoritative_commit_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id,
                     action.receipt_result_kind, action.authoritative_commit_id)
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (action.owner_user_id, action.project_id, commit.manuscript_object_id)
          LEFT JOIN storyos.authoritative_revisions AS prior_revision
                 ON (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.manuscript_object_id, prior_revision.revision_id) =
                    (action.owner_user_id, action.project_id, commit.manuscript_object_id,
                     commit.prior_revision_id)
          LEFT JOIN storyos.authoritative_payloads AS prior_payload
                 ON (prior_payload.owner_user_id, prior_payload.project_id,
                     prior_payload.payload_id) =
                    (prior_revision.owner_user_id, prior_revision.project_id,
                     prior_revision.payload_id)
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric
                AND action.disposition = 'forward'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)
}

/// Restores the prior Revision of `frontier` as a new Revision with a Compensation Author
/// Action, and settles the Undo Receipt and fence.
pub(crate) async fn compensate_revision(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedProseFrontier,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let sequences = AuthoritativeRevision::allocate(client, &command.project_scope)
        .await
        .map_err(undo_sequence_error)?;
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        "{}",
        &frontier.current_head_revision_id,
        &sequences.ids.revision_id,
        UndoReceiptAuthority::Prose {
            revision_id: sequences.ids.revision_id.clone(),
            commit_id: sequences.ids.authoritative_commit_id.clone(),
        },
    )
    .await?;
    let applied = write_revision(
        client,
        &command.project_scope,
        &command.ids,
        sequences,
        RevisionWrite {
            effect: (),
            chapter_id: frontier.chapter_id.clone(),
            prior_revision_id: frontier.current_head_revision_id.clone(),
            payload: frontier.prior_payload.clone(),
            members: RevisionMembers::CopyFrom(frontier.prior_revision_id.clone()),
            disposition: ActionDisposition::Compensation { source_sequence },
            editor_session_id: command.editor_session_id.as_ref().to_owned(),
            writer_base: RevisionBase::Required,
        },
    )
    .await
    .map_err(undo_sequence_error)?;
    let response_project = settle_idempotency(client, command).await?;
    let author_undo_frontier_sequence =
        crate::editor_session::current_author_undo_frontier_sequence(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
        )
        .await
        .map_err(undo_from_session)?;
    Ok(UndoLatestAuthorActionSettlement {
        source_reopen_event: None,
        ids: command.ids.clone(),
        effect: UndoLatestAuthorActionSettlementEffect::Compensated {
            source_sequence,
            author_action_sequence: applied.author_action_sequence,
            authoritative_commit_id: applied.ids.authoritative_commit_id,
            revision_id: applied.ids.revision_id,
            body: applied.body,
            blocks: applied.blocks,
            author_undo_frontier_sequence,
            proposal_id: None,
            proposal_revision_id: None,
        },
        receipt_created_at,
        project_activity_position: applied.project_activity_position,
        response_project,
    })
}

/// Decodes a prose or Acceptance Compensation from the Revision that its Commit binds.
pub(crate) async fn decode_revision_compensation(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    replay: &CompensationReplay,
) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
    let Some(revision_id) = replay.resulting_revision_id.clone() else {
        return Ok(
            UndoLatestAuthorActionSettlementEffect::CompensatedProposal {
                source_sequence: replay.source_sequence,
                author_action_sequence: replay.author_action_sequence,
                proposal_revision_id: None,
                author_undo_frontier_sequence: replay.author_undo_frontier_sequence,
            },
        );
    };
    let stored = replay
        .resulting_payload
        .clone()
        .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
    let chapter_id = replay
        .chapter_id
        .clone()
        .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
    let blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &chapter_id,
        &revision_id,
        &stored,
    )
    .await
    .map_err(undo_database_error)?;
    Ok(UndoLatestAuthorActionSettlementEffect::Compensated {
        source_sequence: replay.source_sequence,
        author_action_sequence: replay.author_action_sequence,
        authoritative_commit_id: replay
            .authoritative_commit_id
            .clone()
            .ok_or(UndoLatestAuthorActionError::BindingConflict)?,
        body: crate::manuscript_block::display_body_from_stored(&stored, &blocks),
        revision_id,
        blocks,
        author_undo_frontier_sequence: replay.author_undo_frontier_sequence,
        proposal_id: None,
        proposal_revision_id: None,
    })
}

/// The Author Undo error of a profile write.
fn undo_sequence_error(error: ProjectCommandError) -> UndoLatestAuthorActionError {
    match error {
        ProjectCommandError::BindingConflict | ProjectCommandError::WriterIneligible => {
            UndoLatestAuthorActionError::BindingConflict
        }
        ProjectCommandError::HistoricalAcknowledgementUnavailable => {
            UndoLatestAuthorActionError::HistoricalAcknowledgementUnavailable
        }
        ProjectCommandError::InvalidChallenge => UndoLatestAuthorActionError::InvalidChallenge,
        ProjectCommandError::MissingProject => UndoLatestAuthorActionError::MissingProject,
        ProjectCommandError::Unavailable(source) => {
            UndoLatestAuthorActionError::Unavailable(source)
        }
    }
}

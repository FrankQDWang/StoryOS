//! The prose Compensation of Author Undo: it restores the Revision before one applied Author
//! Edit through the `AuthoritativeRevision` profile (ADR 0044).

use storyos_application::{ProjectCommandError, ProjectScope, UndoRecords};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::command_replay::ReplayFault;
use crate::command_sequence::{
    ActionDisposition, AuthoritativeRevision, RevisionBase, RevisionMembers, RevisionSequences,
    RevisionWrite, SettlementProfile, write_revision,
};
use crate::manuscript_block::{blocks_from_stored_payload, display_body_from_stored};
use crate::undo_compensation::{CompensationAdapter, CompensationReplay, UndoRequest};
use crate::undo_latest_author_action::undo_database_error;

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
    type Sequences = RevisionSequences;

    async fn load(
        client: &Client,
        command: &UndoRequest,
        (): (),
        sequence: u64,
    ) -> Result<Option<ObservedProseFrontier>, ProjectCommandError> {
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

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<RevisionSequences, ProjectCommandError> {
        AuthoritativeRevision::allocate(client, scope, &()).await
    }

    fn commit_ids(sequences: &RevisionSequences) -> Vec<String> {
        AuthoritativeRevision::commit_ids(sequences)
    }

    async fn compensate(
        client: &Client,
        command: &UndoRequest,
        evidence: &ObservedProseFrontier,
        sequences: RevisionSequences,
        source_sequence: u64,
    ) -> Result<UndoRecords, ProjectCommandError> {
        compensate_revision(client, command, evidence, sequences, source_sequence).await
    }

    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault> {
        decode_revision_compensation(replay)
    }
}

/// Reads the Commit, the prior payload, and the current Head of the Forward Author Action at
/// `sequence`. Each column is null when its record is absent.
pub(crate) async fn load_revision_evidence(
    client: &Client,
    command: &UndoRequest,
    sequence: u64,
) -> Result<Option<tokio_postgres::Row>, ProjectCommandError> {
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
/// Action through the `AuthoritativeRevision` profile.
pub(crate) async fn compensate_revision(
    client: &Client,
    command: &UndoRequest,
    frontier: &ObservedProseFrontier,
    sequences: RevisionSequences,
    source_sequence: u64,
) -> Result<UndoRecords, ProjectCommandError> {
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
    .await?;
    Ok(UndoRecords::Revision {
        authoritative_commit_id: applied.ids.authoritative_commit_id,
        revision_id: applied.ids.revision_id,
        body: applied.body,
        blocks: applied.blocks,
        project_activity_position: applied.project_activity_position,
        proposal_id: None,
        proposal_revision_id: None,
    })
}

/// Decodes a prose or Acceptance Compensation from the Revision that its Commit binds.
pub(crate) fn decode_revision_compensation(
    replay: &CompensationReplay,
) -> Result<UndoRecords, ReplayFault> {
    let damaged = || ReplayFault::Unavailable("a Revision Compensation is damaged".into());
    let revision = replay.revision.ok_or_else(damaged)?;
    let activity = revision.activity.as_ref().ok_or_else(damaged)?;
    if activity.authoritative_commit_id != revision.authoritative_commit_id
        || activity.resulting_revision_id != revision.revision_id
    {
        return Err(damaged());
    }
    let blocks = blocks_from_stored_payload(&revision.payload, &revision.member_block_ids);
    Ok(UndoRecords::Revision {
        authoritative_commit_id: revision.authoritative_commit_id.clone(),
        revision_id: revision.revision_id.clone(),
        body: display_body_from_stored(&revision.payload, &blocks),
        blocks,
        project_activity_position: activity
            .project_activity_position
            .parse()
            .map_err(|_| damaged())?,
        proposal_id: None,
        proposal_revision_id: None,
    })
}

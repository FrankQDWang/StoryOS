//! The Proposal edit Compensation of Author Undo (ADR 0044).

use storyos_application::{ProjectCommandError, ProjectScope, UndoRecords};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;
use uuid::Uuid;

use super::{ProposalEditContext, append_proposal_revision_as};
use crate::command_replay::ReplayFault;
use crate::undo_compensation::{
    CompensationAction, CompensationAdapter, CompensationReplay, UndoRequest,
    allocate_compensation_action,
};
use crate::undo_latest_author_action::{undo_database_error, undo_from_author_edit};

/// Appends a Proposal Revision with the candidate text of the parent of the current head.
pub(crate) struct ProposalEditCompensation;

impl CompensationAdapter for ProposalEditCompensation {
    type Forward = ();
    type Evidence = ObservedProposalFrontier;
    type Sequences = CompensationAction;

    async fn load(
        client: &Client,
        command: &UndoRequest,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedProposalFrontier>, ProjectCommandError> {
        load_proposal_frontier(client, &command.project_scope, sequence).await
    }

    fn frontier_kind(_evidence: &ObservedProposalFrontier) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CompensationAction, ProjectCommandError> {
        allocate_compensation_action(client, scope).await
    }

    fn receipt_payload(
        evidence: &ObservedProposalFrontier,
        project_activity_position: u64,
    ) -> serde_json::Map<String, serde_json::Value> {
        proposal_receipt_payload(
            &evidence.compensation_revision_id,
            &evidence.current_revision_id,
            project_activity_position,
        )
    }

    async fn compensate(
        client: &Client,
        command: &UndoRequest,
        frontier: &ObservedProposalFrontier,
        sequences: CompensationAction,
        source_sequence: u64,
    ) -> Result<UndoRecords, ProjectCommandError> {
        let context = ProposalEditContext {
            structured_candidate: false,
            proposal_id: frontier.proposal_id.clone(),
            operation_id: None,
            prior_revision_id: frontier.current_revision_id.clone(),
            manuscript_block_id: frontier.manuscript_block_id.clone(),
            base_authoritative_revision_id: frontier.base_authoritative_revision_id.clone(),
            kind: String::new(),
            ranges: Vec::new(),
            candidate_text: frontier.restored_candidate_text.clone(),
        };
        let proposal_revision_id = append_proposal_revision_as(
            client,
            &command.project_scope,
            &context,
            &command.expected_authoritative_revision_id,
            &frontier.restored_candidate_text,
            frontier.compensation_revision_id.clone(),
        )
        .await
        .map_err(|error| match error {
            storyos_application::AuthorEditError::BindingConflict => {
                ProjectCommandError::BindingConflict
            }
            other => undo_from_author_edit(other),
        })?;
        settle_proposal_compensation(
            client,
            command,
            sequences,
            source_sequence,
            proposal_revision_id,
        )
        .await
    }

    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault> {
        decode_proposal_compensation(replay, replay.restored_proposal_revision_id.clone())
    }
}

/// The Undo Receipt payload of a Compensation that appends `proposal_revision_id` to the source
/// head `source_proposal_revision_id`.
pub(crate) fn proposal_receipt_payload(
    proposal_revision_id: &str,
    source_proposal_revision_id: &str,
    project_activity_position: u64,
) -> serde_json::Map<String, serde_json::Value> {
    [
        ("proposal_revision_id", proposal_revision_id.to_owned()),
        (
            "source_proposal_revision_id",
            source_proposal_revision_id.to_owned(),
        ),
        (
            "project_activity_position",
            project_activity_position.to_string(),
        ),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.into()))
    .collect()
}

/// Records the Compensation Author Action of a Proposal Compensation that appended
/// `proposal_revision_id`.
pub(crate) async fn settle_proposal_compensation(
    client: &Client,
    command: &UndoRequest,
    sequences: CompensationAction,
    source_sequence: u64,
    proposal_revision_id: String,
) -> Result<UndoRecords, ProjectCommandError> {
    crate::structural_authority_settlement::persist_current_chapter_compensation_author_action(
        client,
        &command.project_scope,
        sequences.author_action_sequence,
        &command.ids.receipt_id,
        source_sequence,
    )
    .await
    .map_err(undo_database_error)?;
    Ok(UndoRecords::Proposal {
        proposal_revision_id: Some(proposal_revision_id),
        project_activity_position: sequences.project_activity_position,
    })
}

/// Decodes a Proposal Compensation whose appended Proposal Revision is `proposal_revision_id`.
/// The Activity position is the one that its Receipt payload records.
pub(crate) fn decode_proposal_compensation(
    replay: &CompensationReplay,
    proposal_revision_id: Option<String>,
) -> Result<UndoRecords, ReplayFault> {
    let damaged = || ReplayFault::Unavailable("a Proposal Compensation is damaged".into());
    if replay.receipt_payload.contains_key("proposal_revision_id") && proposal_revision_id.is_none()
    {
        return Err(damaged());
    }
    let project_activity_position = match replay.receipt_payload.get("project_activity_position") {
        Some(serde_json::Value::String(text)) => text.parse().map_err(|_| damaged())?,
        Some(_) => return Err(damaged()),
        None if replay.receipt_payload.is_empty() => 0,
        None => return Err(damaged()),
    };
    Ok(UndoRecords::Proposal {
        proposal_revision_id,
        project_activity_position,
    })
}

pub(crate) struct ObservedProposalFrontier {
    pub sequence: u64,
    /// The Proposal Revision that the Compensation appends.
    pub compensation_revision_id: String,
    pub chapter_id: String,
    pub proposal_id: String,
    pub current_revision_id: String,
    pub restored_candidate_text: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
}

async fn load_proposal_frontier(
    client: &Client,
    scope: &ProjectScope,
    sequence: u64,
) -> Result<Option<ObservedProposalFrontier>, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT proposal.chapter_id::text, proposal.proposal_id::text,
                    head.current_revision_id::text, parent.candidate_text,
                    proposal.manuscript_block_id::text,
                    revision.base_authoritative_revision_id::text
               FROM storyos.author_action_entries AS action
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.revision_id) =
                    (receipt.owner_user_id, receipt.project_id,
                     receipt.proposal_revision_ids[1])
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id) =
                    (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id)
               JOIN storyos.proposals AS proposal
                 ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                    (revision.owner_user_id, revision.project_id, revision.proposal_id)
               JOIN storyos.proposal_revisions AS parent
                 ON (parent.owner_user_id, parent.project_id, parent.proposal_id,
                     parent.revision_id) =
                    (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.parent_revision_id)
              LEFT JOIN storyos.author_action_entries AS compensation
                ON compensation.owner_user_id = action.owner_user_id
               AND compensation.project_id = action.project_id
               AND compensation.disposition = 'compensation'
               AND compensation.compensated_source_sequence = action.author_action_sequence
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric
                AND action.disposition = 'forward'
                AND receipt.result_kind = 'proposal_revised'
                AND compensation.author_action_sequence IS NULL",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    Ok(row.map(|row| ObservedProposalFrontier {
        sequence,
        compensation_revision_id: Uuid::now_v7().to_string(),
        chapter_id: row.get(/*idx*/ 0),
        proposal_id: row.get(/*idx*/ 1),
        current_revision_id: row.get(/*idx*/ 2),
        restored_candidate_text: row.get(/*idx*/ 3),
        manuscript_block_id: row.get(/*idx*/ 4),
        base_authoritative_revision_id: row.get(/*idx*/ 5),
    }))
}

//! The Proposal edit Compensation of Author Undo (ADR 0044).

use storyos_application::{
    ProjectScope, UndoLatestAuthorActionCommand, UndoLatestAuthorActionError,
    UndoLatestAuthorActionSettlement, UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use super::{ProposalEditContext, append_proposal_revision};
use crate::author_edit::parse_u64;
use crate::undo_compensation::{CompensationAdapter, CompensationReplay};
use crate::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_author_edit, undo_from_session,
};

/// Appends a Proposal Revision with the candidate text of the parent of the current head.
pub(crate) struct ProposalEditCompensation;

impl CompensationAdapter for ProposalEditCompensation {
    type Forward = ();
    type Evidence = ObservedProposalFrontier;

    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedProposalFrontier>, UndoLatestAuthorActionError> {
        load_proposal_frontier(client, &command.project_scope, sequence).await
    }

    fn frontier_kind(_evidence: &ObservedProposalFrontier) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &ObservedProposalFrontier,
        source_sequence: u64,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        persist_proposal_compensation(client, command, evidence, source_sequence).await
    }

    async fn decode(
        _client: &Client,
        _command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
        let payload: serde_json::Value = serde_json::from_str(&replay.result_payload)
            .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?;
        if payload.get("proposal_revision_id").is_some()
            && replay.restored_proposal_revision_id.is_none()
        {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        }
        Ok(
            UndoLatestAuthorActionSettlementEffect::CompensatedProposal {
                source_sequence: replay.source_sequence,
                author_action_sequence: replay.author_action_sequence,
                proposal_revision_id: replay.restored_proposal_revision_id.clone(),
                author_undo_frontier_sequence: replay.author_undo_frontier_sequence,
            },
        )
    }
}

async fn persist_proposal_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedProposalFrontier,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
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
    let proposal_revision_id = append_proposal_revision(
        client,
        &command.project_scope,
        &context,
        &command.expected_authoritative_revision_id,
        &frontier.restored_candidate_text,
    )
    .await
    .map_err(|error| match error {
        storyos_application::AuthorEditError::BindingConflict => {
            UndoLatestAuthorActionError::BindingConflict
        }
        other => undo_from_author_edit(other),
    })?;
    let counter_row = client
        .query_one(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = author_action_sequence + 1
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
          RETURNING author_action_sequence::text, project_activity_position::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let author_action_sequence =
        parse_u64(counter_row.get(/*idx*/ 0)).map_err(undo_from_author_edit)?;
    let project_activity_position =
        parse_u64(counter_row.get(/*idx*/ 1)).map_err(undo_from_author_edit)?;
    let payload = serde_json::json!({
        "proposal_revision_id": proposal_revision_id,
        "source_proposal_revision_id": frontier.current_revision_id,
        "project_activity_position": project_activity_position.to_string(),
    })
    .to_string();
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        &payload,
        &command.expected_authoritative_revision_id,
        &command.expected_authoritative_revision_id,
        UndoReceiptAuthority::None,
    )
    .await?;
    crate::structural_authority_settlement::persist_current_chapter_compensation_author_action(
        client,
        &command.project_scope,
        author_action_sequence,
        &command.ids.receipt_id,
        source_sequence,
    )
    .await
    .map_err(undo_database_error)?;
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
        effect: UndoLatestAuthorActionSettlementEffect::CompensatedProposal {
            source_sequence,
            author_action_sequence,
            proposal_revision_id: Some(proposal_revision_id),
            author_undo_frontier_sequence,
        },
        receipt_created_at,
        project_activity_position,
        response_project,
    })
}

pub(crate) struct ObservedProposalFrontier {
    pub sequence: u64,
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
) -> Result<Option<ObservedProposalFrontier>, UndoLatestAuthorActionError> {
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
        chapter_id: row.get(/*idx*/ 0),
        proposal_id: row.get(/*idx*/ 1),
        current_revision_id: row.get(/*idx*/ 2),
        restored_candidate_text: row.get(/*idx*/ 3),
        manuscript_block_id: row.get(/*idx*/ 4),
        base_authoritative_revision_id: row.get(/*idx*/ 5),
    }))
}

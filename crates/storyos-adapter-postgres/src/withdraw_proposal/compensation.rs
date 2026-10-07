//! The author withdrawal Compensation of Author Undo (ADR 0044).

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::author_edit::parse_u64;
use crate::undo_compensation::{CompensationAdapter, CompensationReplay};
use crate::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_author_edit, undo_from_session,
};

/// Reopens a Proposal that the author withdrew.
pub(crate) struct AuthorWithdrawalCompensation;

impl CompensationAdapter for AuthorWithdrawalCompensation {
    type Forward = ();
    type Evidence = ObservedAuthorWithdrawal;

    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedAuthorWithdrawal>, UndoLatestAuthorActionError> {
        load_author_withdrawal(client, command, sequence).await
    }

    fn frontier_kind(_evidence: &ObservedAuthorWithdrawal) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &ObservedAuthorWithdrawal,
        source_sequence: u64,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        persist_withdrawal_compensation(client, command, evidence, source_sequence).await
    }

    async fn decode(
        _client: &Client,
        _command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
        let payload: serde_json::Value = serde_json::from_str(&replay.result_payload)
            .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))?;
        let proposal_revision_id = payload["proposal_revision_id"]
            .as_str()
            .map(str::to_owned)
            .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
        Ok(
            UndoLatestAuthorActionSettlementEffect::CompensatedProposal {
                source_sequence: replay.source_sequence,
                author_action_sequence: replay.author_action_sequence,
                proposal_revision_id: Some(proposal_revision_id),
                author_undo_frontier_sequence: replay.author_undo_frontier_sequence,
            },
        )
    }
}

pub(crate) struct ObservedAuthorWithdrawal {
    pub sequence: u64,
    pub chapter_id: String,
    pub current_revision_id: String,
    pub generation: String,
    pub candidate_text: String,
    pub base_authoritative_revision_id: String,
}

async fn load_author_withdrawal(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    sequence: u64,
) -> Result<Option<ObservedAuthorWithdrawal>, UndoLatestAuthorActionError> {
    let row = client
        .query_opt(
            "SELECT proposal.chapter_id::text,
                    head.current_revision_id::text,
                    revision.generation,
                    revision.candidate_text,
                    revision.base_authoritative_revision_id::text
               FROM storyos.author_action_entries AS action
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id)
               JOIN storyos.proposal_withdrawals AS withdrawal
                 ON (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.withdrawal_receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                AND withdrawal.author_action_sequence = action.author_action_sequence
                AND withdrawal.withdrawal_reason = 'author_withdrew'
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id)
                AND head.current_revision_id = withdrawal.proposal_revision_id
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
                AND revision.closure = 'withdrawn'
               JOIN storyos.proposals AS proposal
                 ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                    (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id)
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric
                AND action.disposition = 'forward'
                AND receipt.command_kind = 'withdrawProposal'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id: Option<String> = row.get(0);
    let Some(chapter_id) = chapter_id else {
        return Ok(None);
    };
    Ok(Some(ObservedAuthorWithdrawal {
        sequence,
        chapter_id,
        current_revision_id: row.get(1),
        generation: row.get(2),
        candidate_text: row.get(3),
        base_authoritative_revision_id: row.get(4),
    }))
}

async fn persist_withdrawal_compensation(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &ObservedAuthorWithdrawal,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let proposal_revision_id = Uuid::now_v7().to_string();
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             SELECT withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id,
                    $4::text::uuid, $5, 'pending', 'open', $6, $7::text::uuid,
                    withdrawal.proposal_revision_id
               FROM storyos.proposal_withdrawals AS withdrawal
              WHERE withdrawal.owner_user_id = $1::text::uuid
                AND withdrawal.project_id = $2::text::uuid
                AND withdrawal.author_action_sequence = $3::text::numeric
                AND withdrawal.withdrawal_reason = 'author_withdrew'
                AND withdrawal.proposal_revision_id = $8::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &source_sequence.to_string(),
                &proposal_revision_id,
                &frontier.generation,
                &frontier.candidate_text,
                &frontier.base_authoritative_revision_id,
                &frontier.current_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if inserted != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    let head_updates = client
        .execute(
            "UPDATE storyos.proposal_heads AS head
                SET current_revision_id = $4::text::uuid
               FROM storyos.proposal_withdrawals AS withdrawal
              WHERE head.owner_user_id = withdrawal.owner_user_id
                AND head.project_id = withdrawal.project_id
                AND head.proposal_id = withdrawal.proposal_id
                AND head.current_revision_id = withdrawal.proposal_revision_id
                AND withdrawal.owner_user_id = $1::text::uuid
                AND withdrawal.project_id = $2::text::uuid
                AND withdrawal.author_action_sequence = $3::text::numeric
                AND withdrawal.proposal_revision_id = $5::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &source_sequence.to_string(),
                &proposal_revision_id,
                &frontier.current_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if head_updates != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
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
    let author_action_sequence = parse_u64(counter_row.get(0)).map_err(undo_from_author_edit)?;
    let project_activity_position = parse_u64(counter_row.get(1)).map_err(undo_from_author_edit)?;
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

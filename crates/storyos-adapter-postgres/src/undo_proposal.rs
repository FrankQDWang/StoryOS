use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};

use super::author_edit::parse_u64;
use super::author_edit_proposal::{ProposalEditContext, append_proposal_revision};
use super::undo_latest_author_action::{
    UndoReceiptAuthority, insert_undo_receipt, settle_idempotency, undo_database_error,
    undo_from_author_edit, undo_from_session,
};

pub(super) async fn persist_proposal_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    frontier: &super::author_edit_proposal::ObservedProposalFrontier,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let context = ProposalEditContext {
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
    let author_action_sequence = parse_u64(counter_row.get(0)).map_err(undo_from_author_edit)?;
    let project_activity_position = parse_u64(counter_row.get(1)).map_err(undo_from_author_edit)?;
    let receipt_created_at = insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        "{}",
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
        ids: command.ids.clone(),
        effect: UndoLatestAuthorActionSettlementEffect::CompensatedProposal {
            source_sequence,
            author_action_sequence,
            proposal_revision_id,
            author_undo_frontier_sequence,
        },
        receipt_created_at,
        project_activity_position,
        response_project,
    })
}

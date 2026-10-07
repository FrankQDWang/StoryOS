//! The exact Compensation of a Proposal decision that appended one Proposal Revision (ADR 0044).

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
};
use tokio_postgres::{Client, Row};
use uuid::Uuid;

use crate::author_edit_proposal::settle_proposal_compensation;
use crate::undo_latest_author_action::undo_database_error;

/// A Proposal decision whose resulting Proposal Revision is still the Proposal head.
pub(crate) struct ObservedProposalDecision {
    pub sequence: u64,
    pub proposal_id: String,
    pub chapter_id: String,
    /// The Proposal Revision before the decision.
    pub source_revision_id: String,
    /// The Proposal Revision that the decision appended. It is the current head.
    pub resulting_revision_id: String,
}

impl ObservedProposalDecision {
    /// Reads a row that starts with the Proposal, Chapter, source, and resulting identities.
    pub(crate) fn from_row(row: &Row, sequence: u64) -> Self {
        Self {
            sequence,
            proposal_id: row.get(/*idx*/ 0),
            chapter_id: row.get(/*idx*/ 1),
            source_revision_id: row.get(/*idx*/ 2),
            resulting_revision_id: row.get(/*idx*/ 3),
        }
    }
}

/// Loads a Proposal decision whose resulting revision is still the head. `None` is a Barrier.
///
/// `history` names the decision record table and its Receipt column, which keep one row for
/// each decision with the source and resulting Proposal Revisions.
pub(crate) async fn load_proposal_decision(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    sequence: u64,
    history: DecisionHistory,
) -> Result<Option<ObservedProposalDecision>, UndoLatestAuthorActionError> {
    let query = format!(
        "SELECT history.proposal_id::text, proposal.chapter_id::text,
                history.source_proposal_revision_id::text,
                history.resulting_proposal_revision_id::text
           FROM storyos.author_action_entries AS action
           JOIN storyos.{table} AS history
             ON (history.owner_user_id, history.project_id, history.{receipt_column},
                 history.author_action_sequence) =
                (action.owner_user_id, action.project_id, action.receipt_id,
                 action.author_action_sequence)
           JOIN storyos.proposals AS proposal
             ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                (history.owner_user_id, history.project_id, history.proposal_id)
           JOIN storyos.proposal_heads AS head
             ON (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id) =
                (history.owner_user_id, history.project_id, history.proposal_id,
                 history.resulting_proposal_revision_id)
          WHERE action.owner_user_id = $1::text::uuid AND action.project_id = $2::text::uuid
            AND action.author_action_sequence = $3::text::numeric
            AND action.disposition = 'forward' AND proposal.chapter_id IS NOT NULL
            FOR UPDATE OF head",
        table = history.table,
        receipt_column = history.receipt_column,
    );
    let row = client
        .query_opt(
            &query,
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    Ok(row.map(|row| ObservedProposalDecision::from_row(&row, sequence)))
}

/// The decision record table of a Proposal decision and the column of its Domain Receipt.
pub(crate) struct DecisionHistory {
    pub(crate) table: &'static str,
    pub(crate) receipt_column: &'static str,
}

/// Appends a Proposal Revision equal to the source revision of `decision`, with a copy of its
/// validation receipt, and records the Compensation.
pub(crate) async fn compensate_proposal_decision(
    client: &Client,
    command: &UndoLatestAuthorActionCommand,
    decision: &ObservedProposalDecision,
    source_sequence: u64,
) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
    let scope = &command.project_scope;
    let revision_id = Uuid::now_v7().to_string();
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, candidate_blocks, base_authoritative_revision_id,
                parent_revision_id)
             SELECT owner_user_id, project_id, proposal_id, $4::text::uuid, generation,
                    validation, closure, candidate_text, candidate_blocks,
                    base_authoritative_revision_id, $5::text::uuid
               FROM storyos.proposal_revisions
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND revision_id = $6::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &decision.proposal_id,
                &revision_id,
                &decision.resulting_revision_id,
                &decision.source_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if inserted != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    client
        .execute(
            "INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             SELECT owner_user_id, project_id, $4::text::uuid, proposal_id, $5::text::uuid,
                    result, base_authoritative_revision_id, manuscript_block_id, candidate_text,
                    reservation_state
               FROM storyos.validation_receipts
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND proposal_revision_id = $6::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &decision.proposal_id,
                &Uuid::now_v7().to_string(),
                &revision_id,
                &decision.source_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.proposal_heads
                SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &decision.proposal_id,
                &revision_id,
                &decision.resulting_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if head_updates != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    settle_proposal_compensation(
        client,
        command,
        source_sequence,
        &decision.resulting_revision_id,
        revision_id,
    )
    .await
}

//! The exact Compensation of a Proposal decision that appended one Proposal Revision (ADR 0044,
//! ADR 0068).

use storyos_application::{ProjectCommandError, UndoRecords};
use tokio_postgres::{Client, Row};
use uuid::Uuid;

use crate::author_edit_proposal::{proposal_receipt_payload, settle_proposal_compensation};
use crate::undo_compensation::{CompensationAction, UndoRequest};
use crate::undo_frontier::proposal_head_restores;
use crate::undo_latest_author_action::undo_database_error;

/// A Proposal decision whose resulting Proposal Revision the Proposal head restores.
pub(crate) struct ObservedProposalDecision {
    pub sequence: u64,
    /// The Proposal Revision that the Compensation appends.
    pub compensation_revision_id: String,
    pub proposal_id: String,
    pub chapter_id: String,
    /// The Proposal Revision before the decision.
    pub source_revision_id: String,
    /// The Proposal Revision that the decision appended.
    pub resulting_revision_id: String,
    /// The current Proposal head. It is the resulting Revision, or a later Revision that later
    /// Proposal Compensations wrote (ADR 0068). The Compensation appends its Revision to it.
    pub current_revision_id: String,
}

impl ObservedProposalDecision {
    /// Reads a row that starts with the Proposal, Chapter, source, resulting, and current head
    /// identities.
    pub(crate) fn from_row(row: &Row, sequence: u64) -> Self {
        Self {
            sequence,
            compensation_revision_id: Uuid::now_v7().to_string(),
            proposal_id: row.get(/*idx*/ 0),
            chapter_id: row.get(/*idx*/ 1),
            source_revision_id: row.get(/*idx*/ 2),
            resulting_revision_id: row.get(/*idx*/ 3),
            current_revision_id: row.get(/*idx*/ 4),
        }
    }

    /// Whether the current head restores the resulting Revision. Only later Proposal
    /// Compensations can explain a head that moved after the decision (ADR 0068).
    pub(crate) async fn head_restores_result(
        &self,
        client: &Client,
        command: &UndoRequest,
    ) -> Result<bool, ProjectCommandError> {
        if self.current_revision_id == self.resulting_revision_id {
            return Ok(true);
        }
        proposal_head_restores(
            client,
            command,
            &self.proposal_id,
            &self.current_revision_id,
            &self.resulting_revision_id,
        )
        .await
    }
}

/// Loads a Proposal decision whose resulting revision the head restores. `None` is a Barrier.
///
/// `history` names the decision record table and its Receipt column, which keep one row for
/// each decision with the source and resulting Proposal Revisions.
pub(crate) async fn load_proposal_decision(
    client: &Client,
    command: &UndoRequest,
    sequence: u64,
    history: DecisionHistory,
) -> Result<Option<ObservedProposalDecision>, ProjectCommandError> {
    let query = format!(
        "SELECT history.proposal_id::text, proposal.chapter_id::text,
                history.source_proposal_revision_id::text,
                history.resulting_proposal_revision_id::text,
                head.current_revision_id::text
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
             ON (head.owner_user_id, head.project_id, head.proposal_id) =
                (history.owner_user_id, history.project_id, history.proposal_id)
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
    let Some(row) = row else {
        return Ok(None);
    };
    let decision = ObservedProposalDecision::from_row(&row, sequence);
    if !decision.head_restores_result(client, command).await? {
        return Ok(None);
    }
    Ok(Some(decision))
}

/// The decision record table of a Proposal decision and the column of its Domain Receipt.
pub(crate) struct DecisionHistory {
    pub(crate) table: &'static str,
    pub(crate) receipt_column: &'static str,
}

/// The Undo Receipt payload of the Compensation of `decision`.
pub(crate) fn decision_receipt_payload(
    decision: &ObservedProposalDecision,
    project_activity_position: u64,
) -> serde_json::Map<String, serde_json::Value> {
    proposal_receipt_payload(
        &decision.compensation_revision_id,
        &decision.current_revision_id,
        project_activity_position,
    )
}

/// Appends to the current head a copy of the source revision of `decision` and of its validation
/// receipt. Then records the Compensation.
pub(crate) async fn compensate_proposal_decision(
    client: &Client,
    command: &UndoRequest,
    decision: &ObservedProposalDecision,
    sequences: CompensationAction,
    source_sequence: u64,
) -> Result<UndoRecords, ProjectCommandError> {
    let scope = &command.project_scope;
    let revision_id = decision.compensation_revision_id.clone();
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
                &decision.current_revision_id,
                &decision.source_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if inserted != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    // A Replan changes the base. The parent chain of the restored Revision then gives the set of
    // the other base. Thus the restored Revision gets a copy of the source set (ADR 0063). The
    // current head restores the resulting Revision, so it has the base of that Revision.
    client
        .execute(
            "INSERT INTO storyos.proposal_anchors
               (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                manuscript_block_id, base_authoritative_revision_id, manuscript_schema_version,
                coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest,
                proposal_revision_id)
             SELECT anchor.owner_user_id, anchor.project_id, anchor.proposal_id,
                    anchor.operation_id, anchor.anchor_order, anchor.manuscript_block_id,
                    anchor.base_authoritative_revision_id, anchor.manuscript_schema_version,
                    anchor.coordinate_profile, anchor.range_from, anchor.range_to,
                    anchor.boundary_profile, anchor.base_slice_digest, $4::text::uuid
               FROM storyos.proposal_anchors AS anchor
              WHERE anchor.owner_user_id = $1::text::uuid AND anchor.project_id = $2::text::uuid
                AND anchor.proposal_id = $3::text::uuid
                AND anchor.proposal_revision_id = storyos.proposal_anchor_set_revision(
                      $1::text::uuid, $2::text::uuid, $3::text::uuid, $6::text::uuid)
                AND EXISTS (
                  SELECT 1
                    FROM storyos.proposal_revisions AS source
                    JOIN storyos.proposal_revisions AS head
                      USING (owner_user_id, project_id, proposal_id)
                   WHERE source.owner_user_id = $1::text::uuid
                     AND source.project_id = $2::text::uuid
                     AND source.proposal_id = $3::text::uuid
                     AND source.revision_id = $6::text::uuid
                     AND head.revision_id = $5::text::uuid
                     AND source.base_authoritative_revision_id
                         <> head.base_authoritative_revision_id
                )",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &decision.proposal_id,
                &revision_id,
                &decision.current_revision_id,
                &decision.source_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
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
                &decision.current_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if head_updates != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    settle_proposal_compensation(client, command, sequences, source_sequence, revision_id).await
}

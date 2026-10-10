//! The Proposal Revision that one candidate edit appends.

use storyos_application::{AuthorEditError, ProjectScope};
use storyos_core::{OpenBlockProposal, open_block_proposal};
use tokio_postgres::Client;
use uuid::Uuid;

use super::ProposalEditContext;
use crate::author_edit::author_edit_database_error;

/// Appends `revision_id` with `candidate_text` for the Operation of `context` and gives it a new
/// Validation Receipt. The Revision records that Operation and its candidate before the edit.
pub(crate) async fn append_proposal_revision_as(
    client: &Client,
    scope: &ProjectScope,
    context: &ProposalEditContext,
    current_authoritative_revision_id: &str,
    candidate_text: &str,
    revision_id: String,
) -> Result<String, AuthorEditError> {
    let owner = scope.owner_user_id.as_ref();
    let project = scope.project_id.as_ref();
    let facts = client
        .query_one(
            "SELECT EXISTS (
                    SELECT 1
                      FROM storyos.authoritative_heads AS head
                      JOIN storyos.manuscript_revision_members AS member
                        ON (member.owner_user_id, member.project_id,
                            member.manuscript_object_id, member.revision_id) =
                           (head.owner_user_id, head.project_id,
                            head.manuscript_object_id, head.current_revision_id)
                     WHERE head.owner_user_id = $1::text::uuid
                       AND head.project_id = $2::text::uuid
                       AND member.manuscript_block_id = $3::text::uuid
                  ),
                  EXISTS (
                    SELECT 1
                      FROM storyos.proposal_operations AS reservation
                     WHERE reservation.owner_user_id = $1::text::uuid
                       AND reservation.project_id = $2::text::uuid
                       AND reservation.manuscript_block_id = $3::text::uuid
                       AND reservation.reservation_state = 'unresolved'
                       AND reservation.proposal_id <> $4::text::uuid
                  )",
            &[
                &owner,
                &project,
                &context.manuscript_block_id,
                &context.proposal_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let classification = open_block_proposal(&OpenBlockProposal {
        scope_matches: true,
        target_block_present: facts.get(/*idx*/ 0),
        expected_base_revision_id: context.base_authoritative_revision_id.clone(),
        current_base_revision_id: Some(current_authoritative_revision_id.to_owned()),
        conflicting_reservation: facts.get(/*idx*/ 1),
    });
    let (validation, receipt_result) = match classification {
        storyos_core::OpenBlockProposalResult::Applied => ("valid", "valid"),
        storyos_core::OpenBlockProposalResult::Conflicted { .. } => ("conflicted", "conflicted"),
        storyos_core::OpenBlockProposalResult::Refused { .. } => {
            return Err(AuthorEditError::BindingConflict);
        }
    };
    let edited = client
        .query_opt(
            "SELECT operation_id::text, candidate_text
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
                AND manuscript_block_id = $4::text::uuid
                AND ($5::text IS NULL OR operation_id = $5::text::uuid)
                AND resolution = 'pending'
              FOR UPDATE",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &context.manuscript_block_id,
                &context.operation_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?
        .map(|row| {
            (
                row.get::<_, String>(/*idx*/ 0),
                row.get::<_, String>(/*idx*/ 1),
            )
        });
    if context.operation_id.is_some() && edited.is_none() {
        return Err(AuthorEditError::BindingConflict);
    }
    let (edited_operation_id, prior_operation_candidate_text) = edited.unzip();
    let validation_receipt_id = Uuid::now_v7().to_string();
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id,
                candidate_blocks, edited_operation_id, prior_operation_candidate_text)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                     $5, 'open', (SELECT CASE WHEN proposal.manuscript_block_id=$9::text::uuid
                       THEN $6 ELSE prior.candidate_text END
                       FROM storyos.proposals AS proposal
                       JOIN storyos.proposal_revisions AS prior
                         USING (owner_user_id, project_id, proposal_id)
                      WHERE proposal.owner_user_id=$1::text::uuid AND proposal.project_id=$2::text::uuid
                        AND proposal.proposal_id=$3::text::uuid AND prior.revision_id=$8::text::uuid),
                     $7::text::uuid, $8::text::uuid,
                     (SELECT candidate_blocks FROM storyos.proposal_revisions
                       WHERE owner_user_id=$1::text::uuid AND project_id=$2::text::uuid
                         AND proposal_id=$3::text::uuid AND revision_id=$8::text::uuid),
                     $10::text::uuid, $11)",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &revision_id,
                &validation,
                &candidate_text,
                &context.base_authoritative_revision_id,
                &context.prior_revision_id,
                &context.manuscript_block_id,
                &edited_operation_id,
                &prior_operation_candidate_text,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    let head_updates = client
        .execute(
            "UPDATE storyos.proposal_heads
                SET current_revision_id = $4::text::uuid
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid AND current_revision_id = $5::text::uuid",
            &[
                &owner,
                &project,
                &context.proposal_id,
                &revision_id,
                &context.prior_revision_id,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if head_updates != 1 {
        return Err(AuthorEditError::BindingConflict);
    }
    client
        .execute(
            "INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7::text::uuid, $8::text::uuid, $9, 'unresolved')",
            &[
                &owner,
                &project,
                &validation_receipt_id,
                &context.proposal_id,
                &revision_id,
                &receipt_result,
                &context.base_authoritative_revision_id,
                &context.manuscript_block_id,
                &candidate_text,
            ],
        )
        .await
        .map_err(author_edit_database_error)?;
    if let Some(operation_id) = &edited_operation_id {
        client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET candidate_text = $4
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND operation_id = $5::text::uuid",
                &[
                    &owner,
                    &project,
                    &context.proposal_id,
                    &candidate_text,
                    operation_id,
                ],
            )
            .await
            .map_err(author_edit_database_error)?;
    }
    Ok(revision_id)
}

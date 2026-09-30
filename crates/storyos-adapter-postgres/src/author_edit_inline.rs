use storyos_application::{ApplyAuthorEditCommand, AuthorEditError};
use storyos_core::{AuthorEditPrimitive, VersionedTargetOwnership};
use tokio_postgres::Client;

use super::author_edit::author_edit_database_error;
use super::author_edit_proposal::ProposalEditContext;

pub(super) async fn load_inline_target_ownership(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    context: Option<&ProposalEditContext>,
    current_revision_id: &str,
) -> Result<Option<VersionedTargetOwnership>, AuthorEditError> {
    let Some(context) = context else {
        return Ok(None);
    };
    let [unit] = command.author_edit_units.as_slice() else {
        return Ok(None);
    };
    let [
        AuthorEditPrimitive::ReplaceBlockSelection {
            manuscript_block_id,
            ..
        },
    ] = unit.normalized_primitives.as_slice()
    else {
        return Ok(None);
    };
    if context.kind != "inline_edit"
        || context.base_authoritative_revision_id != current_revision_id
        || context.manuscript_block_id != *manuscript_block_id
        || command.proposal_target.is_some()
        || command.retry_source.is_some()
        || command.expected_proposal_head_revision_ids != [context.prior_revision_id.clone()]
    {
        return Ok(None);
    }
    let rows = client.query(
        "SELECT anchor.range_from, anchor.range_to
           FROM storyos.proposals AS proposal
           JOIN storyos.proposal_heads AS head USING (owner_user_id, project_id, proposal_id)
           JOIN storyos.proposal_revisions AS revision
             ON (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id) =
                (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
           JOIN storyos.proposal_operations AS operation USING (owner_user_id, project_id, proposal_id)
           JOIN storyos.proposal_anchors AS anchor USING (owner_user_id, project_id, proposal_id, operation_id)
           LEFT JOIN storyos.proposal_validation_conditions AS failure
             ON (failure.owner_user_id, failure.project_id, failure.proposal_id, failure.proposal_revision_id) =
                (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)
          WHERE proposal.owner_user_id=$1::text::uuid AND proposal.project_id=$2::text::uuid
            AND proposal.chapter_id=$3::text::uuid AND proposal.proposal_id=$4::text::uuid
            AND head.current_revision_id=$5::text::uuid AND proposal.kind='inline_edit'
            AND revision.base_authoritative_revision_id=$6::text::uuid AND revision.closure='open'
            AND revision.generation='ready' AND COALESCE(failure.validation, revision.validation)='valid'
            AND operation.manuscript_block_id=$7::text::uuid AND operation.resolution='pending'
            AND operation.reservation_state='unresolved' AND anchor.manuscript_block_id=$7::text::uuid
            AND anchor.base_authoritative_revision_id=$6::text::uuid AND anchor.manuscript_schema_version=1
            AND anchor.coordinate_profile='prosemirror-token-utf16.v1'
            AND anchor.boundary_profile='exclusive-authoritative-edges.v1'
          ORDER BY anchor.anchor_order LIMIT 2",
        &[&command.project_scope.owner_user_id.as_ref(), &command.project_scope.project_id.as_ref(),
          &command.chapter_id, &context.proposal_id, &context.prior_revision_id, &current_revision_id,
          &manuscript_block_id],
    ).await.map_err(author_edit_database_error)?;
    let [row] = rows.as_slice() else {
        return Ok(None);
    };
    let from = u32::try_from(row.get::<_, i32>(0))
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    let to = u32::try_from(row.get::<_, i32>(1))
        .map_err(|error| AuthorEditError::Unavailable(Box::new(error)))?;
    Ok(Some(VersionedTargetOwnership::InlineReservation {
        manuscript_block_id: manuscript_block_id.clone(),
        proposal_head_revision_id: context.prior_revision_id.clone(),
        reserved_ranges: vec![(from, to)],
    }))
}

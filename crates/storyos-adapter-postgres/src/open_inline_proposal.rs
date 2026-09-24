use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    EXCLUSIVE_AUTHORITATIVE_EDGES_V1, INLINE_PROSE_CHANGE_SOURCE, InlineTargetBlock,
    OpenInlineProposal, OpenInlineProposalAnchor, PROSEMIRROR_TOKEN_UTF16_V1, open_inline_proposal,
    proposal_anchor_base_slice_digest,
};
use uuid::Uuid;

use crate::admitted_proposal_target::{load_admitted_targets, load_current_target};

pub(crate) async fn open_selected_inline_change(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let Some(target) = load_admitted_targets(client, claim, chapter_id)
        .await?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let block_id = target.block_id.as_str();
    let revision_id = target.revision_id.as_str();
    let current = load_current_target(client, claim, chapter_id, block_id).await?;
    let Some((from, to)) = utf16_range_of(&target.block_text, INLINE_PROSE_CHANGE_SOURCE) else {
        return Ok(None);
    };
    let digest = proposal_anchor_base_slice_digest(
        block_id,
        "paragraph",
        1,
        PROSEMIRROR_TOKEN_UTF16_V1,
        from,
        to,
        INLINE_PROSE_CHANGE_SOURCE,
    );
    let classification = open_inline_proposal(&OpenInlineProposal {
        scope_matches: true,
        target_block_present: current.revision_id.is_some(),
        expected_base_revision_id: revision_id.to_owned(),
        current_base_revision_id: current.revision_id,
        conflicting_reservation: current.reserved,
        current_schema_version: 1,
        current_coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
        blocks: vec![InlineTargetBlock {
            manuscript_block_id: block_id.to_owned(),
            block_kind: "paragraph".to_owned(),
            text: target.block_text.clone(),
        }],
        anchors: vec![OpenInlineProposalAnchor {
            manuscript_block_id: block_id.to_owned(),
            base_authoritative_revision_id: revision_id.to_owned(),
            manuscript_schema_version: 1,
            coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
            from,
            to,
            boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
            base_slice_digest: digest.clone(),
        }],
    });
    let Some(validation_result) = classification.validation_receipt_result() else {
        return Ok(None);
    };
    persist_applied_inline(
        client,
        claim,
        chapter_id,
        decision_id,
        candidate_text,
        block_id,
        revision_id,
        from,
        to,
        &digest,
        validation_result,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn persist_applied_inline(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
    block_id: &str,
    revision_id: &str,
    from: u32,
    to: u32,
    digest: &str,
    validation_result: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let proposal_id = Uuid::now_v7().to_string();
    let proposal_revision_id = Uuid::now_v7().to_string();
    let operation_id = Uuid::now_v7().to_string();
    let validation_receipt_id = Uuid::now_v7().to_string();
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    client
        .execute(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'inline_edit',
                     $4::text::uuid, $5::text::uuid, $6::text::uuid, $7::text::uuid)",
            &[
                &owner,
                &project,
                &proposal_id,
                &chapter_id,
                &block_id,
                &claim.run_id,
                &decision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                     'valid', 'open', $5, $6::text::uuid)",
            &[
                &owner,
                &project,
                &proposal_id,
                &proposal_revision_id,
                &candidate_text,
                &revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[&owner, &project, &proposal_id, &proposal_revision_id],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'pending', 'unresolved', $6)",
            &[
                &owner,
                &project,
                &proposal_id,
                &operation_id,
                &block_id,
                &candidate_text,
            ],
        )
        .await
        .map_err(database_error)?;
    let from_i32 =
        i32::try_from(from).map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    let to_i32 =
        i32::try_from(to).map_err(|error| CompleteAgentRunError::Unavailable(Box::new(error)))?;
    client
        .execute(
            "INSERT INTO storyos.proposal_anchors
               (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                manuscript_block_id, base_authoritative_revision_id, manuscript_schema_version,
                coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 1,
                     $5::text::uuid, $6::text::uuid, 1, $7, $8, $9, $10, $11)",
            &[
                &owner,
                &project,
                &proposal_id,
                &operation_id,
                &block_id,
                &revision_id,
                &PROSEMIRROR_TOKEN_UTF16_V1,
                &from_i32,
                &to_i32,
                &EXCLUSIVE_AUTHORITATIVE_EDGES_V1,
                &digest,
            ],
        )
        .await
        .map_err(database_error)?;
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
                &proposal_id,
                &proposal_revision_id,
                &validation_result,
                &revision_id,
                &block_id,
                &candidate_text,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(Some(proposal_id))
}

fn utf16_range_of(text: &str, needle: &str) -> Option<(u32, u32)> {
    let byte = text.find(needle)?;
    let from = u32::try_from(text[..byte].encode_utf16().count()).ok()?;
    let width = u32::try_from(needle.encode_utf16().count()).ok()?;
    Some((from, from.checked_add(width)?))
}

fn database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    EXCLUSIVE_AUTHORITATIVE_EDGES_V1, INLINE_PROSE_CHANGE_SOURCE, InlineTargetBlock,
    OpenInlineProposal, OpenInlineProposalAnchor, PROSEMIRROR_TOKEN_UTF16_V1, open_inline_proposal,
    proposal_anchor_base_slice_digest,
};
use uuid::Uuid;

pub(crate) async fn open_selected_inline_change(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let target = select_inline_target(client, claim, chapter_id).await?;
    let Some(block_id) = target.block_id.as_deref() else {
        return Ok(None);
    };
    let Some(revision_id) = target.revision_id.as_deref() else {
        return Ok(None);
    };
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
        target_block_present: true,
        expected_base_revision_id: revision_id.to_owned(),
        current_base_revision_id: Some(revision_id.to_owned()),
        conflicting_reservation: target.conflicting_reservation,
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

struct InlineTarget {
    block_id: Option<String>,
    revision_id: Option<String>,
    block_text: String,
    conflicting_reservation: bool,
}

async fn select_inline_target(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
) -> Result<InlineTarget, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT member.manuscript_block_id::text, member.revision_id::text,
                    convert_from(payload.canonical_bytes, 'UTF8'),
                    reservation.proposal_id IS NOT NULL
               FROM storyos.authoritative_heads AS head
               JOIN storyos.authoritative_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id,
                     revision.manuscript_object_id, revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
               JOIN storyos.manuscript_revision_members AS member
                 ON (member.owner_user_id, member.project_id, member.manuscript_object_id,
                     member.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
               LEFT JOIN storyos.proposal_operations AS reservation
                 ON (reservation.owner_user_id, reservation.project_id,
                     reservation.manuscript_block_id) =
                    (member.owner_user_id, member.project_id, member.manuscript_block_id)
                AND reservation.reservation_state = 'unresolved'
              WHERE head.owner_user_id = $1::text::uuid
                AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid
              ORDER BY member.block_order",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(database_error)?;
    let mut first_live = None;
    for row in rows {
        let block_id: String = row.get(0);
        let revision_id: String = row.get(1);
        let stored: String = row.get(2);
        let reserved: bool = row.get(3);
        let block_text = crate::manuscript_block::blocks_from_stored_payload(
            &stored,
            std::slice::from_ref(&block_id),
        )
        .into_iter()
        .next()
        .map(|block| block.text)
        .unwrap_or_default();
        if first_live.is_none() {
            first_live = Some((block_id.clone(), revision_id.clone(), block_text.clone()));
        }
        if !reserved {
            return Ok(InlineTarget {
                block_id: Some(block_id),
                revision_id: Some(revision_id),
                block_text,
                conflicting_reservation: false,
            });
        }
    }
    Ok(match first_live {
        Some((block_id, revision_id, block_text)) => InlineTarget {
            block_id: Some(block_id),
            revision_id: Some(revision_id),
            block_text,
            conflicting_reservation: true,
        },
        None => InlineTarget {
            block_id: None,
            revision_id: None,
            block_text: String::new(),
            conflicting_reservation: false,
        },
    })
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
                resolution, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'pending', 'unresolved')",
            &[&owner, &project, &proposal_id, &operation_id, &block_id],
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

//! The Proposal that an Undo Acceptance reopens, derives, or places for a Reversal (ADR 0044).

use storyos_application::ProjectCommandError;

use crate::undo_compensation::UndoRequest;
use storyos_core::ManuscriptBlock;
use uuid::Uuid;

use super::{AcceptanceFacts, LoadedAcceptance, database_error};
use crate::author_edit::sha256_hex;

pub(super) enum LinkMode<'a> {
    Reopen,
    Derived,
    Reversal { blocks: &'a [ManuscriptBlock] },
}

pub(super) async fn evidence_usable(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
    loaded: &LoadedAcceptance,
    facts: &AcceptanceFacts,
    current_envelope: Option<String>,
    prior_envelope: Option<String>,
    current_payload: Option<String>,
) -> Result<bool, ProjectCommandError> {
    // A stored empty Chapter payload is usable prior evidence.
    if !loaded.prior_payload_present || facts.block_ids.is_empty() {
        return Ok(false);
    }
    let Some(current_payload) = current_payload else {
        return Ok(false);
    };
    if !digest_agrees(&loaded.prior_payload, prior_envelope.as_deref()) {
        return Ok(false);
    }
    if loaded.current_head_revision_id == loaded.resulting_revision_id {
        let digest = sha256_hex(current_payload.as_bytes());
        return Ok(current_envelope.as_deref() == Some(digest.as_str()));
    }
    if !digest_agrees(&current_payload, current_envelope.as_deref()) {
        return Ok(false);
    }
    let prior_blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &loaded.prior_revision_id,
        &loaded.prior_payload,
    )
    .await
    .map_err(database_error)?;
    let current_blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &loaded.current_head_revision_id,
        &current_payload,
    )
    .await
    .map_err(database_error)?;
    Ok(facts.block_ids.iter().all(|block_id| {
        prior_blocks
            .iter()
            .any(|block| &block.manuscript_block_id == block_id)
            && current_blocks
                .iter()
                .any(|block| &block.manuscript_block_id == block_id)
    }))
}

pub(super) async fn reservation_blocked(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
    block_ids: &[String],
) -> Result<bool, ProjectCommandError> {
    if block_ids.is_empty() {
        return Ok(true);
    }
    let blocked = client
        .query_one(
            "SELECT EXISTS (
               SELECT 1 FROM storyos.proposal_operations
                WHERE owner_user_id = $1::text::uuid
                  AND project_id = $2::text::uuid
                  AND manuscript_block_id = ANY($3::text[]::uuid[])
                  AND reservation_state = 'unresolved')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &block_ids,
            ],
        )
        .await
        .map_err(database_error)?
        .get(/*idx*/ 0);
    Ok(blocked)
}

pub(super) async fn place_proposal(
    client: &tokio_postgres::Client,
    command: &UndoRequest,
    facts: &AcceptanceFacts,
    base_revision_id: &str,
    mode: LinkMode<'_>,
) -> Result<(String, String), ProjectCommandError> {
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    let revision_id = Uuid::now_v7().to_string();
    let (proposal_id, parent, kind_override, candidate_override) = match &mode {
        LinkMode::Reopen => (
            facts.proposal_id.clone(),
            Some(facts.proposal_revision_id.as_str()),
            None,
            None,
        ),
        LinkMode::Derived => (Uuid::now_v7().to_string(), None, None, None),
        LinkMode::Reversal { blocks } => {
            let text = blocks
                .iter()
                .find(|block| block.manuscript_block_id == facts.manuscript_block_id)
                .map(|block| block.text.clone())
                .ok_or(ProjectCommandError::BindingConflict)?;
            (
                Uuid::now_v7().to_string(),
                None,
                Some("reversal"),
                Some(text),
            )
        }
    };
    if !matches!(mode, LinkMode::Reopen) {
        let inserted = client
            .execute(
                "INSERT INTO storyos.proposals
                   (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                    source_run_id, source_decision_id, bundle_policy, source_draft_id,
                    source_draft_revision_id, source_draft_payload_digest)
                 SELECT owner_user_id, project_id, $4::text::uuid, COALESCE($5, kind),
                        chapter_id, manuscript_block_id, source_run_id, source_decision_id,
                        bundle_policy, source_draft_id, source_draft_revision_id,
                        source_draft_payload_digest
                   FROM storyos.proposals
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid",
                &[
                    &owner,
                    &project,
                    &facts.proposal_id,
                    &proposal_id,
                    &kind_override,
                ],
            )
            .await
            .map_err(database_error)?;
        if inserted != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
    }
    let candidate = candidate_override
        .clone()
        .unwrap_or_else(|| facts.candidate_text.clone());
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id,
                candidate_blocks)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                    'valid', 'open', COALESCE($5, source.candidate_text), $6::text::uuid,
                    $7::text::uuid, CASE WHEN $5 IS NULL THEN source.candidate_blocks ELSE NULL END
               FROM storyos.proposal_revisions AS source
              WHERE source.owner_user_id = $1::text::uuid
                AND source.project_id = $2::text::uuid
                AND source.proposal_id = $8::text::uuid
                AND source.revision_id = $9::text::uuid",
            &[
                &owner,
                &project,
                &proposal_id,
                &revision_id,
                &candidate_override,
                &base_revision_id,
                &parent,
                &facts.proposal_id,
                &facts.proposal_revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    if inserted != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    if matches!(mode, LinkMode::Reopen) {
        let updated = client
            .execute(
                "UPDATE storyos.proposal_heads
                    SET current_revision_id = $4::text::uuid
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND current_revision_id = $5::text::uuid",
                &[
                    &owner,
                    &project,
                    &proposal_id,
                    &revision_id,
                    &facts.proposal_revision_id,
                ],
            )
            .await
            .map_err(database_error)?;
        if updated != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let updated = client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET resolution = 'pending', reservation_state = 'unresolved'
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($4::text[]::uuid[])
                    AND resolution = 'applied'",
                &[
                    &owner,
                    &project,
                    &proposal_id,
                    &facts.selected_operation_ids,
                ],
            )
            .await
            .map_err(database_error)?;
        if updated != facts.selected_operation_ids.len() as u64 {
            return Err(ProjectCommandError::BindingConflict);
        }
    } else {
        client
            .execute(
                "INSERT INTO storyos.proposal_heads
                   (owner_user_id, project_id, proposal_id, current_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
                &[&owner, &project, &proposal_id, &revision_id],
            )
            .await
            .map_err(database_error)?;
        let inserted = client
            .execute(
                "INSERT INTO storyos.proposal_operations
                   (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                    resolution, reservation_state, candidate_text, predecessor_operation_ids,
                    candidate_blocks)
                 SELECT owner_user_id, project_id, $4::text::uuid, operation_id, manuscript_block_id,
                        'pending', 'unresolved', candidate_text, predecessor_operation_ids,
                        candidate_blocks
                   FROM storyos.proposal_operations
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($5::text[]::uuid[])",
                &[
                    &owner,
                    &project,
                    &facts.proposal_id,
                    &proposal_id,
                    &facts.selected_operation_ids,
                ],
            )
            .await
            .map_err(database_error)?;
        if inserted != facts.selected_operation_ids.len() as u64 {
            return Err(ProjectCommandError::BindingConflict);
        }
        if let LinkMode::Reversal { blocks } = &mode {
            for block in blocks.iter().filter(|block| {
                facts
                    .block_ids
                    .iter()
                    .any(|block_id| block_id == &block.manuscript_block_id)
            }) {
                client
                    .execute(
                        "UPDATE storyos.proposal_operations
                            SET candidate_text = $5, candidate_blocks = NULL
                          WHERE owner_user_id = $1::text::uuid
                            AND project_id = $2::text::uuid
                            AND proposal_id = $3::text::uuid
                            AND manuscript_block_id = $4::text::uuid",
                        &[
                            &owner,
                            &project,
                            &proposal_id,
                            &block.manuscript_block_id,
                            &block.text,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
        } else {
            client
                .execute(
                    "INSERT INTO storyos.proposal_anchors
                       (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                        manuscript_block_id, base_authoritative_revision_id, manuscript_schema_version,
                        coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest)
                     SELECT owner_user_id, project_id, $4::text::uuid, operation_id, anchor_order,
                            manuscript_block_id, $5::text::uuid, manuscript_schema_version,
                            coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest
                       FROM storyos.proposal_anchors
                      WHERE owner_user_id = $1::text::uuid
                        AND project_id = $2::text::uuid
                        AND proposal_id = $3::text::uuid
                        AND operation_id = ANY($6::text[]::uuid[])",
                    &[
                        &owner,
                        &project,
                        &facts.proposal_id,
                        &proposal_id,
                        &base_revision_id,
                        &facts.selected_operation_ids,
                    ],
                )
                .await
                .map_err(database_error)?;
        }
    }
    client
        .execute(
            "INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'valid', $6::text::uuid, $7::text::uuid, $8, 'unresolved')",
            &[
                &owner,
                &project,
                &Uuid::now_v7().to_string(),
                &proposal_id,
                &revision_id,
                &base_revision_id,
                &facts.manuscript_block_id,
                &candidate,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok((proposal_id, revision_id))
}

fn digest_agrees(payload: &str, envelope: Option<&str>) -> bool {
    match envelope {
        Some(digest) => sha256_hex(payload.as_bytes()) == digest,
        None => true,
    }
}

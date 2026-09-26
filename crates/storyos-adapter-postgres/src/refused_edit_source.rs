use storyos_application::{ApplyAuthorEditCommand, AuthorEditError};
use storyos_core::{
    CurrentOrderedSourceFacts, EditSourceOwner, OpenInlineProposalAnchor, ProposalEditSourceFacts,
};
use tokio_postgres::Client;

use crate::author_edit::author_edit_database_error;

pub(super) async fn load_sources(
    client: &Client,
    command: &ApplyAuthorEditCommand,
    revision_id: &str,
    body: &str,
) -> Result<CurrentOrderedSourceFacts, AuthorEditError> {
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    let blocks = crate::manuscript_block::load_revision_blocks(
        client,
        owner,
        project,
        &command.chapter_id,
        revision_id,
        body,
    )
    .await
    .map_err(author_edit_database_error)?;
    let selection = command
        .author_edit_units
        .iter()
        .find_map(|unit| unit.selection_snapshot.ordered_selection.as_ref());
    let Some(selection) = selection else {
        return Ok(CurrentOrderedSourceFacts {
            blocks,
            proposals: Vec::new(),
            complete: false,
        });
    };
    let selected_block_id = |source: &storyos_core::SelectedEditSource| match &source.owner {
        EditSourceOwner::Manuscript {
            manuscript_block_id,
        }
        | EditSourceOwner::Proposal {
            manuscript_block_id,
            ..
        } => manuscript_block_id.clone(),
    };
    let first = selection.sources.first().map(selected_block_id);
    let last = selection.sources.last().map(selected_block_id);
    let first_index = blocks
        .iter()
        .position(|block| Some(&block.manuscript_block_id) == first.as_ref());
    let last_index = blocks
        .iter()
        .position(|block| Some(&block.manuscript_block_id) == last.as_ref());
    let covered_ids = first_index
        .zip(last_index)
        .and_then(|(first, last)| blocks.get(first..=last))
        .map(|covered| {
            covered
                .iter()
                .map(|block| block.manuscript_block_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let first_offset = selection
        .sources
        .first()
        .and_then(|source| match source.owner {
            EditSourceOwner::Manuscript { .. } => Some(i64::from(source.from)),
            EditSourceOwner::Proposal { .. } => None,
        });
    let last_offset = selection
        .sources
        .last()
        .and_then(|source| match source.owner {
            EditSourceOwner::Manuscript { .. } => Some(i64::from(source.to)),
            EditSourceOwner::Proposal { .. } => None,
        });
    let first_operation = selection
        .sources
        .first()
        .and_then(|source| match &source.owner {
            EditSourceOwner::Manuscript { .. } => None,
            EditSourceOwner::Proposal { operation_id, .. } => Some(operation_id.as_str()),
        });
    let last_operation = selection
        .sources
        .last()
        .and_then(|source| match &source.owner {
            EditSourceOwner::Manuscript { .. } => None,
            EditSourceOwner::Proposal { operation_id, .. } => Some(operation_id.as_str()),
        });
    let first_proposal = selection
        .sources
        .first()
        .and_then(|source| match &source.owner {
            EditSourceOwner::Manuscript { .. } => None,
            EditSourceOwner::Proposal { proposal_id, .. } => Some(proposal_id.as_str()),
        });
    let last_proposal = selection
        .sources
        .last()
        .and_then(|source| match &source.owner {
            EditSourceOwner::Manuscript { .. } => None,
            EditSourceOwner::Proposal { proposal_id, .. } => Some(proposal_id.as_str()),
        });
    let (selected_operations, source_byte_limits): (Vec<String>, Vec<i64>) = selection
        .sources
        .iter()
        .filter_map(|source| match &source.owner {
            EditSourceOwner::Manuscript { .. } => None,
            EditSourceOwner::Proposal {
                proposal_id,
                operation_id,
                ..
            } => Some((
                format!("{proposal_id}:{operation_id}"),
                source.source_text.len() as i64,
            )),
        })
        .unzip();
    let row_limit = selection.sources.len() as i64 + 1;
    let inline_bound = storyos_application::AUTHOR_EDIT_INLINE_SOURCE_MAX_BYTES as i64;
    let rows = client.query(
        "SELECT proposal.proposal_id::text, operation.operation_id::text,
                revision.revision_id::text, operation.manuscript_block_id::text,
                proposal.kind, revision.generation,
                COALESCE(failure.validation, revision.validation), revision.closure,
                operation.resolution, operation.reservation_state,
                revision.base_authoritative_revision_id::text,
                CASE
                  WHEN NOT ((proposal.proposal_id::text || ':' || operation.operation_id::text) = ANY($13::text[])) THEN ''
                  WHEN row_number() OVER (PARTITION BY proposal.proposal_id, operation.operation_id ORDER BY anchor.anchor_order) > 1 THEN ''
                  WHEN octet_length(operation.candidate_text) <= $6
                   AND octet_length(operation.candidate_text) <= (
                     SELECT min(source.source_bytes) FROM unnest($13::text[], $16::bigint[]) AS source(owner_key, source_bytes)
                      WHERE source.owner_key = proposal.proposal_id::text || ':' || operation.operation_id::text)
                    THEN operation.candidate_text ELSE NULL END,
                anchor.base_authoritative_revision_id::text, anchor.manuscript_schema_version,
                anchor.coordinate_profile, anchor.range_from, anchor.range_to,
                anchor.boundary_profile, anchor.base_slice_digest
           FROM storyos.proposals AS proposal
           JOIN storyos.proposal_heads AS head USING (owner_user_id, project_id, proposal_id)
           JOIN storyos.proposal_revisions AS revision
             ON (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id) =
                (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
           JOIN storyos.proposal_operations AS operation
             ON (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
           LEFT JOIN storyos.proposal_validation_conditions AS failure
             ON (failure.owner_user_id, failure.project_id, failure.proposal_id, failure.proposal_revision_id) =
                (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)
           LEFT JOIN storyos.proposal_anchors AS anchor
             ON (anchor.owner_user_id, anchor.project_id, anchor.proposal_id, anchor.operation_id) =
                (operation.owner_user_id, operation.project_id, operation.proposal_id, operation.operation_id)
          WHERE proposal.owner_user_id = $1::text::uuid AND proposal.project_id = $2::text::uuid
            AND proposal.chapter_id = $3::text::uuid AND operation.resolution = 'pending'
            AND operation.reservation_state = 'unresolved' AND revision.closure = 'open'
            AND operation.manuscript_block_id::text = ANY($4::text[])
            AND ((proposal.proposal_id::text || ':' || operation.operation_id::text) = ANY($13::text[]) OR (
              (operation.manuscript_block_id::text <> $7::text OR proposal.kind <> 'inline_edit'
               OR anchor.range_to > COALESCE($9::bigint, (
                 SELECT min(bound.range_from) FROM storyos.proposal_anchors AS bound
                  WHERE bound.owner_user_id = proposal.owner_user_id AND bound.project_id = proposal.project_id
                    AND bound.proposal_id = $14::text::uuid AND bound.operation_id = $11::text::uuid)))
              AND (operation.manuscript_block_id::text <> $8::text OR proposal.kind <> 'inline_edit'
               OR anchor.range_from < COALESCE($10::bigint, (
                 SELECT max(bound.range_to) FROM storyos.proposal_anchors AS bound
                  WHERE bound.owner_user_id = proposal.owner_user_id AND bound.project_id = proposal.project_id
                    AND bound.proposal_id = $15::text::uuid AND bound.operation_id = $12::text::uuid)))
            ))
          ORDER BY proposal.proposal_id, operation.operation_id, anchor.anchor_order LIMIT $5",
        &[&owner, &project, &command.chapter_id, &covered_ids, &row_limit, &inline_bound, &first, &last, &first_offset, &last_offset, &first_operation, &last_operation, &selected_operations, &first_proposal, &last_proposal, &source_byte_limits],
    ).await.map_err(author_edit_database_error)?;
    let complete = rows.len() <= selection.sources.len()
        && rows
            .iter()
            .all(|row| row.get::<_, Option<String>>(11).is_some());
    let mut proposals: Vec<ProposalEditSourceFacts> = Vec::new();
    for row in rows {
        let source_owner = EditSourceOwner::Proposal {
            proposal_id: row.get(0),
            operation_id: row.get(1),
            revision_id: row.get(2),
            manuscript_block_id: row.get(3),
        };
        let anchor =
            row.get::<_, Option<String>>(12)
                .map(|base_revision_id| OpenInlineProposalAnchor {
                    manuscript_block_id: row.get(3),
                    base_authoritative_revision_id: base_revision_id,
                    manuscript_schema_version: row.get::<_, i32>(13) as u32,
                    coordinate_profile: row.get(14),
                    from: row.get::<_, i32>(15) as u32,
                    to: row.get::<_, i32>(16) as u32,
                    boundary_profile: row.get(17),
                    base_slice_digest: row.get(18),
                });
        if let Some(previous) = proposals.last_mut()
            && previous.owner == source_owner
        {
            previous.anchors.extend(anchor);
        } else {
            proposals.push(ProposalEditSourceFacts {
                owner: source_owner,
                kind: row.get(4),
                generation: row.get(5),
                validation: row.get(6),
                closure: row.get(7),
                resolution: row.get(8),
                reservation: row.get(9),
                base_revision_id: row.get(10),
                candidate_text: row.get::<_, Option<String>>(11).unwrap_or_default(),
                anchors: anchor.into_iter().collect(),
            });
        }
    }
    Ok(CurrentOrderedSourceFacts {
        blocks,
        proposals,
        complete,
    })
}

//! Prove the Inline Anchors of a conflicted Proposal Revision on the current Authoritative
//! Revision, and write the new Anchor set at an applied Replan.

use std::collections::BTreeMap;

use storyos_application::{ProjectCommandError, ProjectScope};
use storyos_core::{
    ManuscriptBlock, OpenInlineProposalAnchor, ReplanAnchorFacts, prove_replanned_inline_anchors,
};
use tokio_postgres::Client;

use crate::command_sequence::unavailable;

/// One Anchor of the new Anchor set, with the Operation that owns it.
pub(super) struct ProvenAnchor {
    operation_id: String,
    anchor_order: i32,
    anchor: OpenInlineProposalAnchor,
}

/// The new Anchor set, or `None` when one Anchor has no proof on `current_revision_id`.
pub(super) async fn prove_inline_anchors(
    client: &Client,
    scope: &ProjectScope,
    proposal_id: &str,
    conflicted_revision_id: &str,
    chapter_id: &str,
    current_revision_id: &str,
) -> Result<Option<Vec<ProvenAnchor>>, ProjectCommandError> {
    let rows = client
        .query(
            "SELECT anchor.operation_id::text, anchor.anchor_order,
                    anchor.manuscript_block_id::text,
                    anchor.base_authoritative_revision_id::text,
                    anchor.manuscript_schema_version, anchor.coordinate_profile,
                    anchor.range_from, anchor.range_to, anchor.boundary_profile,
                    anchor.base_slice_digest,
                    EXISTS (
                      SELECT 1
                        FROM storyos.proposal_operations AS reservation
                       WHERE (reservation.owner_user_id, reservation.project_id,
                              reservation.manuscript_block_id) =
                             (anchor.owner_user_id, anchor.project_id,
                              anchor.manuscript_block_id)
                         AND reservation.proposal_id <> anchor.proposal_id
                         AND reservation.reservation_state = 'unresolved'
                    )
               FROM storyos.proposal_anchors AS anchor
              WHERE anchor.owner_user_id = $1::text::uuid
                AND anchor.project_id = $2::text::uuid
                AND anchor.proposal_id = $3::text::uuid
                AND anchor.proposal_revision_id = storyos.proposal_anchor_set_revision(
                      $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)
              ORDER BY anchor.operation_id, anchor.anchor_order",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
                &conflicted_revision_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    let mut revisions = BTreeMap::new();
    let mut owners = Vec::with_capacity(rows.len());
    let mut facts = Vec::with_capacity(rows.len());
    for row in rows {
        let anchor = OpenInlineProposalAnchor {
            manuscript_block_id: row.get(/*idx*/ 2),
            base_authoritative_revision_id: row.get(/*idx*/ 3),
            manuscript_schema_version: u32::try_from(row.get::<_, i32>(/*idx*/ 4))
                .map_err(unavailable)?,
            coordinate_profile: row.get(/*idx*/ 5),
            from: u32::try_from(row.get::<_, i32>(/*idx*/ 6)).map_err(unavailable)?,
            to: u32::try_from(row.get::<_, i32>(/*idx*/ 7)).map_err(unavailable)?,
            boundary_profile: row.get(/*idx*/ 8),
            base_slice_digest: row.get(/*idx*/ 9),
        };
        for revision_id in [
            anchor.base_authoritative_revision_id.as_str(),
            current_revision_id,
        ] {
            if !revisions.contains_key(revision_id) {
                let blocks = load_blocks(client, scope, chapter_id, revision_id).await?;
                revisions.insert(revision_id.to_owned(), blocks);
            }
        }
        let block = |revision_id: &str| {
            revisions[revision_id]
                .iter()
                .find(|block| block.manuscript_block_id == anchor.manuscript_block_id)
                .cloned()
        };
        // Inline creation refuses a Block that has a reservation, so another reservation covers
        // the complete Block.
        let other_reservations = if row.get(/*idx*/ 10) {
            vec![(0, u32::MAX)]
        } else {
            Vec::new()
        };
        owners.push((row.get(/*idx*/ 0), row.get(/*idx*/ 1)));
        facts.push(ReplanAnchorFacts {
            base_block: block(&anchor.base_authoritative_revision_id),
            current_block: block(current_revision_id),
            anchor,
            other_reservations,
        });
    }
    let Ok(anchors) = prove_replanned_inline_anchors(current_revision_id, &facts) else {
        return Ok(None);
    };
    Ok(Some(
        owners
            .into_iter()
            .zip(anchors)
            .map(|((operation_id, anchor_order), anchor)| ProvenAnchor {
                operation_id,
                anchor_order,
                anchor,
            })
            .collect(),
    ))
}

async fn load_blocks(
    client: &Client,
    scope: &ProjectScope,
    chapter_id: &str,
    revision_id: &str,
) -> Result<Vec<ManuscriptBlock>, ProjectCommandError> {
    let Some(row) = client
        .query_opt(
            "SELECT convert_from(payload.canonical_bytes, 'UTF8')
               FROM storyos.authoritative_revisions AS revision
               JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id)
              WHERE revision.owner_user_id = $1::text::uuid
                AND revision.project_id = $2::text::uuid
                AND revision.manuscript_object_id = $3::text::uuid
                AND revision.revision_id = $4::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &chapter_id,
                &revision_id,
            ],
        )
        .await
        .map_err(unavailable)?
    else {
        return Ok(Vec::new());
    };
    crate::manuscript_block::load_revision_blocks(
        client,
        scope.owner_user_id.as_ref(),
        scope.project_id.as_ref(),
        chapter_id,
        revision_id,
        &row.get::<_, String>(/*idx*/ 0),
    )
    .await
    .map_err(unavailable)
}

/// Write the new Anchor set for the Proposal Revision that the Replan writes.
pub(super) async fn write_anchor_set(
    client: &Client,
    scope: &ProjectScope,
    proposal_id: &str,
    proposal_revision_id: &str,
    anchors: &[ProvenAnchor],
) -> Result<(), ProjectCommandError> {
    for proven in anchors {
        let anchor = &proven.anchor;
        let schema_version =
            i32::try_from(anchor.manuscript_schema_version).map_err(unavailable)?;
        let from = i32::try_from(anchor.from).map_err(unavailable)?;
        let to = i32::try_from(anchor.to).map_err(unavailable)?;
        client
            .execute(
                "INSERT INTO storyos.proposal_anchors
                   (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                    manuscript_block_id, base_authoritative_revision_id,
                    manuscript_schema_version, coordinate_profile, range_from, range_to,
                    boundary_profile, base_slice_digest, proposal_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, $5,
                         $6::text::uuid, $7::text::uuid, $8, $9, $10, $11, $12, $13,
                         $14::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &proposal_id,
                    &proven.operation_id,
                    &proven.anchor_order,
                    &anchor.manuscript_block_id,
                    &anchor.base_authoritative_revision_id,
                    &schema_version,
                    &anchor.coordinate_profile,
                    &from,
                    &to,
                    &anchor.boundary_profile,
                    &anchor.base_slice_digest,
                    &proposal_revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
    }
    Ok(())
}

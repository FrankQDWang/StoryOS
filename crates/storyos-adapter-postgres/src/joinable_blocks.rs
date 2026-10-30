//! The Chapter blocks that can join the Proposal of the referenced candidate (ADR 0071).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError, DeclaredTarget};
use storyos_core::DeclaredTargetKind;

/// Mark each declared Chapter block that can join the Proposal of the referenced candidates. All of
/// them are candidates of one Proposal. A Chapter text reference covers a joinable block, and the
/// block has no reservation. The joinable blocks and the pending Operations of that `block_edit`
/// Proposal make one adjacent run.
pub(crate) async fn mark(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    declared: &mut [DeclaredTarget],
) -> Result<(), CompleteAgentRunError> {
    let Some(record) = crate::candidate_revision_target::active_record(client, claim).await? else {
        return Ok(());
    };
    let references = record
        .operation_requirement
        .selection_references
        .as_deref()
        .unwrap_or_default();
    let candidates = crate::selection_references::candidate_targets(references);
    let Some((chapter_id, target, _)) = candidates.first().cloned() else {
        return Ok(());
    };
    if candidates.iter().any(|(chapter, candidate, _)| {
        *chapter != chapter_id || candidate.proposal_id != target.proposal_id
    }) {
        return Ok(());
    }
    let covered: std::collections::BTreeSet<&str> = references
        .iter()
        .filter(|admitted| {
            admitted.candidate_text.is_none() && admitted.reference.chapter_id() == chapter_id
        })
        .flat_map(|admitted| admitted.manuscript_block_ids.iter().map(String::as_str))
        .collect();
    let rows = client
        .query(
            "SELECT member.manuscript_block_id::text, reservation.proposal_id::text,
                    proposal.kind
               FROM storyos.authoritative_heads AS head
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
               LEFT JOIN storyos.proposals AS proposal
                 ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                    (reservation.owner_user_id, reservation.project_id, reservation.proposal_id)
              WHERE head.owner_user_id = $1::text::uuid AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid
              ORDER BY member.block_order",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(crate::agent_run_work::complete_database_error)?;
    let blocks: Vec<(String, Option<String>, Option<String>)> = rows
        .iter()
        .map(|row| (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2)))
        .collect();
    let own: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, (_, proposal, _))| proposal.as_deref() == Some(target.proposal_id.as_str()))
        .map(|(index, _)| index)
        .collect();
    let (Some(&first), Some(&last)) = (own.first(), own.last()) else {
        return Ok(());
    };
    if last - first + 1 != own.len() || blocks[first].2.as_deref() != Some("block_edit") {
        return Ok(());
    }
    // The reverse collect keeps the first declared position of each Block ID.
    let positions: std::collections::BTreeMap<&str, usize> = declared
        .iter()
        .enumerate()
        .filter(|(_, declared)| {
            declared.chapter_id == chapter_id && declared.kind == DeclaredTargetKind::ChapterBlock
        })
        .map(|(position, declared)| (declared.block_id.as_str(), position))
        .rev()
        .collect();
    let joinable = |index: usize| {
        let (block_id, reservation, _) = &blocks[index];
        let position = *positions.get(block_id.as_str())?;
        (reservation.is_none() && covered.contains(block_id.as_str())).then_some(position)
    };
    let upward = (1..)
        .zip((0..first).rev())
        .map_while(|(distance, index)| Some((joinable(index)?, -distance)));
    let downward = (1..)
        .zip(last + 1..blocks.len())
        .map_while(|(distance, index)| Some((joinable(index)?, distance)));
    let joined: Vec<(usize, i32)> = upward.chain(downward).collect();
    for (position, distance) in joined {
        declared[position].kind = DeclaredTargetKind::JoinableBlock { distance };
    }
    Ok(())
}

//! Proposal Growth: the joined blocks of one candidate revision (ADR 0071).

use super::agent_run_work::complete_database_error as database_error;
use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{OpenBlockProposal, OpenBlockProposalResult, ProseChangeCandidate};

/// Whether each joined block is still in the current head at the base of the candidate, with no
/// reservation. The caller holds the lock of the Proposal head.
pub(crate) async fn joined_blocks_are_free(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    base_revision_id: &str,
    joined: &[&ProseChangeCandidate],
) -> Result<bool, CompleteAgentRunError> {
    for change in joined {
        let current = crate::admitted_proposal_target::load_current_target(
            client,
            claim,
            &change.chapter_id,
            &change.manuscript_block_id,
        )
        .await?;
        let result = storyos_core::open_block_proposal(&OpenBlockProposal {
            scope_matches: true,
            target_block_present: current.revision_id.is_some(),
            expected_base_revision_id: base_revision_id.to_owned(),
            current_base_revision_id: current.revision_id,
            conflicting_reservation: current.reserved,
        });
        if change.base_authoritative_revision_id != base_revision_id
            || !matches!(result, OpenBlockProposalResult::Applied)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Add one pending Operation with its reservation for each joined block, and make the bundle of
/// the Proposal atomic. Returns the new Operation IDs in the order of `joined`.
pub(crate) async fn join(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    proposal_id: &str,
    joined: &[&ProseChangeCandidate],
) -> Result<Vec<String>, CompleteAgentRunError> {
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    let mut operation_ids = Vec::with_capacity(joined.len());
    for change in joined {
        let operation_id = uuid::Uuid::now_v7().to_string();
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
                    &change.manuscript_block_id,
                    &change.candidate_text,
                ],
            )
            .await
            .map_err(database_error)?;
        operation_ids.push(operation_id);
    }
    client
        .execute(
            "UPDATE storyos.proposals SET bundle_policy = 'atomic'
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid",
            &[&owner, &project, &proposal_id],
        )
        .await
        .map_err(database_error)?;
    Ok(operation_ids)
}

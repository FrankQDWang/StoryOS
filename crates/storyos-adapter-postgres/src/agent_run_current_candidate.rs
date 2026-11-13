//! The current revision of each pending candidate that a Run references. When the author
//! changes such a candidate during the Run, the next RunStep reads its current revision (ADR 0085).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{CurrentCandidate, CurrentPassageAssemblyRecord, ProposalCandidateTarget};

use crate::agent_run_work::complete_database_error;

/// Moves the candidate target and the candidate references of the requirement of `retained` to the current
/// revision of each candidate that is still pending. A candidate that is no longer pending stays
/// at its revision, so the Context Assembly finds it unavailable as before.
pub(crate) async fn follow(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    retained: &mut CurrentPassageAssemblyRecord,
) -> Result<(), CompleteAgentRunError> {
    let requirement = &mut retained.operation_requirement;
    let targets: Vec<ProposalCandidateTarget> = requirement
        .candidate_target
        .iter()
        .cloned()
        .chain(
            requirement
                .selection_references
                .iter()
                .flatten()
                .filter_map(|admitted| admitted.reference.candidate_target()),
        )
        .collect();
    if targets.is_empty() {
        return Ok(());
    }
    let (proposals, operations): (Vec<String>, Vec<String>) = targets
        .into_iter()
        .map(|target| (target.proposal_id, target.operation_id))
        .unzip();
    let rows = client
        .query(
            "SELECT DISTINCT operation.proposal_id::text, operation.operation_id::text,
                    head.current_revision_id::text, operation.candidate_text
               FROM unnest($3::text[], $4::text[]) AS target(proposal_id, operation_id)
               JOIN storyos.proposal_operations AS operation
                 ON operation.owner_user_id = $1::text::uuid AND operation.project_id = $2::text::uuid
                AND operation.proposal_id = target.proposal_id::uuid
                AND operation.operation_id = target.operation_id::uuid
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id)
                  = (operation.owner_user_id, operation.project_id, operation.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)
                  = (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
              WHERE revision.closure = 'open' AND operation.resolution = 'pending'
                AND operation.reservation_state = 'unresolved'",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &proposals,
                &operations,
            ],
        )
        .await
        .map_err(complete_database_error)?;
    let current: Vec<CurrentCandidate<'_>> = rows
        .iter()
        .map(|row| CurrentCandidate {
            target: ProposalCandidateTarget {
                proposal_id: row.get(/*idx*/ 0),
                operation_id: row.get(/*idx*/ 1),
                revision_id: row.get(/*idx*/ 2),
            },
            candidate_text: row.get(/*idx*/ 3),
        })
        .collect();
    if let Some(target) = requirement.candidate_target.as_mut()
        && let Some(candidate) = current.iter().find(|candidate| {
            candidate.target.proposal_id == target.proposal_id
                && candidate.target.operation_id == target.operation_id
        })
    {
        target.revision_id.clone_from(&candidate.target.revision_id);
    }
    storyos_core::follow_current_candidates(
        requirement.selection_references.iter_mut().flatten(),
        &current,
    );
    Ok(())
}

#[cfg(test)]
#[path = "agent_run_current_candidate_tests.rs"]
mod tests;

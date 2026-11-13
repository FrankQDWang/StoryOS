//! The current revision of each pending candidate that a Run references. When the author
//! changes such a candidate during the Run, the next RunStep reads its current revision (ADR 0085).

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{
    AdmittedSelection, CurrentCandidate, CurrentPassageAssemblyRecord, ProposalCandidateTarget,
    SelectionReference,
};

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
    let rows = current_rows(client, claim, targets).await?;
    let current = current_candidates(&rows);
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

/// Moves each candidate reference of a steering input to the current revision of its candidate
/// when that candidate is still pending. A moved reference covers all of the current text.
pub(crate) async fn follow_added(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    added: &mut [SelectionReference],
) -> Result<(), CompleteAgentRunError> {
    let targets = added
        .iter()
        .filter_map(SelectionReference::candidate_target)
        .collect();
    let rows = current_rows(client, claim, targets).await?;
    let mut admitted: Vec<AdmittedSelection> = added
        .iter()
        .map(|reference| AdmittedSelection {
            reference: reference.clone(),
            selected_text: None,
            manuscript_block_ids: Vec::new(),
            candidate_text: None,
        })
        .collect();
    storyos_core::follow_current_candidates(&mut admitted, &current_candidates(&rows));
    for (reference, followed) in added.iter_mut().zip(admitted) {
        *reference = followed.reference;
    }
    Ok(())
}

fn current_candidates(rows: &[tokio_postgres::Row]) -> Vec<CurrentCandidate<'_>> {
    rows.iter()
        .map(|row| CurrentCandidate {
            target: ProposalCandidateTarget {
                proposal_id: row.get(/*idx*/ 0),
                operation_id: row.get(/*idx*/ 1),
                revision_id: row.get(/*idx*/ 2),
            },
            candidate_text: row.get(/*idx*/ 3),
        })
        .collect()
}

/// The head revision and the candidate text of each of `targets` that is still pending.
async fn current_rows(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    targets: Vec<ProposalCandidateTarget>,
) -> Result<Vec<tokio_postgres::Row>, CompleteAgentRunError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let (proposals, operations): (Vec<String>, Vec<String>) = targets
        .into_iter()
        .map(|target| (target.proposal_id, target.operation_id))
        .unzip();
    client
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
        .map_err(complete_database_error)
}

#[cfg(test)]
#[path = "agent_run_current_candidate_tests.rs"]
mod tests;

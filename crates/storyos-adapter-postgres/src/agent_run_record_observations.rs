//! The in-process execution of a `record_memory_observations` call (ADR 0094 sections 5 and 8).
//! Core settles the call: it validates the list against the current entries of the author, decides
//! the outcome of the capture, and decides whether the call completes the Run. The store writes that settlement in the Tool Gateway
//! transaction that checked the Run Lease fence, so no request follows a settled call.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{MemoryEntryOrigin, ObservationCallSettlement, ObservationCandidate};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    candidates: &[ObservationCandidate],
) -> Result<(), CompleteAgentRunError> {
    // A conflict item names a current entry of the author of the Project of the Run.
    let entries = crate::memory_consolidation_versions::pass_entries(client, &claim.project_scope)
        .await
        .map_err(CompleteAgentRunError::Unavailable)?;
    let author_entries: Vec<u64> = (entries.iter())
        .filter(|item| item.entry.origin == MemoryEntryOrigin::AuthorRequest)
        .map(|item| item.entry.entry.number)
        .collect();
    let settlement = storyos_core::settle_observation_call(candidates, &author_entries);
    let (refusal, result) = match &settlement {
        ObservationCallSettlement::Settled {
            outcome,
            observations,
            result,
        } => {
            crate::memory_capture::record_observations(client, claim, *outcome, observations)
                .await?;
            (None, result)
        }
        ObservationCallSettlement::Refused { refusal, result } => (Some(refusal.label()), result),
    };
    resolve(
        client,
        claim,
        row,
        Resolution {
            state: settlement.call_state(),
            refusal,
            attempt: None,
            outcome: None,
            result: Some(result),
        },
    )
    .await?;
    if settlement.completes_run() {
        crate::agent_run_work::complete(client, claim).await?;
    }
    Ok(())
}

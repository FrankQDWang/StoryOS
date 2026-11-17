//! The in-process execution of a `record_memory_observations` call (ADR 0094 sections 5 and 8).
//! Core settles the call: it validates the list, decides the outcome of the capture, and decides
//! whether the call completes the Run. The store writes that settlement in the Tool Gateway
//! transaction that checked the Run Lease fence, so no request follows a settled call.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ObservationCallSettlement, ObservationCandidate};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    candidates: &[ObservationCandidate],
) -> Result<(), CompleteAgentRunError> {
    let settlement = storyos_core::settle_observation_call(candidates);
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

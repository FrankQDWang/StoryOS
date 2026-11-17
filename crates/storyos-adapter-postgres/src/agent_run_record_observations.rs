//! The in-process execution of a `record_memory_observations` call (ADR 0094 sections 5 and 8).
//! Core validates the list. A refused list settles the call as refused with its result, and the
//! model can call again in the next RunStep. A valid list stores the observations, settles the
//! capture, settles the call, and completes the Run in the Tool Gateway transaction that checked
//! the Run Lease fence, so no request follows it.

use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{ObservationCandidate, ToolCallState};

use crate::agent_run_tool_authority::{Resolution, ToolCallRow, resolve};

pub(crate) async fn execute(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    row: &ToolCallRow,
    candidates: &[ObservationCandidate],
) -> Result<(), CompleteAgentRunError> {
    let (state, refusal, result) = match storyos_core::validate_observations(candidates) {
        Ok(observations) => {
            crate::memory_capture::record_observations(client, claim, &observations).await?;
            let result = storyos_core::observations_result(observations.len());
            (ToolCallState::Settled, None, result)
        }
        Err(refusal) => (
            ToolCallState::Refused,
            Some(refusal.label()),
            refusal.result(),
        ),
    };
    resolve(
        client,
        claim,
        row,
        Resolution {
            state,
            refusal,
            attempt: None,
            outcome: None,
            result: Some(&result),
        },
    )
    .await?;
    if state == ToolCallState::Settled {
        crate::agent_run_work::complete(client, claim).await?;
    }
    Ok(())
}

//! The Tool Gateway: the only owner of the dispatch order for the ToolCalls of one durable Agent
//! Decision (ADR 0034, ADR 0039, ADR 0084).

use crate::model_gateway_ports::{GatewayClock, ModelDispatchStore};
use crate::model_lease_renewal::renew_while;
use crate::tool_gateway_ports::{ToolDestinationAdapter, ToolDispatchRecord, ToolDispatchStore};
use crate::{ClaimedAgentRun, CompleteAgentRunError};

/// Dispatches each pending ToolCall in item order: gets the next ToolCall, prepares, commits the
/// dispatch claim, exchanges with no open transaction, and records the observation after a Run
/// Lease check. Returns when each ToolCall is resolved.
pub(crate) async fn dispatch_tool_calls(
    store: &(impl ModelDispatchStore + ToolDispatchStore),
    tools: &impl ToolDestinationAdapter,
    clock: &impl GatewayClock,
    claim: &ClaimedAgentRun,
) -> Result<(), CompleteAgentRunError> {
    while let Some(request) = store.next_tool_call(claim).await? {
        let prepared = match tools.prepare(&request).await {
            Ok(prepared) => prepared,
            Err(refusal) => {
                store
                    .record_tool_call(
                        claim,
                        ToolDispatchRecord::Refusal {
                            request: &request,
                            refusal,
                        },
                    )
                    .await?;
                continue;
            }
        };
        let Some(dispatch) = store
            .commit_tool_dispatch_claim(claim, &request, &prepared.projection)
            .await?
        else {
            continue;
        };
        let observation = renew_while(store, clock, claim, tools.exchange(prepared.prepared)).await;
        store
            .record_tool_call(
                claim,
                ToolDispatchRecord::Exchange {
                    claim: &dispatch,
                    request: &request,
                    observation,
                },
            )
            .await?;
    }
    Ok(())
}

//! The Tool destination port of the MCP client: one Dispatch exchange for each claimed ToolCall.

use storyos_application::{
    CredentialResolver, PreDispatchRefusal, PreparedRequest, ToolCallRequest,
    ToolDestinationAdapter,
};
use storyos_core::ToolObservation;

use crate::{CallResult, McpClient, McpExchange, McpObservation, McpRequest};

impl<R: CredentialResolver> ToolDestinationAdapter for McpClient<R> {
    type Prepared = McpExchange;

    async fn prepare(
        &self,
        request: &ToolCallRequest,
    ) -> Result<PreparedRequest<McpExchange>, PreDispatchRefusal> {
        let dispatch = McpRequest::Dispatch {
            server_url: request.endpoint.clone(),
            registration: request.registration,
            tool: request.tool.name.to_owned(),
            arguments: request.server_arguments.clone(),
        };
        McpClient::prepare(self, &dispatch, request.credential_reference.as_ref()).await
    }

    async fn exchange(&self, prepared: McpExchange) -> ToolObservation {
        match McpClient::exchange(self, prepared).await {
            McpObservation::NotSubmitted => ToolObservation::NotSubmitted,
            McpObservation::Rejected { reason } => ToolObservation::Rejected { reason },
            McpObservation::JsonRpcError { code } => ToolObservation::JsonRpcError { code },
            McpObservation::InvalidResult { reason } => ToolObservation::InvalidResult { reason },
            // A Dispatch never answers with a tool list.
            McpObservation::Discovered(_) => ToolObservation::InvalidResult {
                reason: "discovered",
            },
            McpObservation::Called(CallResult { text, is_error, .. }) => {
                ToolObservation::Called { text, is_error }
            }
            McpObservation::Drift(differences) => ToolObservation::Drift(differences),
            McpObservation::OutcomeUnknown => ToolObservation::OutcomeUnknown,
        }
    }
}

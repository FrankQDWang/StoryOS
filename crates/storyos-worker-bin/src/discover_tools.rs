//! One `Discover` exchange with the Exa MCP server for a research record (ADR 0083). It writes
//! the Tool Discovery Record as JSON to stdout.

use storyos_adapter_mcp_client::{DISCOVERY_TIMEOUT, McpClient, McpObservation, McpRequest};
use storyos_application::{CredentialReference, PreDispatchRefusal};
use storyos_core::{EXA_CREDENTIAL_REFERENCE, EXA_MCP_SERVER_URL};

use crate::credential::SettingsCredentials;

/// The exit code: 0 for a record, 1 for each other observation, 2 for a refusal before dispatch.
pub(crate) async fn discover_exa(resolver: SettingsCredentials) -> i32 {
    let client = McpClient {
        resolver,
        timeout: DISCOVERY_TIMEOUT,
    };
    let request = McpRequest::Discover {
        server_url: EXA_MCP_SERVER_URL.to_owned(),
    };
    let reference = CredentialReference(EXA_CREDENTIAL_REFERENCE.to_owned());
    let prepared = match client.prepare(&request, Some(&reference)).await {
        Ok(prepared) => prepared,
        Err(PreDispatchRefusal::CredentialUnavailable) => {
            tracing::error!(reason = "credential_unavailable", "mcp discovery refused");
            return 2;
        }
        Err(PreDispatchRefusal::UnsupportedRequest) => {
            tracing::error!(reason = "unsupported_request", "mcp discovery refused");
            return 2;
        }
    };
    match client.exchange(prepared.prepared).await {
        McpObservation::Discovered(record) => match serde_json::to_string_pretty(&record) {
            Ok(json) => {
                println!("{json}");
                0
            }
            Err(_) => 1,
        },
        McpObservation::NotSubmitted
        | McpObservation::Rejected { .. }
        | McpObservation::JsonRpcError { .. }
        | McpObservation::InvalidResult { .. }
        | McpObservation::Called(_)
        | McpObservation::Drift(_)
        | McpObservation::OutcomeUnknown => 1,
    }
}

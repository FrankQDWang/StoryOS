//! The MCP client adapter: the Streamable HTTP transport of one remote MCP server (ADR 0039,
//! ADR 0083).
//!
//! The transport follows the MCP specification revision `2025-11-25`, read on 2026-11-03 at
//! <https://modelcontextprotocol.io/specification/2025-11-25/basic/transports>,
//! <https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle>, and
//! <https://modelcontextprotocol.io/specification/2025-11-25/server/tools>.

mod record;
mod session;

use std::time::Duration;

use serde_json::{Map, Value, json};
use storyos_application::{
    CredentialReference, CredentialResolver, DiagnosticField as _, DiagnosticId,
    PreDispatchRefusal, PreparedRequest, ResolvedCredential, WirePayloadProjection,
};

pub use record::{CallResult, DiscoveredTool, ToolDiscoveryRecord};

/// The one MCP protocol revision that the client sends and accepts.
pub const PROTOCOL_REVISION: &str = "2025-11-25";

const EXECUTION_PROFILE: &str = "storyos.mcp-streamable-http.v1";

/// One closed request to an MCP server. Each exchange opens its own session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpRequest {
    /// `initialize`, then each page of `tools/list`.
    Discover { server_url: String },
    /// `initialize`, then one `tools/call`.
    Call {
        server_url: String,
        tool: String,
        arguments: Map<String, Value>,
    },
}

/// The complete result of one exchange. It has no error channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpObservation {
    /// The operation request did not leave this process.
    NotSubmitted,
    /// The server refused an HTTP request, with the status and no body text.
    Rejected {
        reason: String,
    },
    /// A native JSON-RPC error. The message text is not kept.
    JsonRpcError {
        code: i64,
    },
    /// The server answered, but the answer failed the shape check.
    InvalidResult {
        reason: &'static str,
    },
    Discovered(ToolDiscoveryRecord),
    Called(CallResult),
    /// A timeout, a dropped body, or a server error after the operation request left.
    OutcomeUnknown,
}

/// Sends each prepared exchange once, with no retry, redirect, or proxy. It resolves the
/// Credential Reference through `resolver` before the dispatch claim.
pub struct McpClient<R> {
    pub resolver: R,
    /// The bound of one complete exchange.
    pub timeout: Duration,
}

/// The single-use prepared exchange. It holds the `x-api-key` value until the exchange ends.
pub struct McpExchange {
    request: McpRequest,
    credential: ResolvedCredential,
}

impl<R: CredentialResolver> McpClient<R> {
    /// Refuses a missing or unresolved Credential Reference, and a server URL that is not a
    /// plain HTTP or HTTPS URL, before the dispatch claim.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn prepare(
        &self,
        request: &McpRequest,
        credential_reference: Option<&CredentialReference>,
    ) -> Result<PreparedRequest<McpExchange>, PreDispatchRefusal> {
        let (McpRequest::Discover { server_url } | McpRequest::Call { server_url, .. }) = request;
        if !plain_url(server_url) {
            return Err(PreDispatchRefusal::UnsupportedRequest);
        }
        let operation = match request {
            McpRequest::Discover { .. } => json!({"method": "tools/list"}),
            McpRequest::Call {
                tool, arguments, ..
            } => json!({
                "method": "tools/call",
                "params": {"name": tool, "arguments": arguments},
            }),
        };
        let wire = storyos_core::canonical_json(&json!({
            "server_url": server_url,
            "protocol_revision": PROTOCOL_REVISION,
            "initialize": session::initialize_params(),
            "operation": operation,
        }));
        let Some(reference) = credential_reference else {
            return Err(PreDispatchRefusal::CredentialUnavailable);
        };
        let credential = self
            .resolver
            .resolve(reference)
            .await
            .ok_or(PreDispatchRefusal::CredentialUnavailable)?;
        Ok(PreparedRequest {
            projection: WirePayloadProjection {
                execution_profile: EXECUTION_PROFILE.to_owned(),
                mapping_revision: PROTOCOL_REVISION.to_owned(),
                digest: format!("sha256:{}", storyos_core::hex_sha256(wire.as_bytes())),
                serialized_payload: Some(wire),
                cache_setting: None,
            },
            prepared: McpExchange {
                request: request.clone(),
                credential,
            },
        })
    }

    /// Consumes the prepared value, and drops the key when the exchange ends.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn exchange(&self, prepared: McpExchange) -> McpObservation {
        let McpExchange {
            request,
            credential,
        } = prepared;
        let (McpRequest::Discover { server_url } | McpRequest::Call { server_url, .. }) = &request;
        let observation = tokio::time::timeout(
            self.timeout,
            session::run(server_url, &credential, &request),
        )
        .await
        .unwrap_or(McpObservation::OutcomeUnknown);
        tracing::info!(
            server_url = DiagnosticId(server_url).diagnostic(),
            observation = observation.category().diagnostic(),
            "mcp exchange"
        );
        observation
    }
}

impl McpObservation {
    fn category(&self) -> &'static str {
        match self {
            Self::NotSubmitted => "not_submitted",
            Self::Rejected { .. } => "rejected",
            Self::JsonRpcError { .. } => "json_rpc_error",
            Self::InvalidResult { reason } => reason,
            Self::Discovered(_) => "discovered",
            Self::Called(_) => "called",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }
}

/// A URL with no credential, query, or fragment, so that a diagnostic line can carry it.
fn plain_url(text: &str) -> bool {
    reqwest::Url::parse(text).is_ok_and(|url| {
        matches!(url.scheme(), "https" | "http")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

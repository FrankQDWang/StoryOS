//! The MCP client adapter: the Streamable HTTP transport of one remote MCP server (ADR 0039,
//! ADR 0083).
//!
//! The transport follows the MCP specification revision `2025-11-25`, read on 2026-11-03 at
//! <https://modelcontextprotocol.io/specification/2025-11-25/basic/transports>,
//! <https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle>, and
//! <https://modelcontextprotocol.io/specification/2025-11-25/server/tools>.
//!
//! Bounds: one response body is read up to 1 MiB, and the total text of a Call result is at most
//! 256 KiB. The one-off discovery of the Worker binary bounds its exchange with
//! `DISCOVERY_TIMEOUT`. The specification treats the `Mcp-Session-Id` as security-sensitive, so a
//! diagnostic line has only the first 8 hex digits of its SHA-256.
//!
//! Each exchange opens its own session and sends each request one time. When the server ends the
//! session with HTTP 404, the observation is `Rejected { reason: "session_ended" }`. A resend is a
//! new Destination Attempt (ADR 0039), and its exchange starts a new session.

mod contract_check;
mod record;
mod session;
mod tool_destination;

use std::time::Duration;

use serde_json::{Map, Value, json};
use storyos_application::{
    CredentialReference, CredentialResolver, DiagnosticField as _, DiagnosticId,
    PreDispatchRefusal, PreparedRequest, ResolvedCredential, WirePayloadProjection,
};
use storyos_core::{ContractDifference, ToolRegistration};

pub use record::{CallResult, DiscoveredTool, ToolDiscoveryRecord};

/// The one MCP protocol revision that the client speaks. A Discover of a server that answers
/// another revision records that revision with no tool list.
pub const PROTOCOL_REVISION: &str = "2025-11-25";

/// The bound of the one-off `Discover` exchange of a research record.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);

const EXECUTION_PROFILE: &str = storyos_core::MCP_STREAMABLE_HTTP_ADAPTER_REVISION;

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
    /// `initialize`, each page of `tools/list`, the Tool Contract Drift check against
    /// `registration`, and then one `tools/call` of `tool` only when the contract has no drift.
    Dispatch {
        server_url: String,
        registration: &'static ToolRegistration,
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
    /// The fresh tool list of a Dispatch drifted from the pinned contract, so `tools/call` was
    /// not sent.
    Drift(Vec<ContractDifference>),
    /// A timeout or a dropped body after the operation request left.
    OutcomeUnknown,
    /// The server answered the operation request with an HTTP 5xx status. It received the
    /// request, and the result is not known.
    ServerError {
        reason: String,
    },
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
    /// The exact `tools/call` body of a Dispatch, which the projection records.
    call_body: Option<String>,
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
        let server_url = request.server_url();
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
            McpRequest::Dispatch { .. } => Value::Null,
        };
        // A Dispatch records the exact JSON-RPC body of its `tools/call`. The other requests
        // record a description of their session.
        let call_body = match request {
            McpRequest::Dispatch {
                tool, arguments, ..
            } => Some(session::dispatch_call_body(tool, arguments)),
            McpRequest::Discover { .. } | McpRequest::Call { .. } => None,
        };
        let wire = call_body.clone().unwrap_or_else(|| {
            storyos_core::canonical_json(&json!({
                "server_url": server_url,
                "protocol_revision": PROTOCOL_REVISION,
                "initialize": session::initialize_params(),
                "operation": operation,
            }))
        });
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
                call_body,
            },
        })
    }

    /// Consumes the prepared value, and drops the key when the exchange ends.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn exchange(&self, prepared: McpExchange) -> McpObservation {
        let McpExchange {
            request,
            credential,
            call_body,
        } = prepared;
        let server_url = request.server_url();
        let observation = session::run(
            server_url,
            &credential,
            &request,
            call_body.as_deref(),
            self.timeout,
        )
        .await;
        tracing::info!(
            server_url = DiagnosticId(server_url).diagnostic(),
            observation = observation.category().diagnostic(),
            "mcp exchange"
        );
        observation
    }
}

impl McpRequest {
    fn server_url(&self) -> &str {
        let (Self::Discover { server_url }
        | Self::Call { server_url, .. }
        | Self::Dispatch { server_url, .. }) = self;
        server_url
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
            Self::Drift(_) => "drift",
            Self::OutcomeUnknown => "outcome_unknown",
            Self::ServerError { .. } => "server_error",
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

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;

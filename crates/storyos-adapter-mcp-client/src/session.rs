//! One MCP session over the Streamable HTTP transport: each JSON-RPC message is one POST.

use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
use serde_json::{Map, Value, json};
use storyos_application::{DiagnosticField as _, DiagnosticId, ResolvedCredential};
use storyos_core::ContractCheck;
use tokio::time::Instant;

use crate::record::{self, DiscoveredTool, MAX_TEXT_BYTES, ToolDiscoveryRecord};
use crate::{McpObservation, McpRequest, PROTOCOL_REVISION};

const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";
const KEY_HEADER: &str = "x-api-key";
/// The read bound of one response body, 1 MiB: four times the text bound of a Call, for JSON
/// escapes and a structured copy of the text.
const MAX_BODY_BYTES: usize = 4 * MAX_TEXT_BYTES;

/// The `initialize` parameters. The client declares no capability, so a server request is never
/// answered.
pub(crate) fn initialize_params() -> Value {
    json!({
        "protocolVersion": PROTOCOL_REVISION,
        "capabilities": {},
        "clientInfo": {"name": "storyos", "version": env!("CARGO_PKG_VERSION")},
    })
}

/// Why one message of the session gave no result.
enum Failure {
    /// The request did not leave this process.
    Unsent,
    Rejected(String),
    JsonRpc(i64),
    Invalid(&'static str),
    /// The request left, and its result is not known.
    Lost,
    /// HTTP 404 to a request with a session identifier: the server ended the session and did
    /// not process the request.
    SessionEnded,
}

impl Failure {
    /// Before the operation request, a lost answer proves that the operation was not sent.
    fn before_operation(self) -> McpObservation {
        match self {
            Self::Unsent | Self::Lost => McpObservation::NotSubmitted,
            other => other.after_operation(),
        }
    }

    fn after_operation(self) -> McpObservation {
        match self {
            Self::Unsent => McpObservation::NotSubmitted,
            Self::Rejected(reason) => McpObservation::Rejected { reason },
            Self::JsonRpc(code) => McpObservation::JsonRpcError { code },
            Self::Invalid(reason) => McpObservation::InvalidResult { reason },
            Self::Lost => McpObservation::OutcomeUnknown,
            // The exchange does not send the request again: each resend is a new Destination
            // Attempt (ADR 0039). The next exchange starts a new session.
            Self::SessionEnded => McpObservation::Rejected {
                reason: "session_ended".to_owned(),
            },
        }
    }
}

struct Session<'a> {
    client: reqwest::Client,
    url: &'a str,
    credential: &'a ResolvedCredential,
    session_id: Option<String>,
    /// The first 8 hex digits of the SHA-256 of the session identifier, or `none`. The
    /// specification treats the identifier as security-sensitive, so no diagnostic line has it.
    session_tag: String,
    next_id: u64,
    /// The response bytes of the current message.
    received: u64,
    /// The end of the complete exchange.
    deadline: Instant,
}

/// `initialize`, `notifications/initialized`, and then the operation of `request`.
pub(crate) async fn run(
    url: &str,
    credential: &ResolvedCredential,
    request: &McpRequest,
    timeout: Duration,
) -> McpObservation {
    let Ok(client) = reqwest::Client::builder()
        .http1_only()
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
    else {
        return McpObservation::NotSubmitted;
    };
    let mut session = Session {
        client,
        url,
        credential,
        session_id: None,
        session_tag: "none".to_owned(),
        next_id: 1,
        received: 0,
        deadline: Instant::now() + timeout,
    };
    let identity = match session.request("initialize", initialize_params()).await {
        Ok(result) => record::server_identity(&result),
        Err(failure) => return failure.before_operation(),
    };
    let (server_name, server_version, protocol_revision) = match identity {
        Ok(identity) => identity,
        Err(reason) => return McpObservation::InvalidResult { reason },
    };
    if protocol_revision != PROTOCOL_REVISION {
        // The client sends nothing more in a revision that it does not speak.
        return match request {
            McpRequest::Discover { .. } => McpObservation::Discovered(ToolDiscoveryRecord {
                server_url: url.to_owned(),
                server_name,
                server_version,
                protocol_revision,
                tools: Vec::new(),
            }),
            McpRequest::Call { .. } | McpRequest::Dispatch { .. } => {
                McpObservation::InvalidResult {
                    reason: "protocol_revision",
                }
            }
        };
    }
    if let Err(failure) = session.notify("notifications/initialized").await {
        return failure.before_operation();
    }
    match request {
        McpRequest::Discover { .. } => match session.list_tools().await {
            Ok(tools) => McpObservation::Discovered(ToolDiscoveryRecord {
                server_url: url.to_owned(),
                server_name,
                server_version,
                protocol_revision,
                tools,
            }),
            Err(observation) => observation,
        },
        McpRequest::Call {
            tool, arguments, ..
        } => session.call(tool, arguments).await,
        McpRequest::Dispatch {
            registration,
            tool,
            arguments,
            ..
        } => {
            // A failed list stops the exchange before `tools/call`, so no argument left.
            let Ok(tools) = session.list_tools().await else {
                return McpObservation::NotSubmitted;
            };
            // The endpoint is the evidence of the admitted Processing Destination Identity, so
            // the check compares the server identity and the tool contract.
            let record = ToolDiscoveryRecord {
                server_url: registration.server_url.to_owned(),
                server_name,
                server_version,
                protocol_revision,
                tools,
            };
            match record.check(registration) {
                ContractCheck::Drift(differences) => McpObservation::Drift(differences),
                ContractCheck::Unchanged | ContractCheck::ProvenanceChange => {
                    session.call(tool, arguments).await
                }
            }
        }
    }
}

impl Session<'_> {
    /// Each page of `tools/list`.
    async fn list_tools(&mut self) -> Result<Vec<DiscoveredTool>, McpObservation> {
        let mut tools = Vec::new();
        let mut cursor = None;
        loop {
            let params = match &cursor {
                Some(cursor) => json!({"cursor": cursor}),
                None => json!({}),
            };
            let page = self
                .request("tools/list", params)
                .await
                .map_err(Failure::after_operation)?;
            match record::tools_page(&page, &mut tools) {
                Ok(Some(next)) => cursor = Some(next),
                Ok(None) => return Ok(tools),
                Err(reason) => return Err(McpObservation::InvalidResult { reason }),
            }
        }
    }

    /// One `tools/call`.
    async fn call(&mut self, tool: &str, arguments: &Map<String, Value>) -> McpObservation {
        let params = json!({"name": tool, "arguments": arguments});
        match self.request("tools/call", params).await {
            Ok(result) => match record::call_result(result) {
                Ok(result) => McpObservation::Called(result),
                Err(reason) => McpObservation::InvalidResult { reason },
            },
            Err(failure) => failure.after_operation(),
        }
    }

    async fn request(&mut self, method: &'static str, params: Value) -> Result<Value, Failure> {
        let id = self.next_id;
        self.next_id += 1;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let sent = body.to_string();
        let sent_bytes = sent.len() as u64;
        self.received = 0;
        let reply = match self.post(method, sent).await {
            Ok(response) => self.reply(response, id).await,
            Err(failure) => Err(failure),
        };
        self.trace(method, sent_bytes);
        reply
    }

    /// Sends one notification. The server accepts it with no body.
    async fn notify(&mut self, method: &'static str) -> Result<(), Failure> {
        let sent = json!({"jsonrpc": "2.0", "method": method}).to_string();
        let sent_bytes = sent.len() as u64;
        self.received = 0;
        let posted = self.post(method, sent).await.map(drop);
        self.trace(method, sent_bytes);
        posted
    }

    async fn post(&mut self, method: &str, body: String) -> Result<reqwest::Response, Failure> {
        let Ok(mut key) = HeaderValue::from_str(self.credential.expose()) else {
            return Err(Failure::Unsent);
        };
        key.set_sensitive(true);
        let mut request = self
            .client
            .post(self.url)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream")
            .header(KEY_HEADER, key);
        if method != "initialize" {
            request = request.header(VERSION_HEADER, PROTOCOL_REVISION);
        }
        if let Some(session_id) = &self.session_id {
            request = request.header(SESSION_HEADER, session_id);
        }
        // A timeout proves nothing about the request, also before the connection.
        let response = match tokio::time::timeout_at(self.deadline, request.body(body).send()).await
        {
            Ok(Ok(response)) => response,
            Ok(Err(error)) if error.is_builder() || error.is_connect() => {
                return Err(Failure::Unsent);
            }
            Ok(Err(_)) | Err(_) => return Err(Failure::Lost),
        };
        let status = response.status();
        if status.is_server_error() {
            return Err(Failure::Lost);
        }
        if status == reqwest::StatusCode::NOT_FOUND && self.session_id.is_some() {
            return Err(Failure::SessionEnded);
        }
        if !status.is_success() {
            return Err(Failure::Rejected(format!("http_{}", status.as_u16())));
        }
        if method == "initialize"
            && let Some(value) = response.headers().get(SESSION_HEADER)
        {
            // The specification permits only visible ASCII in a session identifier.
            let bytes = value.as_bytes();
            if bytes.is_empty() || !bytes.iter().all(|byte| (0x21..=0x7e).contains(byte)) {
                return Err(Failure::Invalid("invalid_session_id"));
            }
            self.session_tag = storyos_core::hex_sha256(bytes)[..8].to_owned();
            self.session_id = value.to_str().ok().map(str::to_owned);
        }
        Ok(response)
    }

    /// Decodes a JSON body or a stream of server-sent events to the response of `id`.
    async fn reply(&mut self, mut response: reqwest::Response, id: u64) -> Result<Value, Failure> {
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let events = if content_type.starts_with("text/event-stream") {
            true
        } else if content_type.starts_with("application/json") {
            false
        } else {
            return Err(Failure::Invalid("content_type"));
        };
        let mut buffer = Vec::new();
        let mut stream = EventStream::default();
        loop {
            let chunk = match tokio::time::timeout_at(self.deadline, response.chunk()).await {
                Ok(Ok(chunk)) => chunk,
                Ok(Err(_)) | Err(_) => return Err(Failure::Lost),
            };
            let ended = chunk.is_none();
            if let Some(chunk) = chunk {
                self.received += chunk.len() as u64;
                if self.received > MAX_BODY_BYTES as u64 {
                    return Err(Failure::Invalid("body_too_large"));
                }
                buffer.extend_from_slice(&chunk);
            }
            if events {
                while let Some(message) = stream.next_message(&mut buffer)? {
                    if let Some(reply) = self.answer(message, id) {
                        return reply;
                    }
                }
            }
            if ended {
                break;
            }
        }
        if events {
            // The stream ended before the response.
            return Err(Failure::Lost);
        }
        let message = message(&buffer)?;
        self.answer(message, id)
            .unwrap_or(Err(Failure::Invalid("no_response")))
    }

    /// The result or the error of the response to `id`. A server request or notification is
    /// refused and recorded, and another message is ignored.
    fn answer(&self, mut message: Value, id: u64) -> Option<Result<Value, Failure>> {
        if let Some(method) = message.get("method") {
            tracing::warn!(
                server_url = DiagnosticId(self.url).diagnostic(),
                session_tag = DiagnosticId(&self.session_tag).diagnostic(),
                refused_method = refused_method(method.as_str()).diagnostic(),
                "mcp server message refused"
            );
            return None;
        }
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            return None;
        }
        let version = message.get("jsonrpc").and_then(Value::as_str) == Some("2.0");
        let error = message.get("error").is_some();
        let code = message.pointer("/error/code").and_then(Value::as_i64);
        // A response has exactly one of `result` and `error`.
        Some(match (version, message.get_mut("result"), error, code) {
            (true, Some(result), false, _) => Ok(result.take()),
            (true, None, true, Some(code)) => Err(Failure::JsonRpc(code)),
            _ => Err(Failure::Invalid("malformed_message")),
        })
    }

    fn trace(&self, method: &'static str, request_bytes: u64) {
        tracing::debug!(
            server_url = DiagnosticId(self.url).diagnostic(),
            method = method.diagnostic(),
            session_tag = DiagnosticId(&self.session_tag).diagnostic(),
            request_bytes = request_bytes.diagnostic(),
            response_bytes = self.received.diagnostic(),
            "mcp message"
        );
    }
}

/// The parse state of one server-sent event stream. A line ends with CRLF, LF, or CR. A blank
/// line ends an event. A leading UTF-8 byte order mark is removed.
#[derive(Default)]
struct EventStream {
    started: bool,
    /// The last line ended with a CR, so an LF at the start of the buffer belongs to it.
    after_cr: bool,
    data: Vec<String>,
}

impl EventStream {
    /// Takes the complete lines of `buffer`, and returns the JSON-RPC message of the next event
    /// that has data.
    fn next_message(&mut self, buffer: &mut Vec<u8>) -> Result<Option<Value>, Failure> {
        if !self.started {
            if buffer.len() < 3 && b"\xEF\xBB\xBF".starts_with(buffer) {
                return Ok(None);
            }
            if buffer.starts_with(b"\xEF\xBB\xBF") {
                buffer.drain(..3);
            }
            self.started = true;
        }
        while let Some(line) = self.next_line(buffer) {
            let line = std::str::from_utf8(&line).map_err(|_| Failure::Invalid("invalid_utf8"))?;
            if line.is_empty() {
                let data = std::mem::take(&mut self.data);
                if data.iter().any(|line| !line.is_empty()) {
                    return message(data.join("\n").as_bytes()).map(Some);
                }
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            if field == "data" {
                self.data
                    .push(value.strip_prefix(' ').unwrap_or(value).to_owned());
            }
        }
        Ok(None)
    }

    /// Removes and returns the next complete line of `buffer`, without its end.
    fn next_line(&mut self, buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
        if self.after_cr && !buffer.is_empty() {
            self.after_cr = false;
            if buffer[0] == b'\n' {
                buffer.drain(..1);
            }
        }
        let end = buffer
            .iter()
            .position(|byte| matches!(byte, b'\n' | b'\r'))?;
        let separator = match (buffer[end], buffer.get(end + 1)) {
            (b'\r', Some(b'\n')) => 2,
            (b'\r', None) => {
                self.after_cr = true;
                1
            }
            _ => 1,
        };
        let line = buffer[..end].to_vec();
        buffer.drain(..end + separator);
        Some(line)
    }
}

fn message(bytes: &[u8]) -> Result<Value, Failure> {
    let text = std::str::from_utf8(bytes).map_err(|_| Failure::Invalid("invalid_utf8"))?;
    serde_json::from_str(text).map_err(|_| Failure::Invalid("malformed_message"))
}

/// A static category of a server method, so that no server text reaches a diagnostic line.
fn refused_method(method: Option<&str>) -> &'static str {
    match method {
        Some("ping") => "ping",
        Some("notifications/message") => "notifications/message",
        Some("notifications/progress") => "notifications/progress",
        Some("notifications/tools/list_changed") => "notifications/tools/list_changed",
        Some("roots/list") => "roots/list",
        Some("sampling/createMessage") => "sampling/createMessage",
        Some("elicitation/create") => "elicitation/create",
        Some(_) | None => "other",
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

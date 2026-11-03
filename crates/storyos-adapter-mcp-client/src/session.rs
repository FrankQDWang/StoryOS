//! One MCP session over the Streamable HTTP transport: each JSON-RPC message is one POST.

use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
use serde_json::{Value, json};
use storyos_application::{DiagnosticField as _, DiagnosticId, ResolvedCredential};

use crate::record::{self, MAX_TEXT_BYTES, ToolDiscoveryRecord};
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
}

/// `initialize`, `notifications/initialized`, and then the operation of `request`.
pub(crate) async fn run(
    url: &str,
    credential: &ResolvedCredential,
    request: &McpRequest,
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
    };
    let identity = match session.request("initialize", initialize_params()).await {
        Ok(result) => record::server_identity(&result),
        Err(failure) => return failure.before_operation(),
    };
    let (server_name, server_version) = match identity {
        Ok(identity) => identity,
        Err(reason) => return McpObservation::InvalidResult { reason },
    };
    if let Err(failure) = session.notify("notifications/initialized").await {
        return failure.before_operation();
    }
    match request {
        McpRequest::Discover { .. } => {
            let mut tools = Vec::new();
            let mut cursor = None;
            loop {
                let params = match &cursor {
                    Some(cursor) => json!({"cursor": cursor}),
                    None => json!({}),
                };
                let page = match session.request("tools/list", params).await {
                    Ok(page) => page,
                    Err(failure) => return failure.after_operation(),
                };
                match record::tools_page(&page, &mut tools) {
                    Ok(Some(next)) => cursor = Some(next),
                    Ok(None) => break,
                    Err(reason) => return McpObservation::InvalidResult { reason },
                }
            }
            McpObservation::Discovered(ToolDiscoveryRecord {
                server_url: url.to_owned(),
                server_name,
                server_version,
                protocol_revision: PROTOCOL_REVISION.to_owned(),
                tools,
            })
        }
        McpRequest::Call {
            tool, arguments, ..
        } => {
            let params = json!({"name": tool, "arguments": arguments});
            match session.request("tools/call", params).await {
                Ok(result) => match record::call_result(result) {
                    Ok(result) => McpObservation::Called(result),
                    Err(reason) => McpObservation::InvalidResult { reason },
                },
                Err(failure) => failure.after_operation(),
            }
        }
    }
}

impl Session<'_> {
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
        let response = match request.body(body).send().await {
            Ok(response) => response,
            Err(error) if error.is_builder() || error.is_connect() => return Err(Failure::Unsent),
            Err(_) => return Err(Failure::Lost),
        };
        let status = response.status();
        if status.is_server_error() {
            return Err(Failure::Lost);
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
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(_) => return Err(Failure::Lost),
            };
            self.received += chunk.len() as u64;
            buffer.extend_from_slice(&chunk);
            if buffer.len() > MAX_BODY_BYTES {
                return Err(Failure::Invalid("body_too_large"));
            }
            while events && let Some((end, separator)) = event_end(&buffer) {
                let event: Vec<u8> = buffer.drain(..end + separator).collect();
                if let Some(message) = event_message(&event[..end])?
                    && let Some(reply) = self.answer(message, id)
                {
                    return reply;
                }
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
        if let Some(result) = message.get_mut("result") {
            return Some(Ok(result.take()));
        }
        Some(
            match message.pointer("/error/code").and_then(Value::as_i64) {
                Some(code) => Err(Failure::JsonRpc(code)),
                None => Err(Failure::Invalid("malformed_message")),
            },
        )
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

/// The end of the first complete event and the length of its blank line.
fn event_end(buffer: &[u8]) -> Option<(usize, usize)> {
    (0..buffer.len()).find_map(|position| {
        let tail = &buffer[position..];
        if tail.starts_with(b"\n\n") {
            Some((position, 2))
        } else if tail.starts_with(b"\r\n\r\n") {
            Some((position, 4))
        } else {
            None
        }
    })
}

/// The JSON-RPC message of one event, or `None` for an event with no data.
fn event_message(event: &[u8]) -> Result<Option<Value>, Failure> {
    let text = std::str::from_utf8(event).map_err(|_| Failure::Invalid("invalid_utf8"))?;
    let data: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect();
    if data.iter().all(|line| line.is_empty()) {
        return Ok(None);
    }
    message(data.join("\n").as_bytes()).map(Some)
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

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use storyos_application::{
    CredentialReference, CredentialResolver, PreDispatchRefusal, ResolvedCredential,
};
use storyos_core::{ContractCheck, ContractDifference, EXA_TOOL_REGISTRATION, ToolRegistration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{
    CallResult, DiscoveredTool, McpClient, McpObservation, McpRequest, PROTOCOL_REVISION,
    ToolDiscoveryRecord,
};

const REFERENCE: &str = "settings:STORYOS_EXA_API_KEY";
const KEY_CANARY: &str = "storyos-exa-key-canary-7c41";
const CONTENT_CANARY: &str = "storyos-content-canary-93ab";
const SESSION: &str = "session-canary-0f3a";
/// The canonical JSON of the two input schemas, written independently of the client.
const SEARCH_CANONICAL: &str =
    r#"{"properties":{"query":{"type":"string"}},"required":["query"],"type":"object"}"#;
const FETCH_CANONICAL: &str =
    r#"{"properties":{"urls":{"items":{"type":"string"},"type":"array"}},"type":"object"}"#;

/// Returns a test value that is not a real credential, for one reference only.
struct CanaryResolver;

impl CredentialResolver for CanaryResolver {
    async fn resolve(&self, reference: &CredentialReference) -> Option<ResolvedCredential> {
        (reference.0 == REFERENCE).then(|| ResolvedCredential::new(KEY_CANARY.to_owned()))
    }
}

/// How the fake server sends each response.
#[derive(Clone, Copy)]
enum Body {
    Json,
    /// A priming event, a server request, and a server notification, and then the response.
    Events,
    /// The same events after a byte order mark, with a CR at the end of each line.
    CrEvents,
    /// The CR events on a connection that stays open after the response.
    HeldCrEvents,
}

/// The answer of the fake server to one message.
enum Reply {
    Result(Value),
    Error(i64),
    Accepted,
    /// A promised body that stops after a few bytes.
    Drop,
    /// A complete JSON body as it is.
    Message(Value),
    /// No response, on an open connection.
    Hold,
    /// More than 1 MiB of small server notifications before the response.
    Flood,
    /// HTTP 404 with no body.
    NotFound,
}

/// The method and the MCP headers of one message that the fake server received.
#[derive(Debug, PartialEq)]
struct Received {
    method: String,
    key: Option<String>,
    accept: Option<String>,
    version: Option<String>,
    session: Option<String>,
}

/// An in-process MCP server on a loopback port. It serves each message on its own connection.
async fn serve(
    body: Body,
    session: Option<&'static str>,
    script: impl Fn(&Value) -> Reply + Send + 'static,
) -> (String, Arc<Mutex<Vec<Received>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&received);
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (headers, message) = read_message(&mut stream).await;
            let header = |name: &str| {
                headers.lines().find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case(name)
                        .then(|| value.trim().to_owned())
                })
            };
            let method = message["method"].as_str().unwrap().to_owned();
            log.lock().unwrap().push(Received {
                key: header("x-api-key"),
                accept: header("accept"),
                version: header("mcp-protocol-version"),
                session: header("mcp-session-id"),
                method: method.clone(),
            });
            let session = session.filter(|_| method == "initialize");
            let response = match script(&message) {
                Reply::Result(result) => response(body, session, json!({"jsonrpc": "2.0", "id": message["id"], "result": result})),
                Reply::Error(code) => response(body, session, json!({"jsonrpc": "2.0", "id": message["id"], "error": {"code": code, "message": CONTENT_CANARY}})),
                Reply::Accepted => "HTTP/1.1 202 Accepted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_owned(),
                Reply::Drop => "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 400\r\nconnection: close\r\n\r\n{\"jsonrpc\"".to_owned(),
                Reply::Message(message) => response(Body::Json, session, message),
                Reply::Hold => std::future::pending().await,
                Reply::Flood => {
                    let event = format!("data: {}\n\n", json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {"padding": "a".repeat(1000)}}));
                    let result = json!({"jsonrpc": "2.0", "id": message["id"], "result": {"content": []}});
                    format!("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{}data: {result}\n\n", event.repeat(1100))
                }
                Reply::NotFound => "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_owned(),
            };
            stream.write_all(response.as_bytes()).await.unwrap();
            if matches!(body, Body::HeldCrEvents) {
                tokio::spawn(async move {
                    let _open = stream;
                    std::future::pending::<()>().await;
                });
            }
        }
    });
    (url, received)
}

async fn read_message(stream: &mut tokio::net::TcpStream) -> (String, Value) {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await.unwrap();
        assert_ne!(read, 0, "the request ended before its body");
        bytes.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&bytes);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let length = head
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|value| value.trim().parse::<usize>().unwrap())
            })
            .unwrap();
        if body.len() >= length {
            return (
                head.to_owned(),
                serde_json::from_str(&body[..length]).unwrap(),
            );
        }
    }
}

fn response(body: Body, session: Option<&str>, message: Value) -> String {
    let session = session
        .map(|id| format!("mcp-session-id: {id}\r\n"))
        .unwrap_or_default();
    match body {
        Body::Json => {
            let message = message.to_string();
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{session}content-length: {}\r\nconnection: close\r\n\r\n{message}",
                message.len()
            )
        }
        Body::Events | Body::CrEvents | Body::HeldCrEvents => {
            let request = json!({"jsonrpc": "2.0", "id": "s1", "method": "ping"});
            let notification = json!({"jsonrpc": "2.0", "method": "notifications/message", "params": {"level": "info", "data": CONTENT_CANARY}});
            let events = format!(
                "id: 1\ndata:\n\ndata: {request}\n\ndata: {notification}\n\nid: 2\nevent: message\ndata: {message}\n\n"
            );
            let events = match body {
                Body::CrEvents | Body::HeldCrEvents => {
                    format!("\u{feff}{}", events.replace('\n', "\r"))
                }
                Body::Json | Body::Events => events,
            };
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n{session}connection: close\r\n\r\n{events}"
            )
        }
    }
}

fn conforming(message: &Value) -> Reply {
    match message["method"].as_str().unwrap() {
        "initialize" => Reply::Result(json!({
            "protocolVersion": PROTOCOL_REVISION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fake-exa", "version": "1.2.3"},
        })),
        "notifications/initialized" => Reply::Accepted,
        "tools/list" if message["params"]["cursor"] == "page-2" => {
            Reply::Result(json!({"tools": [{
                "name": "web_fetch_exa",
                "inputSchema": {"type": "object", "properties": {"urls": {"type": "array", "items": {"type": "string"}}}},
                "outputSchema": {"type": "object"},
            }]}))
        }
        "tools/list" => Reply::Result(json!({
            "tools": [{
                "name": "web_search_exa",
                "description": "Search the web.",
                "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]},
                "annotations": {"readOnlyHint": true},
            }],
            "nextCursor": "page-2",
        })),
        "tools/call" => Reply::Result(json!({
            "content": [{"type": "text", "text": CONTENT_CANARY}],
            "isError": false,
            "structuredContent": {"results": 1},
        })),
        method => panic!("unexpected method {method}"),
    }
}

fn record(url: &str) -> ToolDiscoveryRecord {
    let digest =
        |canonical: &str| format!("sha256:{}", storyos_core::hex_sha256(canonical.as_bytes()));
    ToolDiscoveryRecord {
        server_url: url.to_owned(),
        server_name: "fake-exa".to_owned(),
        server_version: "1.2.3".to_owned(),
        protocol_revision: PROTOCOL_REVISION.to_owned(),
        tools: vec![
            DiscoveredTool {
                name: "web_search_exa".to_owned(),
                description: Some("Search the web.".to_owned()),
                input_schema: serde_json::from_str(SEARCH_CANONICAL).unwrap(),
                input_schema_digest: digest(SEARCH_CANONICAL),
                output_schema: None,
                annotations: Some(json!({"readOnlyHint": true})),
            },
            DiscoveredTool {
                name: "web_fetch_exa".to_owned(),
                description: None,
                input_schema: serde_json::from_str(FETCH_CANONICAL).unwrap(),
                input_schema_digest: digest(FETCH_CANONICAL),
                output_schema: Some(json!({"type": "object"})),
                annotations: None,
            },
        ],
    }
}

fn called() -> McpObservation {
    McpObservation::Called(CallResult {
        text: vec![CONTENT_CANARY.to_owned()],
        is_error: false,
        structured_content: Some(json!({"results": 1})),
    })
}

fn discover(url: &str) -> McpRequest {
    McpRequest::Discover {
        server_url: url.to_owned(),
    }
}

fn call(url: &str) -> McpRequest {
    McpRequest::Call {
        server_url: url.to_owned(),
        tool: "web_search_exa".to_owned(),
        arguments: json!({"query": CONTENT_CANARY})
            .as_object()
            .unwrap()
            .clone(),
    }
}

async fn exchange(request: McpRequest) -> McpObservation {
    let client = McpClient {
        resolver: CanaryResolver,
        timeout: Duration::from_secs(/*secs*/ 1),
    };
    let reference = CredentialReference(REFERENCE.to_owned());
    let prepared = client
        .prepare(&request, Some(&reference))
        .await
        .ok()
        .unwrap();
    client.exchange(prepared.prepared).await
}

/// The messages of one exchange with `methods`, each with the key and the transport headers.
fn sent(methods: &[&str], session: Option<&str>) -> Vec<Received> {
    methods
        .iter()
        .map(|method| {
            let first = *method == "initialize";
            Received {
                method: (*method).to_owned(),
                key: Some(KEY_CANARY.to_owned()),
                accept: Some("application/json, text/event-stream".to_owned()),
                version: (!first).then(|| PROTOCOL_REVISION.to_owned()),
                session: session.filter(|_| !first).map(str::to_owned),
            }
        })
        .collect()
}

const DISCOVER: &[&str] = &[
    "initialize",
    "notifications/initialized",
    "tools/list",
    "tools/list",
];
const CALL: &[&str] = &["initialize", "notifications/initialized", "tools/call"];

#[tokio::test]
async fn json_and_event_stream_servers_give_the_same_record_and_call_result() {
    for (body, session) in [
        (Body::Json, Some(SESSION)),
        (Body::Events, Some(SESSION)),
        (Body::CrEvents, Some(SESSION)),
        (Body::HeldCrEvents, Some(SESSION)),
        (Body::Json, None),
    ] {
        let (url, received) = serve(body, session, conforming).await;
        let observations = (exchange(discover(&url)).await, exchange(call(&url)).await);
        let messages = std::mem::take(&mut *received.lock().unwrap());
        let mut expected = sent(DISCOVER, session);
        expected.extend(sent(CALL, session));
        assert_eq!(
            (observations, messages),
            (
                (McpObservation::Discovered(record(&url)), called()),
                expected
            )
        );
    }
}

/// The method that fails, the reply of the server to it, and the expected observation.
type FailureCase = (&'static str, fn(&Value) -> Reply, McpObservation);

#[tokio::test]
async fn each_failure_of_the_server_is_one_observation() {
    let malformed = McpObservation::InvalidResult {
        reason: "malformed_message",
    };
    let cases: [FailureCase; 8] = [
        (
            "tools/call",
            |_| Reply::Error(-32602),
            McpObservation::JsonRpcError { code: -32602 },
        ),
        (
            "tools/call",
            |_| Reply::Drop,
            McpObservation::OutcomeUnknown,
        ),
        (
            "tools/call",
            |_| Reply::Hold,
            McpObservation::OutcomeUnknown,
        ),
        (
            "tools/call",
            |message| {
                Reply::Message(
                    json!({"jsonrpc": "1.0", "id": message["id"], "result": {"content": []}}),
                )
            },
            malformed.clone(),
        ),
        (
            "tools/call",
            |message| {
                Reply::Message(
                    json!({"jsonrpc": "2.0", "id": message["id"], "result": {"content": []}, "error": {"code": 1, "message": ""}}),
                )
            },
            malformed,
        ),
        (
            "tools/call",
            |_| Reply::Flood,
            McpObservation::InvalidResult {
                reason: "body_too_large",
            },
        ),
        // The operation request never left, so the call is proven not submitted.
        ("initialize", |_| Reply::Drop, McpObservation::NotSubmitted),
        ("initialize", |_| Reply::Hold, McpObservation::NotSubmitted),
    ];
    for (method, reply, expected) in cases {
        let script = move |message: &Value| {
            if message["method"] == method {
                reply(message)
            } else {
                conforming(message)
            }
        };
        let (url, _) = serve(Body::Json, Some(SESSION), script).await;
        assert_eq!(exchange(call(&url)).await, expected, "{method}");
    }
}

/// A 404 ends the exchange. The next exchange is a new Destination Attempt with a new session.
#[tokio::test]
async fn an_ended_session_is_a_rejection_and_nothing_is_sent_again() {
    for (session, reason) in [(Some(SESSION), "session_ended"), (None, "http_404")] {
        let script = |message: &Value| {
            if message["method"] == "tools/call" {
                Reply::NotFound
            } else {
                conforming(message)
            }
        };
        let (url, received) = serve(Body::Json, session, script).await;
        let observation = exchange(call(&url)).await;
        let messages = std::mem::take(&mut *received.lock().unwrap());
        let expected = McpObservation::Rejected {
            reason: reason.to_owned(),
        };
        assert_eq!((observation, messages), (expected, sent(CALL, session)));
    }
}

#[tokio::test]
async fn prepare_refuses_a_missing_or_unresolved_credential_and_projects_no_key() {
    let client = McpClient {
        resolver: CanaryResolver,
        timeout: Duration::from_secs(/*secs*/ 5),
    };
    let url = "https://mcp.example.test/mcp";
    let known = CredentialReference(REFERENCE.to_owned());
    let unknown = CredentialReference("settings:STORYOS_OTHER_KEY".to_owned());
    let mut outcomes = Vec::new();
    for (url, reference) in [
        (url, None),
        (url, Some(&unknown)),
        ("https://mcp.example.test/mcp?exaApiKey=x", Some(&known)),
        (url, Some(&known)),
    ] {
        let prepared = client.prepare(&discover(url), reference).await;
        outcomes.push(
            prepared.map(|prepared| format!("{:?}", prepared.projection).contains(KEY_CANARY)),
        );
    }
    assert_eq!(
        outcomes,
        [
            Err(PreDispatchRefusal::CredentialUnavailable),
            Err(PreDispatchRefusal::CredentialUnavailable),
            Err(PreDispatchRefusal::UnsupportedRequest),
            Ok(false),
        ]
    );
}

#[test]
fn a_call_result_needs_text_blocks_within_the_bound() {
    let text = |size: usize| json!({"content": [{"type": "text", "text": "a".repeat(size)}]});
    let cases = [
        (
            json!({"content": [{"type": "image", "data": "", "mimeType": "image/png"}]}),
            Err("non_text_block"),
        ),
        (json!({"content": [], "isError": "yes"}), Err("is_error")),
        (text(256 * 1024 + 1), Err("text_too_large")),
        (
            json!({"content": [{"type": "text", "text": "x"}], "isError": true}),
            Ok(CallResult {
                text: vec!["x".to_owned()],
                is_error: true,
                structured_content: None,
            }),
        ),
    ];
    for (result, expected) in cases {
        assert_eq!(crate::record::call_result(result), expected);
    }
}

/// Collects the JSON lines of the Diagnostic Projection in memory.
#[derive(Clone, Default)]
struct Projection(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Projection {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn diagnostics_carry_no_key_content_or_session_identifier() {
    let projection = Projection::default();
    let writer = projection.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(move || writer.clone())
        .finish();
    // A thread-local subscriber misses events when a parallel test thread registers a callsite
    // first. The process subscriber also takes the lines of the other tests, which hold the same
    // canaries.
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let (url, _) = serve(Body::Events, Some(SESSION), conforming).await;

    let observation = exchange(call(&url)).await;

    let lines = String::from_utf8(projection.0.lock().unwrap().clone()).unwrap();
    let tag = &storyos_core::hex_sha256(SESSION.as_bytes())[..8];
    assert_eq!(
        (
            observation,
            [KEY_CANARY, CONTENT_CANARY, SESSION].map(|canary| lines.contains(canary)),
            lines.contains(&format!(r#""session_tag":"{tag}""#)),
            lines.contains(r#""method":"tools/call""#),
            lines.contains(r#""refused_method":"ping""#),
        ),
        (called(), [false; 3], true, true, true)
    );
}

fn newer_revision(message: &Value) -> Reply {
    match message["method"].as_str().unwrap() {
        "initialize" => Reply::Result(json!({
            "protocolVersion": "2026-06-18",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "exa-search-server", "version": "3.5.0"},
        })),
        method => panic!("unexpected method {method}"),
    }
}

#[tokio::test]
async fn another_protocol_revision_gives_a_record_whose_drift_check_names_it() {
    let (url, received) = serve(Body::Json, None, newer_revision).await;
    let observations = (exchange(discover(&url)).await, exchange(call(&url)).await);
    let messages = std::mem::take(&mut *received.lock().unwrap());
    let record = ToolDiscoveryRecord {
        server_url: url.clone(),
        server_name: "exa-search-server".to_owned(),
        server_version: "3.5.0".to_owned(),
        protocol_revision: "2026-06-18".to_owned(),
        tools: Vec::new(),
    };
    let registration = ToolRegistration {
        server_url: Box::leak(url.into_boxed_str()),
        ..EXA_TOOL_REGISTRATION
    };

    assert_eq!(
        (observations, messages),
        (
            (
                McpObservation::Discovered(record.clone()),
                McpObservation::InvalidResult {
                    reason: "protocol_revision"
                }
            ),
            sent(&["initialize", "initialize"], None)
        )
    );
    assert_eq!(
        record.check(&registration),
        ContractCheck::Drift(vec![ContractDifference::ProtocolRevision {
            pinned: PROTOCOL_REVISION.to_owned(),
            observed: "2026-06-18".to_owned(),
        }])
    );
}

#[tokio::test]
async fn a_dispatch_sends_no_call_after_a_drifted_or_failed_tool_list() {
    let dispatch = |url: &str| McpRequest::Dispatch {
        server_url: url.to_owned(),
        registration: &EXA_TOOL_REGISTRATION,
        tool: "web_search_exa".to_owned(),
        arguments: json!({"query": CONTENT_CANARY})
            .as_object()
            .unwrap()
            .clone(),
    };
    let (drifted_url, drifted) = serve(Body::Json, Some(SESSION), conforming).await;
    let (failed_url, failed) = serve(Body::Json, Some(SESSION), |message| match message["method"]
        .as_str()
        .unwrap()
    {
        "tools/list" => Reply::Error(-32603),
        _ => conforming(message),
    })
    .await;
    let drift = exchange(dispatch(&drifted_url)).await;
    let not_submitted = exchange(dispatch(&failed_url)).await;

    assert_eq!(
        (
            matches!(&drift, McpObservation::Drift(differences)
            if differences.contains(&ContractDifference::ServerName {
                pinned: "exa-search-server".to_owned(),
                observed: "fake-exa".to_owned(),
            })),
            std::mem::take(&mut *drifted.lock().unwrap()),
            not_submitted,
            std::mem::take(&mut *failed.lock().unwrap()),
        ),
        (
            true,
            sent(DISCOVER, Some(SESSION)),
            McpObservation::NotSubmitted,
            sent(&DISCOVER[..3], Some(SESSION)),
        )
    );
}

use serde_json::json;
use storyos_application::{
    CreatePurpose, CreateRequest, DestinationRequest, ModelProviderAdapter, RequestAttempt,
    RequestRoute, ToolEvidence, ToolItemEvidence, ToolProbeRequest,
};

use crate::AgentPlanResponses;
use crate::tests::{CanaryResolver, create, route};

fn tool_item(kind: &str, item_id: &str) -> ToolItemEvidence {
    ToolItemEvidence {
        kind: kind.to_owned(),
        item_id: Some(item_id.to_owned()),
        status: Some("completed".to_owned()),
        call_id: None,
        name: None,
        server_label: None,
        arguments: None,
        listed_tools: Vec::new(),
        output_present: false,
        error: None,
    }
}

#[tokio::test]
async fn a_probe_sends_its_tool_and_constant_input_and_no_project_content() {
    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let route = RequestRoute {
        mapping_revision: storyos_core::AGENT_PLAN_COMPACTION_MAPPING_REVISION.to_owned(),
        ..route("http://127.0.0.1:9")
    };
    let DestinationRequest::Create(request) = create(route, RequestAttempt::New) else {
        unreachable!("create() makes a Create request")
    };
    let mut sent = Vec::new();
    for probe in [
        ToolProbeRequest::FunctionCall,
        ToolProbeRequest::FunctionCallOutput {
            call_id: "call_01".to_owned(),
        },
        ToolProbeRequest::HostedWebSearch,
        ToolProbeRequest::RemoteMcp,
        ToolProbeRequest::MaxToolCallsBound,
    ] {
        let previous_response_reference = match probe {
            ToolProbeRequest::FunctionCallOutput { .. } => Some("resp_01".to_owned()),
            _ => None,
        };
        let prepared = adapter
            .prepare(&DestinationRequest::Create(CreateRequest {
                purpose: CreatePurpose::ToolQualification(probe),
                author_message: "StoryOS Tool probe".to_owned(),
                previous_response_reference,
                ..request.clone()
            }))
            .await
            .ok()
            .unwrap();
        let body = prepared.projection.serialized_payload.unwrap();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        sent.push((
            value["tools"][0]["type"].clone(),
            value["input"][0].clone(),
            value["stream"].clone(),
            value.get("max_tool_calls").cloned(),
            value.get("previous_response_id").cloned(),
            body.contains("The rain fell hard"),
        ));
    }
    let author = json!({"role": "user", "content": [
        {"type": "input_text", "text": "StoryOS Tool probe"}
    ]});

    assert_eq!(
        sent,
        [
            (
                json!("function"),
                author.clone(),
                json!(false),
                None,
                None,
                false
            ),
            (
                json!("function"),
                json!({"type": "function_call_output", "call_id": "call_01",
                       "output": "{\"text\":\"storyos-probe\"}"}),
                json!(false),
                None,
                Some(json!("resp_01")),
                false,
            ),
            (
                json!("web_search"),
                author.clone(),
                json!(false),
                None,
                None,
                false
            ),
            (
                json!("mcp"),
                author.clone(),
                json!(false),
                None,
                None,
                false
            ),
            (
                json!("web_search"),
                author,
                json!(false),
                Some(json!(1)),
                None,
                false
            ),
        ]
    );
}

#[test]
fn the_tool_facts_keep_each_call_correlation_citation_and_usage_count() {
    let response = json!({
        "status": "completed",
        "output": [
            {"id": "rs_01", "type": "reasoning", "summary": []},
            {"id": "fc_01", "type": "function_call", "status": "completed", "call_id": "call_01",
             "name": "storyos_probe_echo", "arguments": "{\"text\":\"storyos-probe\"}"},
            {"id": "ws_01", "type": "web_search_call", "status": "completed",
             "action": {"type": "search", "query": "Rust 1.0"}},
            {"id": "mcpl_01", "type": "mcp_list_tools", "status": "completed",
             "server_label": "deepwiki", "tools": [{"name": "read_wiki_structure"}]},
            {"id": "mcp_01", "type": "mcp_call", "status": "failed", "server_label": "deepwiki",
             "name": "read_wiki_structure", "arguments": "{}", "output": null,
             "error": {"code": "timeout"}},
            {"id": "msg_01", "type": "message", "status": "completed", "content": [
                {"type": "output_text", "text": "2015-05-15", "annotations": [
                    {"type": "url_citation", "url": "https://blog.rust-lang.org/", "title": "Rust"}
                ]}
            ]}
        ],
        "usage": {"input_tokens": 10, "output_tokens": 5, "tool_usage": {"web_search": 1}}
    });
    let ordinary = json!({
        "status": "completed",
        "output": [crate::tests::message("text")],
        "usage": {"tool_usage": {"web_search": 0}}
    });

    assert_eq!(
        (
            super::tool_evidence(&response),
            super::tool_evidence(&ordinary)
        ),
        (
            Some(ToolEvidence {
                items: vec![
                    ToolItemEvidence {
                        call_id: Some("call_01".to_owned()),
                        name: Some("storyos_probe_echo".to_owned()),
                        arguments: Some("{\"text\":\"storyos-probe\"}".to_owned()),
                        ..tool_item("function_call", "fc_01")
                    },
                    ToolItemEvidence {
                        arguments: Some(r#"{"query":"Rust 1.0","type":"search"}"#.to_owned()),
                        ..tool_item("web_search_call", "ws_01")
                    },
                    ToolItemEvidence {
                        server_label: Some("deepwiki".to_owned()),
                        listed_tools: vec!["read_wiki_structure".to_owned()],
                        ..tool_item("mcp_list_tools", "mcpl_01")
                    },
                    ToolItemEvidence {
                        status: Some("failed".to_owned()),
                        name: Some("read_wiki_structure".to_owned()),
                        server_label: Some("deepwiki".to_owned()),
                        arguments: Some("{}".to_owned()),
                        error: Some(r#"{"code":"timeout"}"#.to_owned()),
                        ..tool_item("mcp_call", "mcp_01")
                    },
                ],
                citations: vec!["https://blog.rust-lang.org/".to_owned()],
                tool_usage: vec![("web_search".to_owned(), 1)],
            }),
            None,
        )
    );
}

#[tokio::test]
async fn only_the_remote_mcp_probe_sends_the_mcp_beta_header() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let adapter = AgentPlanResponses {
        resolver: CanaryResolver::default(),
    };
    let mut heads = Vec::new();
    for probe in [
        ToolProbeRequest::RemoteMcp,
        ToolProbeRequest::HostedWebSearch,
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/api/plan/v3", listener.local_addr().unwrap());
        let DestinationRequest::Create(request) = create(route(&base), RequestAttempt::New) else {
            unreachable!("create() makes a Create request")
        };
        let served = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0_u8; 8192];
            let read = stream.read(&mut bytes).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 400 Scripted\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            String::from_utf8_lossy(&bytes[..read]).to_ascii_lowercase()
        });
        let prepared = adapter
            .prepare(&DestinationRequest::Create(CreateRequest {
                purpose: CreatePurpose::ToolQualification(probe),
                ..request
            }))
            .await
            .ok()
            .unwrap();
        adapter
            .exchange(
                prepared.prepared,
                &mut crate::tests::RecordingSink::default(),
            )
            .await;
        heads.push(served.await.unwrap().contains("\r\nark-beta-mcp: true\r\n"));
    }

    assert_eq!(heads, [true, false]);
}

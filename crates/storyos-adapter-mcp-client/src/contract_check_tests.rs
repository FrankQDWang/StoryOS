use serde_json::{Value, json};
use storyos_core::{ContractCheck, ContractDifference, EXA_TOOL_REGISTRATION};

use crate::{DiscoveredTool, ToolDiscoveryRecord};
use crate::record::{digest, tools_page};

type Edit = fn(&mut Value);

const REPORT: &str = include_str!("../../../docs/research/exa-mcp-discovery-2026-11-03.md");

/// The appendix record of the S5-01a Discovery Record.
fn recorded_json() -> Value {
    let appendix = &REPORT[REPORT.find("## Appendix").unwrap()..];
    let start = appendix.find("```json\n").unwrap() + "```json\n".len();
    let end = start + appendix[start..].find("\n```").unwrap();
    serde_json::from_str(&appendix[start..end]).unwrap()
}

/// Parses an appendix record as the client parses a `tools/list` page.
fn record_of(recorded: &Value) -> ToolDiscoveryRecord {
    let listed: Vec<Value> = recorded["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            let mut listed = json!({
                "name": tool["name"],
                "description": tool["description"],
                "inputSchema": tool["input_schema"],
                "annotations": tool["annotations"],
            });
            if !tool["output_schema"].is_null() {
                listed["outputSchema"] = tool["output_schema"].clone();
            }
            listed
        })
        .collect();
    let mut tools = Vec::new();
    tools_page(&json!({ "tools": listed }), &mut tools).unwrap();
    let text = |field: &str| recorded[field].as_str().unwrap().to_owned();
    ToolDiscoveryRecord {
        server_url: text("server_url"),
        server_name: text("server_name"),
        server_version: text("server_version"),
        protocol_revision: text("protocol_revision"),
        tools,
    }
}

fn edited(edit: Edit) -> ToolDiscoveryRecord {
    let mut recorded = recorded_json();
    edit(&mut recorded);
    record_of(&recorded)
}

fn tool<'a>(recorded: &'a mut Value, name: &str) -> &'a mut Value {
    recorded["tools"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|tool| tool["name"] == name)
        .unwrap()
}

#[test]
fn the_pinned_digests_match_the_discovery_record_and_an_extra_unapproved_tool_is_not_drift() {
    let recorded = recorded_json();
    let record = record_of(&recorded);
    let names: Vec<&str> = record.tools().iter().map(DiscoveredTool::name).collect();
    let recorded_digests: Vec<&Value> = recorded["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| &tool["input_schema_digest"])
        .collect();
    let computed_digests: Vec<Value> = record
        .tools()
        .iter()
        .map(|tool| json!(tool.input_schema_digest()))
        .collect();

    assert_eq!(names, ["web_search_exa", "web_fetch_exa", "agent_run"]);
    assert_eq!(recorded_digests, computed_digests.iter().collect::<Vec<_>>());
    assert_eq!(record.check(&EXA_TOOL_REGISTRATION), ContractCheck::Unchanged);
    assert_eq!(EXA_TOOL_REGISTRATION.tool("agent_run"), None);
    assert_eq!(EXA_TOOL_REGISTRATION.tool("web_search_advanced_exa"), None);
}

fn remove_fetch(recorded: &mut Value) {
    recorded["tools"]
        .as_array_mut()
        .unwrap()
        .retain(|tool| tool["name"] != "web_fetch_exa");
}

fn widen_query(recorded: &mut Value) {
    tool(recorded, "web_search_exa")["input_schema"]["properties"]["query"]["maxLength"] =
        json!(9000);
}

fn reword_urls_property(recorded: &mut Value) {
    tool(recorded, "web_fetch_exa")["input_schema"]["properties"]["urls"]["description"] =
        json!("Changed text.");
}

fn add_fetch_output_schema(recorded: &mut Value) {
    tool(recorded, "web_fetch_exa")["output_schema"] = json!({"type": "object"});
}

fn observed_schema_digest(edit: Edit, name: &str) -> String {
    let record = edited(edit);
    let observed = record.tools().iter().find(|tool| tool.name() == name);
    observed.unwrap().input_schema_digest().to_owned()
}

#[test]
fn a_changed_identity_schema_or_removed_approved_tool_is_drift_that_names_the_difference() {
    let pinned = |index: usize| EXA_TOOL_REGISTRATION.tools[index].input_schema_digest.to_owned();
    let cases: [(Edit, ContractDifference); 6] = [
        (
            remove_fetch,
            ContractDifference::ToolRemoved {
                tool: "web_fetch_exa".to_owned(),
            },
        ),
        (
            widen_query,
            ContractDifference::InputSchema {
                tool: "web_search_exa".to_owned(),
                pinned: pinned(0),
                observed: observed_schema_digest(widen_query, "web_search_exa"),
            },
        ),
        (
            reword_urls_property,
            ContractDifference::InputSchema {
                tool: "web_fetch_exa".to_owned(),
                pinned: pinned(1),
                observed: observed_schema_digest(reword_urls_property, "web_fetch_exa"),
            },
        ),
        (
            add_fetch_output_schema,
            ContractDifference::OutputSchema {
                tool: "web_fetch_exa".to_owned(),
                pinned: None,
                observed: Some(digest(&json!({"type": "object"}))),
            },
        ),
        (
            |recorded| recorded["server_name"] = json!("other-server"),
            ContractDifference::ServerName {
                pinned: "exa-search-server".to_owned(),
                observed: "other-server".to_owned(),
            },
        ),
        (
            |recorded| recorded["protocol_revision"] = json!("2026-06-18"),
            ContractDifference::ProtocolRevision {
                pinned: "2025-11-25".to_owned(),
                observed: "2026-06-18".to_owned(),
            },
        ),
    ];
    for (edit, difference) in cases {
        assert_eq!(
            edited(edit).check(&EXA_TOOL_REGISTRATION),
            ContractCheck::Drift(vec![difference])
        );
    }
}

#[test]
fn a_changed_description_annotation_or_server_version_is_provenance_only() {
    let cases: [Edit; 3] = [
        |recorded| tool(recorded, "web_search_exa")["description"] = json!("Changed text."),
        |recorded| {
            tool(recorded, "web_fetch_exa")["annotations"] =
                json!({"destructiveHint": true, "readOnlyHint": false});
        },
        |recorded| recorded["server_version"] = json!("3.6.0"),
    ];
    for edit in cases {
        assert_eq!(
            edited(edit).check(&EXA_TOOL_REGISTRATION),
            ContractCheck::ProvenanceChange
        );
    }
}

use serde_json::json;

use super::{
    BoundTool, ExposedTool, ExposureFacts, ResponseDisposition, ResponseFacts,
    ToolCompatibilityDecision, ToolExposure, requests_function, response_disposition,
};
use crate::{
    AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION, AGENT_PLAN_REGISTRATION, AgentDecisionKind,
    AgentDecisionOutcome, EXA_TOOL_REGISTRATION, HOST_FAKE_REGISTRATION, NativeStreamItem,
    StreamItemRole, StreamItemState, ToolRegistrationLifecycle, WEB_FETCH_TOOL_SPEC,
    WEB_SEARCH_TOOL_SPEC,
};

fn bound(
    lifecycle: ToolRegistrationLifecycle,
    compatibility: Option<ToolCompatibilityDecision>,
) -> BoundTool {
    BoundTool {
        registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
        lifecycle,
        compatibility,
    }
}

fn exposure(bound: &[BoundTool], model_registration_revision: &str) -> ToolExposure {
    ToolExposure::compute(&ExposureFacts {
        bound,
        model_registration_revision,
    })
}

fn research_exposure() -> ToolExposure {
    ToolExposure {
        tools: vec![
            ExposedTool {
                name: "web_search",
                registration_id: "web_research",
                registration_revision: EXA_TOOL_REGISTRATION.revision,
                spec: &WEB_SEARCH_TOOL_SPEC,
            },
            ExposedTool {
                name: "web_fetch",
                registration_id: "web_research",
                registration_revision: EXA_TOOL_REGISTRATION.revision,
                spec: &WEB_FETCH_TOOL_SPEC,
            },
        ],
    }
}

#[test]
fn an_active_compatible_binding_on_a_qualified_registration_exposes_its_tools() {
    let active = [bound(
        ToolRegistrationLifecycle::Active,
        Some(ToolCompatibilityDecision::Compatible),
    )];
    assert_eq!(
        exposure(&active, AGENT_PLAN_REGISTRATION.revision),
        research_exposure()
    );
}

#[test]
fn each_other_binding_or_registration_exposes_nothing() {
    let compatible = Some(ToolCompatibilityDecision::Compatible);
    let cases: [(&[BoundTool], &str); 6] = [
        (&[], AGENT_PLAN_REGISTRATION.revision),
        (
            &[bound(ToolRegistrationLifecycle::Quarantined, compatible)],
            AGENT_PLAN_REGISTRATION.revision,
        ),
        (
            &[bound(ToolRegistrationLifecycle::Retired, compatible)],
            AGENT_PLAN_REGISTRATION.revision,
        ),
        (
            &[bound(ToolRegistrationLifecycle::Active, None)],
            AGENT_PLAN_REGISTRATION.revision,
        ),
        (
            &[BoundTool {
                registration_revision: "018f0000-0000-7001-8000-0000000e0dead".to_owned(),
                lifecycle: ToolRegistrationLifecycle::Active,
                compatibility: compatible,
            }],
            AGENT_PLAN_REGISTRATION.revision,
        ),
        (
            &[bound(ToolRegistrationLifecycle::Active, compatible)],
            HOST_FAKE_REGISTRATION.revision,
        ),
    ];
    for (bound, revision) in cases {
        assert_eq!(exposure(bound, revision), ToolExposure::empty());
    }
    assert_eq!(
        exposure(
            &[bound(ToolRegistrationLifecycle::Active, compatible)],
            AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION.revision
        ),
        ToolExposure::empty()
    );
}

#[test]
fn the_declarations_are_the_typed_intake_as_a_strict_object_schema() {
    let declarations: Vec<serde_json::Value> = research_exposure()
        .declarations()
        .into_iter()
        .map(|declaration| {
            json!({
                "name": declaration.name,
                "description": declaration.description,
                "parameters": declaration.parameters,
            })
        })
        .collect();
    assert_eq!(
        declarations,
        vec![
            json!({
                "name": "web_search",
                "description": "Searches the web for pages that answer one query. Use it when \
                                the author request needs facts that the supplied text does not \
                                give.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query, 1 to 512 characters."
                        },
                        "result_count": {
                            "type": ["integer", "null"],
                            "description": "The number of results, 1 to 10. Null takes the \
                                            default."
                        }
                    },
                    "required": ["query", "result_count"],
                    "additionalProperties": false
                }
            }),
            json!({
                "name": "web_fetch",
                "description": "Reads the text of up to 5 web pages that you give by URL.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "urls": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "1 to 5 absolute http or https URLs."
                        }
                    },
                    "required": ["urls"],
                    "additionalProperties": false
                }
            }),
        ]
    );
}

#[test]
fn the_declared_arguments_fit_the_typed_intake() {
    assert_eq!(
        WEB_SEARCH_TOOL_SPEC.intake(&json!({"query": "tide tables", "result_count": null})),
        Ok(crate::ToolIntake::WebSearch {
            query: "tide tables".to_owned(),
            result_count: None,
        })
    );
}

#[test]
fn the_digest_follows_the_projection_and_the_snapshot_pins_the_revisions() {
    let full = research_exposure();
    let empty = ToolExposure::empty();
    assert_ne!(full.digest(), empty.digest());
    assert_eq!(full.digest(), research_exposure().digest());
    assert_eq!(
        empty.digest(),
        "sha256:fe2f3b4ef49492d81cb350fb689bf9f9dff6cfd1817d72d6ff9fe3350e3d5e6a"
    );
    let snapshot = full.snapshot();
    assert_eq!(
        snapshot,
        json!({
            "digest": full.digest(),
            "tools": [
                {
                    "name": "web_search",
                    "registration_id": "web_research",
                    "registration_revision": EXA_TOOL_REGISTRATION.revision,
                    "spec_revision": "storyos.tool.web-search.v1"
                },
                {
                    "name": "web_fetch",
                    "registration_id": "web_research",
                    "registration_revision": EXA_TOOL_REGISTRATION.revision,
                    "spec_revision": "storyos.tool.web-fetch.v1"
                }
            ]
        })
    );
}

fn item(
    role: StreamItemRole,
    state: StreamItemState,
    native_type: Option<&str>,
) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "item-1".to_owned(),
        role,
        state,
        text: None,
        summary: None,
        call_id: Some("call-1".to_owned()),
        arguments: Some("{\"query\":\"tide tables\",\"result_count\":null}".to_owned()),
        refusal: None,
        native_type: native_type.map(str::to_owned),
        name: Some("web_search".to_owned()),
    }
}

#[test]
fn only_a_complete_function_call_item_requests_a_function() {
    assert!(requests_function(&[
        item(StreamItemRole::Assistant, StreamItemState::Complete, None),
        item(
            StreamItemRole::Tool,
            StreamItemState::Complete,
            Some("function_call")
        ),
    ]));
    let not_requests = [
        item(
            StreamItemRole::Tool,
            StreamItemState::Provisional,
            Some("function_call"),
        ),
        item(
            StreamItemRole::Tool,
            StreamItemState::Incomplete,
            Some("function_call"),
        ),
        item(
            StreamItemRole::Tool,
            StreamItemState::Complete,
            Some("reasoning"),
        ),
        item(
            StreamItemRole::Assistant,
            StreamItemState::Complete,
            Some("function_call"),
        ),
    ];
    for item in not_requests {
        assert!(!requests_function(&[item]));
    }
    assert!(!requests_function(&[]));
}

#[test]
fn only_an_ordinary_response_without_a_decision_and_with_a_complete_call_requests_a_function() {
    let call = [item(
        StreamItemRole::Tool,
        StreamItemState::Complete,
        Some("function_call"),
    )];
    let reply = [item(
        StreamItemRole::Assistant,
        StreamItemState::Complete,
        None,
    )];
    let decision = AgentDecisionOutcome::Decision {
        kind: AgentDecisionKind::Advisory {
            text: "Keep it.".to_owned(),
        },
        selected: true,
        advances_continuation: true,
    };
    let disposition = |probe: bool, outcome: &AgentDecisionOutcome, items: &[NativeStreamItem]| {
        response_disposition(&ResponseFacts {
            probe,
            outcome,
            items,
        })
    };

    assert_eq!(
        [
            disposition(false, &AgentDecisionOutcome::NoDecision, &call),
            disposition(true, &AgentDecisionOutcome::NoDecision, &call),
            disposition(false, &AgentDecisionOutcome::NoDecision, &reply),
            disposition(false, &decision, &call),
            disposition(false, &decision, &reply),
        ],
        [
            ResponseDisposition::FunctionRequested,
            ResponseDisposition::NoDecision,
            ResponseDisposition::NoDecision,
            ResponseDisposition::Decision,
            ResponseDisposition::Decision,
        ]
    );
}

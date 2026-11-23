use serde_json::json;

use super::{
    BoundTool, ExposedTool, ExposureFacts, ResponseDisposition, ResponseFacts,
    ToolCompatibilityDecision, ToolExposure, requests_function, response_disposition,
};
use crate::shared_tests::exposure_facts;
use crate::{
    AGENT_PLAN_REGISTRATION, AgentDecisionKind, AgentDecisionOutcome, EXA_TOOL_REGISTRATION,
    HOST_FAKE_TOOL_LOOP_REGISTRATION, HOST_TOOL_REGISTRATION, NativeStreamItem, StreamItemRole,
    StreamItemState, ToolRegistrationLifecycle, WEB_FETCH_TOOL_SPEC, WEB_SEARCH_TOOL_SPEC,
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
        ..exposure_facts(model_registration_revision)
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
    let mut expected = research_exposure();
    expected
        .tools
        .extend(exposure(&[], AGENT_PLAN_REGISTRATION.revision).tools);
    assert_eq!(
        exposure(&active, AGENT_PLAN_REGISTRATION.revision),
        expected
    );
}

#[test]
fn each_other_binding_or_registration_exposes_nothing() {
    let compatible = Some(ToolCompatibilityDecision::Compatible);
    let host_tools = exposure(&[], AGENT_PLAN_REGISTRATION.revision);
    let cases: [(&[BoundTool], &str); 5] = [
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
    ];
    for (bound, revision) in cases {
        assert_eq!(exposure(bound, revision), host_tools);
    }
    // A Model Registration with no qualified function calling, or an unknown revision, exposes no
    // Tool.
    for revision in [
        crate::PROBE_FIXTURE_REGISTRATION.revision,
        "an unknown revision",
    ] {
        assert_eq!(
            exposure(
                &[bound(ToolRegistrationLifecycle::Active, compatible)],
                revision
            ),
            ToolExposure::empty()
        );
    }
}

#[test]
fn a_registration_of_the_tool_loop_profile_declares_the_host_tools_of_its_mapping() {
    // A Run of the author never records Memory Observations, never applies a pass, and never
    // records an intent.
    let host = |registration: &'static crate::HostToolRegistration| ToolExposure {
        tools: registration
            .tools
            .iter()
            .filter(|spec| {
                !matches!(
                    spec.operation,
                    crate::ToolOperation::RecordMemoryObservations
                        | crate::ToolOperation::ApplyMemoryOperations
                        | crate::ToolOperation::RecordCharacterIntent
                )
            })
            .map(|spec| ExposedTool {
                name: spec.operation.function_name(),
                registration_id: registration.id,
                registration_revision: registration.revision,
                spec,
            })
            .collect(),
    };
    let names = |exposure: ToolExposure| -> Vec<&str> {
        exposure.tools.iter().map(|tool| tool.name).collect()
    };
    let with_catalog = |revision: &str| {
        ToolExposure::compute(&ExposureFacts {
            skill_catalog: true,
            memory_use: true,
            hybrid_search: true,
            ..exposure_facts(revision)
        })
    };
    let without_skill = |registration| {
        let mut exposure = host(registration);
        exposure.tools.retain(|tool| {
            !matches!(
                tool.spec.operation,
                crate::ToolOperation::LoadSkill
                    | crate::ToolOperation::ReadSkillResource
                    | crate::ToolOperation::ReadMemoryEntry
                    | crate::ToolOperation::SearchMemory
                    | crate::ToolOperation::HybridSearchManuscript
            )
        });
        exposure
    };
    // The current mapping and the fake tool loop mapping declare `…0e0106`. Each declares
    // `load_skill` and `read_skill_resource` only for a Run whose pinned catalog has a Skill (ADR
    // 0089 section 3), `read_memory_entry` and `search_memory` only for a Run whose captured
    // settings have use (ADR 0094 section 4), and `hybrid_search_manuscript` only when the search
    // by meaning is possible (ADR 0095 section 4).
    for revision in [
        HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
        AGENT_PLAN_REGISTRATION.revision,
    ] {
        assert_eq!(with_catalog(revision), host(&HOST_TOOL_REGISTRATION));
        assert_eq!(
            exposure(&[], revision),
            without_skill(&HOST_TOOL_REGISTRATION)
        );
    }
    assert_eq!(
        names(with_catalog(AGENT_PLAN_REGISTRATION.revision)),
        vec![
            "propose_edit",
            "submit_research",
            "read_tool_result",
            "load_skill",
            "read_skill_resource",
            "read_chapter",
            "search_manuscript",
            "hybrid_search_manuscript",
            "write_memory",
            "read_memory_entry",
            "search_memory",
            "update_plan",
            "finish"
        ]
    );
    assert_eq!(
        (
            HOST_TOOL_REGISTRATION.revision,
            HOST_TOOL_REGISTRATION.tools.len(),
        ),
        ("018f0000-0000-7001-8000-0000000e0106", 16)
    );
    let declared = host(&HOST_TOOL_REGISTRATION);
    assert_eq!(
        (
            declared.tools[4].declaration().parameters,
            declared.tools[12].declaration().parameters,
        ),
        (
            json!({
                "type": "object",
                "properties": {
                    "skill": {
                        "type": "string",
                        "description": "The name of a Skill that this work has."
                    },
                    "path": {
                        "type": "string",
                        "description": "The path of one file in the package of the Skill, \
                                        relative to the root of the package, with / between the \
                                        parts."
                    }
                },
                "required": ["skill", "path"],
                "additionalProperties": false
            }),
            json!({"type": "object", "properties": {}, "required": [], "additionalProperties": false}),
        )
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

/// A capture exposes its three read-only Tools and its output Tool. A consolidation pass exposes
/// only its output Tool. This is also true with a research binding, a Skill, and the search by meaning. The
/// Memory Settings of a Conversation do not control a capture (ADR 0094 sections 4 to 6). Only a
/// Run of the author declares `update_plan` (ADR 0103 section 2).
#[test]
fn a_background_run_gets_only_the_tools_of_its_kind() {
    let active = [bound(
        ToolRegistrationLifecycle::Active,
        Some(ToolCompatibilityDecision::Compatible),
    )];
    let names = |run_kind, memory_use| -> Vec<&str> {
        ToolExposure::compute(&ExposureFacts {
            bound: &active,
            skill_catalog: true,
            memory_use,
            hybrid_search: true,
            run_kind,
            ..exposure_facts(HOST_FAKE_TOOL_LOOP_REGISTRATION.revision)
        })
        .tools
        .iter()
        .map(|tool| tool.name)
        .collect()
    };
    let capture = [
        "read_chapter",
        "search_manuscript",
        "read_memory_entry",
        "record_memory_observations",
    ];
    let author_only = [
        "web_search",
        "web_fetch",
        "propose_edit",
        "submit_research",
        "finish",
        "read_tool_result",
        "load_skill",
        "read_skill_resource",
        "write_memory",
        "search_memory",
        "hybrid_search_manuscript",
        "update_plan",
    ];
    let author = names(crate::AgentRunKind::Author, true);

    assert_eq!(
        [true, false].map(|memory_use| names(crate::AgentRunKind::MemoryCapture, memory_use)),
        [capture.to_vec(), capture.to_vec()]
    );
    assert!(author_only.iter().all(|name| author.contains(name)));
    assert_eq!(
        [
            crate::AgentRunKind::MemoryConsolidation,
            crate::AgentRunKind::IntentPass
        ]
        .map(|kind| names(kind, true)),
        [["apply_memory_operations"], ["record_character_intent"]]
    );
    assert!(!author.contains(&"record_memory_observations"));
    assert!(!author.contains(&"apply_memory_operations"));
    assert!(!author.contains(&"record_character_intent"));
}

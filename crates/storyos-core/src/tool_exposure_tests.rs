use serde_json::json;

use super::{
    BoundTool, ExposedTool, ExposureFacts, ResponseDisposition, ResponseFacts,
    ToolCompatibilityDecision, ToolExposure, requests_function, response_disposition,
};
use crate::{
    AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION, AGENT_PLAN_REGISTRATION, AgentDecisionKind,
    AgentDecisionOutcome, EXA_TOOL_REGISTRATION, HOST_FAKE_REGISTRATION,
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
        model_registration_revision,
        skill_catalog: false,
        memory_use: false,
        run_kind: crate::AgentRunKind::Author,
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
    // A Model Registration with no qualified function calling exposes no Tool.
    for revision in [
        HOST_FAKE_REGISTRATION.revision,
        AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION.revision,
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
    // A Run of the author never records Memory Observations.
    let host = |registration: &'static crate::HostToolRegistration| ToolExposure {
        tools: registration
            .tools
            .iter()
            .filter(|spec| spec.operation != crate::ToolOperation::RecordMemoryObservations)
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
            bound: &[],
            model_registration_revision: revision,
            skill_catalog: true,
            memory_use: true,
            run_kind: crate::AgentRunKind::Author,
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
            )
        });
        exposure
    };
    // The mapping `v8` and the fake tool loop mapping declare `…0e0104`, and a Run that pinned
    // the Registration of the mapping `v7` keeps `…0e0103`. Each declares `load_skill` and
    // `read_skill_resource` only for a Run whose pinned catalog has a Skill (ADR 0089 section 3),
    // and `read_memory_entry` and `search_memory` only for a Run whose captured settings have use
    // (ADR 0094 section 4).
    for (revision, registration) in [
        (
            HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
            &HOST_TOOL_REGISTRATION,
        ),
        (AGENT_PLAN_REGISTRATION.revision, &HOST_TOOL_REGISTRATION),
        (
            crate::AGENT_PLAN_SKILL_REGISTRATION.revision,
            &crate::SKILL_HOST_TOOL_REGISTRATION,
        ),
    ] {
        assert_eq!(with_catalog(revision), host(registration));
        assert_eq!(exposure(&[], revision), without_skill(registration));
    }
    // A Run that pinned the Registration of the mapping `v6` keeps four Host Tools, and a Run
    // that pinned the Registration of the mapping `v5` keeps three (ADR 0093).
    let v6 = crate::AGENT_PLAN_READ_RESULT_REGISTRATION.revision;
    let v5 = crate::AGENT_PLAN_TOOL_LOOP_REGISTRATION.revision;
    for (revision, registration) in [
        (v6, &crate::READ_RESULT_HOST_TOOL_REGISTRATION),
        (v5, &crate::FIRST_HOST_TOOL_REGISTRATION),
    ] {
        assert_eq!(exposure(&[], revision), host(registration));
        assert_eq!(with_catalog(revision), host(registration));
    }
    assert_eq!(
        (
            names(exposure(&[], v5)),
            names(exposure(&[], v6)),
            names(with_catalog(AGENT_PLAN_REGISTRATION.revision)),
        ),
        (
            vec!["propose_edit", "submit_research", "finish"],
            vec![
                "propose_edit",
                "submit_research",
                "read_tool_result",
                "finish"
            ],
            vec![
                "propose_edit",
                "submit_research",
                "read_tool_result",
                "load_skill",
                "read_skill_resource",
                "read_chapter",
                "search_manuscript",
                "write_memory",
                "read_memory_entry",
                "search_memory",
                "finish"
            ]
        )
    );
    assert_eq!(
        (
            HOST_TOOL_REGISTRATION.revision,
            crate::SKILL_HOST_TOOL_REGISTRATION.revision,
            crate::READ_RESULT_HOST_TOOL_REGISTRATION.revision,
            HOST_TOOL_REGISTRATION.tools.len(),
        ),
        (
            "018f0000-0000-7001-8000-0000000e0104",
            "018f0000-0000-7001-8000-0000000e0103",
            "018f0000-0000-7001-8000-0000000e0102",
            12,
        )
    );
    let declared = host(&HOST_TOOL_REGISTRATION);
    assert_eq!(
        (
            declared.tools[4].declaration().parameters,
            declared.tools[10].declaration().parameters,
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

/// A capture exposes only its output Tool and a consolidation pass exposes no Tool, also with an
/// active research binding, a Skill, and Memory use (ADR 0094 sections 5 and 6).
#[test]
fn a_background_run_gets_only_the_tools_of_its_kind() {
    let active = [bound(
        ToolRegistrationLifecycle::Active,
        Some(ToolCompatibilityDecision::Compatible),
    )];
    let names = |run_kind| -> Vec<&str> {
        ToolExposure::compute(&ExposureFacts {
            bound: &active,
            model_registration_revision: HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
            skill_catalog: true,
            memory_use: true,
            run_kind,
        })
        .tools
        .iter()
        .map(|tool| tool.name)
        .collect()
    };

    assert_eq!(
        names(crate::AgentRunKind::MemoryCapture),
        ["record_memory_observations"]
    );
    assert!(names(crate::AgentRunKind::MemoryConsolidation).is_empty());
    let author = names(crate::AgentRunKind::Author);
    assert!(author.contains(&"web_search") && author.contains(&"finish"));
    assert!(!author.contains(&"record_memory_observations"));
}

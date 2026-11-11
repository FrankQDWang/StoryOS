use serde_json::json;

use super::{
    CurrentToolAuthority, ExposureSnapshot, ToolCallBatch, ToolCallBatchRefusal, ValidatedToolCall,
    validate_tool_call_batch,
};
use crate::{
    AgentDecisionKind, AgentDecisionOutcome, CitableSources, DecisionCandidate,
    EXA_TOOL_REGISTRATION, EnabledTool, ExposureFacts, IntakeRefusal, ModelOutput,
    NativeStreamItem, OutputPhase, StreamItemRole, StreamItemState, ToolCompatibilityDecision,
    ToolExposure, ToolIntake, ToolRegistrationLifecycle, validate_agent_decision,
};

/// The Snapshot of the research Exposure, as the Worker pins it at the dispatch claim.
fn pinned() -> ExposureSnapshot {
    let exposure = ToolExposure::compute(&ExposureFacts {
        enabled: &[EnabledTool {
            registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
            lifecycle: ToolRegistrationLifecycle::Active,
            compatibility: Some(ToolCompatibilityDecision::Compatible),
        }],
        model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
    });
    ExposureSnapshot::decode(Some(&exposure.snapshot())).unwrap()
}

/// The current authority of the pinned research Registration revision.
fn authority(enabled: bool, lifecycle: ToolRegistrationLifecycle) -> CurrentToolAuthority {
    CurrentToolAuthority {
        registration_id: "web_research".to_owned(),
        registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
        enabled,
        lifecycle,
    }
}

/// The Project enables the pinned Registration now, and its revision is active now.
fn current() -> [CurrentToolAuthority; 1] {
    [authority(
        /*enabled*/ true,
        ToolRegistrationLifecycle::Active,
    )]
}

fn call(item_id: &str, call_id: &str, name: &str, arguments: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: item_id.to_owned(),
        role: StreamItemRole::Tool,
        state: StreamItemState::Complete,
        text: None,
        summary: None,
        call_id: Some(call_id.to_owned()),
        arguments: Some(arguments.to_owned()),
        refusal: None,
        native_type: Some("function_call".to_owned()),
        name: Some(name.to_owned()),
    }
}

fn message(text: &str) -> NativeStreamItem {
    NativeStreamItem {
        item_id: "msg_01".to_owned(),
        role: StreamItemRole::Assistant,
        state: StreamItemState::Complete,
        text: Some(text.to_owned()),
        summary: None,
        call_id: None,
        arguments: None,
        refusal: None,
        native_type: None,
        name: None,
    }
}

const SEARCH: &str = r#"{"query":"tide tables of 1890","result_count":null}"#;
const FETCH: &str = r#"{"urls":["https://example.org/tides"]}"#;

fn search_call(call_id: &str) -> ValidatedToolCall {
    ValidatedToolCall {
        call_id: call_id.to_owned(),
        name: "web_search".to_owned(),
        registration_id: "web_research".to_owned(),
        registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
        spec_revision: crate::WEB_SEARCH_TOOL_SPEC.revision.to_owned(),
        intake: ToolIntake::WebSearch {
            query: "tide tables of 1890".to_owned(),
            result_count: None,
        },
    }
}

#[test]
fn a_complete_exposed_batch_validates_in_item_order_with_its_typed_arguments() {
    let items = [
        message("Looking it up."),
        call("fc_01", "call_01", "web_search", SEARCH),
        call("fc_02", "call_02", "web_fetch", FETCH),
    ];
    let fetch = ValidatedToolCall {
        call_id: "call_02".to_owned(),
        name: "web_fetch".to_owned(),
        registration_id: "web_research".to_owned(),
        registration_revision: EXA_TOOL_REGISTRATION.revision.to_owned(),
        spec_revision: crate::WEB_FETCH_TOOL_SPEC.revision.to_owned(),
        intake: ToolIntake::WebFetch {
            urls: vec!["https://example.org/tides".to_owned()],
        },
    };

    assert_eq!(
        (
            validate_tool_call_batch(StreamItemState::Complete, &items, &pinned(), &current()),
            validate_tool_call_batch(
                StreamItemState::Complete,
                &[message("Keep it.")],
                &pinned(),
                &current()
            ),
            validate_tool_call_batch(
                StreamItemState::Incomplete,
                &[message("Keep")],
                &pinned(),
                &current()
            ),
            search_call("call_01").arguments(),
            fetch.arguments(),
            search_call("call_01").argument_digest(),
        ),
        (
            ToolCallBatch::Valid(vec![search_call("call_01"), fetch]),
            ToolCallBatch::none(),
            ToolCallBatch::none(),
            json!({"query": "tide tables of 1890", "result_count": null}),
            json!({"urls": ["https://example.org/tides"]}),
            "sha256:012398a6d5fe487115a70022409a7dd2e27bfbcfdbb9930129b6d2ea5f84d9b2".to_owned(),
        )
    );
}

#[test]
fn one_invalid_member_rejects_the_whole_batch() {
    let valid = call("fc_01", "call_01", "web_search", SEARCH);
    let refusal_in = |state, items: &[NativeStreamItem]| match validate_tool_call_batch(
        state,
        items,
        &pinned(),
        &current(),
    ) {
        ToolCallBatch::Invalid(refusal) => refusal,
        ToolCallBatch::Valid(calls) => panic!("valid batch of {} members", calls.len()),
    };
    let refusal = |items: &[NativeStreamItem]| refusal_in(StreamItemState::Complete, items);
    let long_query = format!(r#"{{"query":"{}"}}"#, "x".repeat(513));
    let six_urls =
        json!({"urls": (0..6).map(|n| format!("https://example.org/{n}")).collect::<Vec<_>>()});
    let unexposed = ExposureSnapshot::decode(Some(&ToolExposure::empty().snapshot())).unwrap();
    let hosted = NativeStreamItem {
        native_type: Some("web_search_call".to_owned()),
        ..call("ws_01", "call_09", "web_search", SEARCH)
    };
    let provisional = NativeStreamItem {
        state: StreamItemState::Provisional,
        ..call("fc_03", "call_03", "web_search", r#"{"query":"tide"#)
    };
    let no_call_id = NativeStreamItem {
        call_id: None,
        ..call("fc_04", "call_04", "web_search", SEARCH)
    };
    let long_call_id = "c".repeat(257);
    let cut_message = NativeStreamItem {
        state: StreamItemState::Incomplete,
        ..message("Looking")
    };

    assert_eq!(
        [
            refusal(&[valid.clone(), call("fc_02", "call_02", "edit", "{}")]),
            refusal(&[call("fc_02", "call_02", "web_search", r#"{"query":"#)]),
            refusal(&[call("fc_02", "call_02", "web_search", "[]")]),
            refusal(&[call("fc_02", "call_02", "web_search", &long_query)]),
            refusal(&[call("fc_02", "call_02", "web_fetch", &six_urls.to_string())]),
            refusal(&[call(
                "fc_02",
                "call_02",
                "web_fetch",
                r#"{"urls":["ftp://example.org/a"]}"#
            )]),
            refusal(&[valid.clone(), call("fc_02", "call_01", "web_fetch", FETCH)]),
            refusal(&[valid.clone(), hosted]),
            refusal(&[provisional]),
            refusal(&[no_call_id]),
            refusal(&[call("fc_05", "", "web_search", SEARCH)]),
            refusal(&[call("fc_06", &long_call_id, "web_search", SEARCH)]),
            refusal(&[call("fc_07", "call\u{0}07", "web_search", SEARCH)]),
            refusal(&[call("fc_08", "call\n08", "web_search", SEARCH)]),
            refusal(&[call("fc_09", "call\u{9f}09", "web_search", SEARCH)]),
            refusal_in(StreamItemState::Incomplete, std::slice::from_ref(&valid)),
            refusal_in(
                StreamItemState::Cancelled,
                &[message("Looking"), valid.clone()]
            ),
            refusal(&[cut_message, valid.clone()]),
            match validate_tool_call_batch(
                StreamItemState::Complete,
                &[valid],
                &unexposed,
                &current(),
            ) {
                ToolCallBatch::Invalid(refusal) => refusal,
                ToolCallBatch::Valid(_) => panic!("exposed"),
            },
        ],
        [
            ToolCallBatchRefusal::Unexposed {
                call_id: "call_02".to_owned()
            },
            ToolCallBatchRefusal::Malformed {
                call_id: "call_02".to_owned()
            },
            ToolCallBatchRefusal::Malformed {
                call_id: "call_02".to_owned()
            },
            ToolCallBatchRefusal::Intake {
                call_id: "call_02".to_owned(),
                refusal: IntakeRefusal::Query
            },
            ToolCallBatchRefusal::Intake {
                call_id: "call_02".to_owned(),
                refusal: IntakeRefusal::UrlCount
            },
            ToolCallBatchRefusal::Intake {
                call_id: "call_02".to_owned(),
                refusal: IntakeRefusal::Url
            },
            ToolCallBatchRefusal::DuplicateCallId {
                call_id: "call_01".to_owned()
            },
            ToolCallBatchRefusal::UnexpectedItem {
                item_id: "ws_01".to_owned()
            },
            ToolCallBatchRefusal::Incomplete {
                item_id: "fc_03".to_owned()
            },
            ToolCallBatchRefusal::Incomplete {
                item_id: "fc_04".to_owned()
            },
            ToolCallBatchRefusal::InvalidCallId {
                item_id: "fc_05".to_owned()
            },
            ToolCallBatchRefusal::InvalidCallId {
                item_id: "fc_06".to_owned()
            },
            ToolCallBatchRefusal::InvalidCallId {
                item_id: "fc_07".to_owned()
            },
            ToolCallBatchRefusal::InvalidCallId {
                item_id: "fc_08".to_owned()
            },
            ToolCallBatchRefusal::InvalidCallId {
                item_id: "fc_09".to_owned()
            },
            ToolCallBatchRefusal::ResponseIncomplete,
            ToolCallBatchRefusal::ResponseIncomplete,
            ToolCallBatchRefusal::ResponseIncomplete,
            ToolCallBatchRefusal::Unexposed {
                call_id: "call_01".to_owned()
            },
        ]
    );
}

/// The refusal of a complete, exposed, intake-valid batch under the `current` authority.
fn current_refusal(current: &[CurrentToolAuthority]) -> (ToolCallBatchRefusal, &'static str) {
    let items = [
        call("fc_01", "call_01", "web_search", SEARCH),
        call("fc_02", "call_02", "web_fetch", FETCH),
    ];
    match validate_tool_call_batch(StreamItemState::Complete, &items, &pinned(), current) {
        ToolCallBatch::Invalid(refusal) => {
            let label = refusal.label();
            (refusal, label)
        }
        ToolCallBatch::Valid(calls) => panic!("valid batch of {} members", calls.len()),
    }
}

#[test]
fn a_registration_that_the_project_does_not_enable_now_rejects_the_batch() {
    let other_revision = CurrentToolAuthority {
        registration_revision: "018f0000-0000-7001-8000-0000000e0f09".to_owned(),
        ..authority(/*enabled*/ true, ToolRegistrationLifecycle::Active)
    };
    let not_enabled = (
        ToolCallBatchRefusal::NotEnabled {
            call_id: "call_01".to_owned(),
        },
        "tool_not_enabled",
    );

    assert_eq!(
        [
            current_refusal(&[authority(
                /*enabled*/ false,
                ToolRegistrationLifecycle::Active
            )]),
            current_refusal(&[]),
            current_refusal(&[other_revision]),
        ],
        [not_enabled.clone(), not_enabled.clone(), not_enabled]
    );
}

#[test]
fn a_pinned_revision_that_is_not_active_now_rejects_the_batch() {
    let unavailable = (
        ToolCallBatchRefusal::RegistrationUnavailable {
            call_id: "call_01".to_owned(),
        },
        "tool_registration_unavailable",
    );

    assert_eq!(
        [
            current_refusal(&[authority(
                /*enabled*/ true,
                ToolRegistrationLifecycle::Quarantined
            )]),
            current_refusal(&[authority(
                /*enabled*/ true,
                ToolRegistrationLifecycle::Retired
            )]),
        ],
        [unavailable.clone(), unavailable]
    );
}

#[test]
fn a_valid_batch_is_the_decision_and_an_invalid_batch_makes_none() {
    let advisory = ModelOutput {
        phase: OutputPhase::FinalAnswer,
        candidate: DecisionCandidate::Advisory {
            text: "Looking it up.".to_owned(),
        },
        prose_changes: None,
    };
    let valid = ToolCallBatch::Valid(vec![search_call("call_01")]);
    let invalid = ToolCallBatch::Invalid(ToolCallBatchRefusal::Unexposed {
        call_id: "call_01".to_owned(),
    });
    let tool_request = AgentDecisionOutcome::Decision {
        kind: AgentDecisionKind::ToolRequest {
            calls: vec![search_call("call_01")],
        },
        selected: true,
        advances_continuation: false,
    };

    assert_eq!(
        [
            validate_agent_decision(None, &[], &CitableSources::default(), valid.clone()),
            validate_agent_decision(Some(&advisory), &[], &CitableSources::default(), valid),
            validate_agent_decision(
                Some(&advisory),
                &[],
                &CitableSources::default(),
                invalid.clone()
            ),
            validate_agent_decision(None, &[], &CitableSources::default(), invalid),
            validate_agent_decision(None, &[], &CitableSources::default(), ToolCallBatch::none()),
        ],
        [
            tool_request.clone(),
            tool_request,
            AgentDecisionOutcome::NoDecision,
            AgentDecisionOutcome::NoDecision,
            AgentDecisionOutcome::NoDecision,
        ]
    );
}

#[test]
fn the_snapshot_record_decodes_to_the_pinned_exposure() {
    let damaged = json!({"digest": "sha256:00", "tools": [{"name": "web_search"}]});

    assert_eq!(
        (
            pinned().tools.len(),
            ExposureSnapshot::decode(None),
            ExposureSnapshot::decode(Some(&serde_json::Value::Null)),
            ExposureSnapshot::decode(Some(&damaged)),
        ),
        (
            2,
            Some(ExposureSnapshot::empty()),
            Some(ExposureSnapshot::empty()),
            None
        )
    );
}

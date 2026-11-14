use serde_json::json;

use super::*;
use crate::{
    EXA_TOOL_REGISTRATION, ToolEffectEnvelope, ToolOperation, WEB_FETCH_TOOL_SPEC,
    WEB_SEARCH_TOOL_SPEC,
};

/// A ToolSpec of a later stage that writes outside StoryOS. No Registration maps it.
const EXTERNAL_WRITE_SPEC: ToolSpec = ToolSpec {
    revision: "storyos.tool.test-external-write.v1",
    operation: ToolOperation::WebSearch,
    intake_contract: &[IntakeField::Query],
    envelope: Some(ToolEffectEnvelope {
        external_read: ExternalRead::OneSearch,
        external_write: ExternalWrite::Declared,
        outbound_disclosure: IntakeField::Query,
    }),
    result_text_max_bytes: 16,
};

fn search_effect(spec: &ToolSpec) -> ToolEffectRequest {
    ToolEffectRequest::derive(
        spec,
        &ToolIntake::WebSearch {
            query: "tide tables of 1890".to_owned(),
            result_count: None,
        },
    )
    .unwrap()
}

fn admitted_facts(effect: &ToolEffectRequest) -> ToolDispatchFacts<'_> {
    ToolDispatchFacts {
        same_scope: true,
        run_grant_names_tool: true,
        lifecycle: ToolRegistrationLifecycle::Active,
        compatibility: Some(ToolCompatibilityDecision::Compatible),
        binding_pins_revision: true,
        credential_reference: Some("settings:STORYOS_EXA_API_KEY"),
        grant_scope: Some(GrantScope::ResearchToolV1),
        effect,
    }
}

#[test]
fn the_plan_maps_the_stored_arguments_to_the_pinned_server_tool_and_its_effects() {
    let search = ToolCallPlan::derive(
        EXA_TOOL_REGISTRATION.revision,
        WEB_SEARCH_TOOL_SPEC.revision,
        &json!({"query": "tide tables of 1890", "result_count": null}),
    )
    .unwrap();
    let fetch = ToolCallPlan::derive(
        EXA_TOOL_REGISTRATION.revision,
        WEB_FETCH_TOOL_SPEC.revision,
        &json!({"urls": ["https://example.org/tides"]}),
    )
    .unwrap();

    assert_eq!(
        (
            (
                search.tool.name,
                json!(search.server_arguments),
                search.effect.record()
            ),
            (
                fetch.tool.name,
                json!(fetch.server_arguments),
                fetch.effect.record()
            ),
            ToolCallPlan::derive(
                EXA_TOOL_REGISTRATION.revision,
                EXTERNAL_WRITE_SPEC.revision,
                &json!({"query": "x"}),
            ),
        ),
        (
            (
                "web_search_exa",
                json!({"query": "tide tables of 1890",
                       "objective": "Rank first the pages that answer this search: tide tables of 1890"}),
                json!({"external_read": "one_search", "external_write": "none",
                       "outbound_disclosure": "agent_query", "disclosed": ["tide tables of 1890"]}),
            ),
            (
                "web_fetch_exa",
                json!({"urls": ["https://example.org/tides"]}),
                json!({"external_read": "given_urls", "external_write": "none",
                       "outbound_disclosure": "agent_url_selection",
                       "disclosed": ["https://example.org/tides"]}),
            ),
            None,
        )
    );
}

#[test]
fn both_research_effects_dispatch_and_each_changed_authority_refuses_with_its_reason() {
    let search = search_effect(&WEB_SEARCH_TOOL_SPEC);
    let fetch = ToolEffectRequest::derive(
        &WEB_FETCH_TOOL_SPEC,
        &ToolIntake::WebFetch {
            urls: vec!["https://example.org/tides".to_owned()],
        },
    )
    .unwrap();
    let write = search_effect(&EXTERNAL_WRITE_SPEC);
    let refusal = |change: fn(&mut ToolDispatchFacts<'_>)| {
        let mut facts = admitted_facts(&search);
        change(&mut facts);
        admit_tool_dispatch(&facts).map_err(|refusal| (refusal.code(), refusal.state()))
    };

    assert_eq!(
        [
            admit_tool_dispatch(&admitted_facts(&search)).map_err(|r| (r.code(), r.state())),
            admit_tool_dispatch(&admitted_facts(&fetch)).map_err(|r| (r.code(), r.state())),
            admit_tool_dispatch(&admitted_facts(&write)).map_err(|r| (r.code(), r.state())),
            refusal(|facts| facts.same_scope = false),
            refusal(|facts| facts.run_grant_names_tool = false),
            refusal(|facts| facts.lifecycle = ToolRegistrationLifecycle::Quarantined),
            refusal(|facts| facts.lifecycle = ToolRegistrationLifecycle::Retired),
            refusal(|facts| facts.compatibility = None),
            refusal(|facts| facts.binding_pins_revision = false),
            refusal(|facts| facts.credential_reference = None),
            refusal(|facts| facts.grant_scope = Some(GrantScope::ProjectWritingAssistanceV2)),
            refusal(|facts| facts.grant_scope = None),
        ],
        [
            Ok(()),
            Ok(()),
            Err(("tool_approval_required", ToolCallState::AwaitingApproval)),
            Err(("disclosure_outside_grant", ToolCallState::Refused)),
            Err(("tool_outside_run_grant", ToolCallState::Refused)),
            Err(("tool_registration_unavailable", ToolCallState::Refused)),
            Err(("tool_registration_unavailable", ToolCallState::Refused)),
            Err(("tool_registration_incompatible", ToolCallState::Refused)),
            Err(("tool_registration_incompatible", ToolCallState::Refused)),
            Err(("credential_unbound", ToolCallState::Refused)),
            Err(("disclosure_outside_grant", ToolCallState::Refused)),
            Err(("disclosure_outside_grant", ToolCallState::Refused)),
        ]
    );
}

#[test]
fn each_observation_settles_the_tool_call_the_attempt_and_the_effect_outcome() {
    let settle = |observation: ToolObservation| {
        let settlement = settle_tool_call(&EXTERNAL_WRITE_SPEC, &observation);
        (
            settlement.state,
            settlement.attempt,
            settlement.outcome.record(),
            settlement.result.map(|result| (result.text, result.bytes)),
            settlement.refusal,
        )
    };
    let read = |read: &str, disclosure: &str| json!({"external_read": read, "outbound_disclosure": disclosure});
    let called = |text: &[&str], is_error| ToolObservation::Called {
        text: text.iter().map(|block| (*block).to_owned()).collect(),
        is_error,
    };

    assert_eq!(
        [
            settle(called(&["Tides", "1890"], false)),
            settle(called(&[], false)),
            settle(called(&["Tides"], true)),
            settle(called(&["seventeen bytes!!"], false)),
            settle(called(&["a\u{0}b"], false)),
            settle(ToolObservation::JsonRpcError { code: -32602 }),
            settle(ToolObservation::Rejected {
                reason: "http_401".to_owned()
            }),
            settle(ToolObservation::InvalidResult {
                reason: "non_text_block"
            }),
            settle(ToolObservation::OutcomeUnknown),
            settle(ToolObservation::AnsweredWithoutResult {
                reason: "http_503".to_owned()
            }),
            settle(ToolObservation::AnsweredWithoutResult {
                reason: "response_incomplete".to_owned()
            }),
            settle(ToolObservation::NotSubmitted),
            settle(ToolObservation::Drift(vec![
                ContractDifference::ToolRemoved {
                    tool: "web_search_exa".to_owned(),
                }
            ])),
        ],
        [
            (
                ToolCallState::Settled,
                ToolAttemptSettlement::TerminalResult,
                read("confirmed", "confirmed"),
                Some(("Tides\n1890".to_owned(), 10)),
                None,
            ),
            (
                ToolCallState::Settled,
                ToolAttemptSettlement::TerminalResult,
                read("confirmed", "confirmed"),
                Some((String::new(), 0)),
                None,
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NativeError,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NativeError,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NativeError,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NativeError,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NativeError,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NativeError,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Unknown,
                ToolAttemptSettlement::OutcomeUnknown,
                read("unknown", "unknown"),
                None,
                None
            ),
            (
                ToolCallState::Unknown,
                ToolAttemptSettlement::OutcomeUnknown,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Unknown,
                ToolAttemptSettlement::OutcomeUnknown,
                read("unknown", "confirmed"),
                None,
                None
            ),
            (
                ToolCallState::Failed,
                ToolAttemptSettlement::NotSubmitted,
                read("not_attempted", "not_attempted"),
                None,
                None,
            ),
            (
                ToolCallState::Refused,
                ToolAttemptSettlement::NotSubmitted,
                read("not_attempted", "not_attempted"),
                None,
                Some("tool_registration_unavailable"),
            ),
        ]
    );
}

#[test]
fn a_function_request_goes_to_the_gateway_refuses_or_continues() {
    let steps = [
        (true, true, true),
        (true, true, false),
        (true, false, false),
        (false, false, false),
        (false, true, true),
    ]
    .map(|(requests, supplies, unresolved)| function_request_step(requests, supplies, unresolved));

    assert_eq!(
        steps,
        [
            FunctionRequestStep::ToolGateway,
            FunctionRequestStep::Continue,
            FunctionRequestStep::Refuse,
            FunctionRequestStep::Complete,
            FunctionRequestStep::ToolGateway,
        ]
    );
}

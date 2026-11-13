use super::*;
use crate::{
    AGENT_PLAN_PENDING_REGISTRATION, AGENT_PLAN_PINNED_VERSION_PENDING_REGISTRATION,
    AGENT_PLAN_REGISTRATION, DeploymentDestination, HOST_FAKE_REGISTRATION,
};

const AUTHORIZATION: &str = "https://github.com/FrankQDWang/StoryOS/issues/696";

#[test]
fn only_an_authorized_decision_on_a_pending_entry_selects_a_probe() {
    use RuntimeQualification::{Pending, QualificationAuthorized, Qualified};
    let pending = AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION.revision;
    let call = ToolProbe::FunctionCall.instruction();

    assert_eq!(
        [
            tool_probe(call, pending, QualificationAuthorized),
            tool_probe(
                ToolProbe::FunctionCallOutput.instruction(),
                pending,
                QualificationAuthorized
            ),
            tool_probe(call, pending, Qualified),
            tool_probe(call, pending, Pending),
            tool_probe(
                call,
                AGENT_PLAN_REGISTRATION.revision,
                QualificationAuthorized
            ),
            tool_probe(
                call,
                HOST_FAKE_REGISTRATION.revision,
                QualificationAuthorized
            ),
            tool_probe(&format!("{call} "), pending, QualificationAuthorized),
        ],
        [
            Some(ToolProbe::FunctionCall),
            Some(ToolProbe::FunctionCallOutput),
            None,
            None,
            None,
            None,
            None,
        ]
    );
}

#[test]
fn the_link_authorizes_a_registration_only_while_it_awaits_qualification() {
    let authorized = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: Some(AUTHORIZATION.to_owned()),
    };

    assert_eq!(
        (
            AGENT_PLAN_PINNED_VERSION_PENDING_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_FUNCTION_CALLING_PENDING_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_PINNED_VERSION_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_REGISTRATION.awaits_qualification(),
            authorized.registration().revision,
            authorized.runtime_qualification(),
            authorized.qualification_authorization(),
        ),
        (
            true,
            true,
            false,
            false,
            AGENT_PLAN_REGISTRATION.revision,
            RuntimeQualification::Qualified,
            None,
        )
    );
}

#[test]
fn a_profile_without_function_calling_evidence_keeps_its_stored_form() {
    let entry = |profile: &ModelCapabilityProfile| {
        serde_json::to_value(profile).unwrap()["entries"]
            .get("function_calling")
            .map(|entry| entry["kind"].clone())
    };

    assert_eq!(
        [
            entry(AGENT_PLAN_PENDING_REGISTRATION.capability_profile),
            entry(&AGENT_PLAN_CAPABILITY_PROFILE),
            entry(&AGENT_PLAN_FUNCTION_CALLING_PENDING_CAPABILITY_PROFILE),
            entry(&AGENT_PLAN_FUNCTION_CALLING_CAPABILITY_PROFILE),
        ],
        [
            None,
            None,
            Some(serde_json::json!("documented")),
            Some(serde_json::json!("qualified")),
        ]
    );
}

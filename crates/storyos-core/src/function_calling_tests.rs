use super::*;
use crate::{
    AGENT_PLAN_CAPABILITY_PROFILE, AGENT_PLAN_REGISTRATION, DeploymentDestination,
    HOST_FAKE_TOOL_LOOP_REGISTRATION, PROBE_FIXTURE_CAPABILITY_PROFILE, PROBE_FIXTURE_REGISTRATION,
};

const AUTHORIZATION: &str = "https://github.com/FrankQDWang/StoryOS/issues/696";

#[test]
fn only_an_authorized_decision_on_a_pending_entry_selects_a_probe() {
    use RuntimeQualification::{Pending, QualificationAuthorized, Qualified};
    let pending = PROBE_FIXTURE_REGISTRATION.revision;
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
                HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
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
    const UNQUALIFIED_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
        revision: "test.unqualified",
        combinations: &[],
        ..AGENT_PLAN_CAPABILITY_PROFILE
    };
    let unqualified = ModelRegistration {
        revision: "test.unqualified",
        capability_profile: &UNQUALIFIED_PROFILE,
        ..AGENT_PLAN_REGISTRATION
    };
    let authorized = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: Some(AUTHORIZATION.to_owned()),
    };

    assert_eq!(
        (
            unqualified.awaits_qualification(),
            PROBE_FIXTURE_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_REGISTRATION.awaits_qualification(),
            authorized.registration().revision,
            authorized.runtime_qualification(),
            authorized.qualification_authorization(),
        ),
        (
            true,
            true,
            false,
            AGENT_PLAN_REGISTRATION.revision,
            RuntimeQualification::Qualified,
            None,
        )
    );
}

#[test]
fn a_profile_without_function_calling_evidence_keeps_its_stored_form() {
    const NO_EVIDENCE: ModelCapabilityProfile = ModelCapabilityProfile {
        revision: "test.no-evidence",
        entries: crate::model_registration::agent_plan_entries(CapabilityEvidence::Unknown),
        ..AGENT_PLAN_CAPABILITY_PROFILE
    };
    let entry = |profile: &ModelCapabilityProfile| {
        serde_json::to_value(profile).unwrap()["entries"]
            .get("function_calling")
            .map(|entry| entry["kind"].clone())
    };

    assert_eq!(
        [
            entry(&NO_EVIDENCE),
            entry(&PROBE_FIXTURE_CAPABILITY_PROFILE),
            entry(&AGENT_PLAN_CAPABILITY_PROFILE),
        ],
        [
            None,
            Some(serde_json::json!("documented")),
            Some(serde_json::json!("qualified")),
        ]
    );
}

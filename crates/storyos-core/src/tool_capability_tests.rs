use super::*;
use crate::{
    AGENT_PLAN_PENDING_REGISTRATION, AGENT_PLAN_PINNED_VERSION_PENDING_REGISTRATION,
    DeploymentDestination, HOST_FAKE_REGISTRATION,
};

const AUTHORIZATION: &str = "https://github.com/FrankQDWang/StoryOS/issues/696";

#[test]
fn only_an_authorized_decision_on_a_pending_tool_entry_selects_a_probe() {
    use RuntimeQualification::{Pending, QualificationAuthorized, Qualified};
    let pending = AGENT_PLAN_TOOL_PENDING_REGISTRATION.revision;
    let probe =
        |message: &str, revision: &str, qualification| tool_probe(message, revision, qualification);
    let web_search = ToolProbe::HostedWebSearch.instruction();

    assert_eq!(
        [
            probe(web_search, pending, QualificationAuthorized),
            probe(
                ToolProbe::FunctionCallOutput.instruction(),
                pending,
                QualificationAuthorized
            ),
            probe(web_search, pending, Qualified),
            probe(web_search, pending, Pending),
            probe(
                web_search,
                AGENT_PLAN_REGISTRATION.revision,
                QualificationAuthorized
            ),
            probe(
                web_search,
                HOST_FAKE_REGISTRATION.revision,
                QualificationAuthorized
            ),
            probe(&format!("{web_search} "), pending, QualificationAuthorized),
        ],
        [
            Some(ToolProbe::HostedWebSearch),
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
    let unauthorized = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };

    assert_eq!(
        (
            AGENT_PLAN_PINNED_VERSION_PENDING_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_TOOL_PENDING_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_PINNED_VERSION_REGISTRATION.awaits_qualification(),
            AGENT_PLAN_REGISTRATION.awaits_qualification(),
            authorized.registration().revision,
            authorized.runtime_qualification(),
            authorized.qualification_authorization(),
            unauthorized.runtime_qualification(),
        ),
        (
            true,
            true,
            false,
            false,
            AGENT_PLAN_REGISTRATION.revision,
            RuntimeQualification::Qualified,
            None,
            RuntimeQualification::Qualified,
        )
    );
}

#[test]
fn a_profile_without_tool_evidence_keeps_its_stored_form_and_the_probes_set_the_verdicts() {
    let tools = |profile: &ModelCapabilityProfile| {
        serde_json::to_value(profile).unwrap()["entries"]
            .get("tools")
            .cloned()
    };

    let kinds = |profile: &ModelCapabilityProfile| {
        tools(profile).map(|tools| {
            tools
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, entry)| (name.clone(), entry["kind"].as_str().unwrap().to_owned()))
                .collect::<Vec<_>>()
        })
    };
    let entries = |kinds: [&str; 4]| {
        Some(
            [
                "function_calling",
                "hosted_web_search",
                "max_tool_calls_bound",
                "remote_mcp",
            ]
            .into_iter()
            .zip(kinds)
            .map(|(name, kind)| (name.to_owned(), kind.to_owned()))
            .collect::<Vec<_>>(),
        )
    };

    assert_eq!(
        (
            tools(AGENT_PLAN_PENDING_REGISTRATION.capability_profile),
            tools(&AGENT_PLAN_CAPABILITY_PROFILE),
            kinds(&AGENT_PLAN_TOOL_PENDING_CAPABILITY_PROFILE),
            kinds(&AGENT_PLAN_TOOL_CAPABILITY_PROFILE),
        ),
        (
            None,
            None,
            entries(["documented"; 4]),
            entries(["qualified", "qualified", "unsupported", "unsupported"]),
        )
    );
}

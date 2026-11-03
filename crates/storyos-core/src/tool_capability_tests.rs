use super::*;
use crate::{AGENT_PLAN_PENDING_REGISTRATION, DeploymentDestination, HOST_FAKE_REGISTRATION};

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
fn the_link_authorizes_a_qualified_registration_only_while_a_tool_entry_is_pending() {
    let authorized = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: Some(AUTHORIZATION.to_owned()),
    };
    let unauthorized = DeploymentDestination::VolcengineAgentPlan {
        qualification_authorization: None,
    };

    assert_eq!(
        (
            authorized.registration().qualified(),
            authorized.runtime_qualification(),
            authorized.qualification_authorization(),
            unauthorized.runtime_qualification(),
            unauthorized.qualification_authorization(),
            AGENT_PLAN_REGISTRATION
                .capability_profile
                .entries
                .tools
                .any_pending(),
        ),
        (
            true,
            RuntimeQualification::QualificationAuthorized,
            Some(AUTHORIZATION),
            RuntimeQualification::Qualified,
            None,
            false,
        )
    );
}

#[test]
fn a_profile_without_tool_evidence_keeps_its_stored_form() {
    let tools = |profile: &ModelCapabilityProfile| {
        serde_json::to_value(profile).unwrap()["entries"]
            .get("tools")
            .cloned()
    };

    assert_eq!(
        (
            tools(AGENT_PLAN_PENDING_REGISTRATION.capability_profile),
            tools(&AGENT_PLAN_CAPABILITY_PROFILE),
            tools(&AGENT_PLAN_TOOL_PENDING_CAPABILITY_PROFILE).map(|tools| tools
                .as_object()
                .unwrap()
                .values()
                .map(|entry| entry["kind"].clone())
                .collect::<Vec<_>>()),
        ),
        (None, None, Some(vec![serde_json::json!("documented"); 4]),)
    );
}

use super::{RouteFacts, admit_qualified, admit_route};
use crate::shared_tests::{UNQUALIFIED_PROFILE, targeted_passage_assembly};
use crate::{
    AGENT_PLAN_REGISTRATION, BudgetBounds, GrantFacts, HOST_FAKE_TOOL_LOOP_REGISTRATION,
    RuntimeQualification, assemble_current_passage_context,
};

/// A current-passage request that the standing grant scope covers.
fn granted() -> GrantFacts<'static> {
    let request = Box::leak(Box::new(assemble_current_passage_context(
        &targeted_passage_assembly("Help with this passage.", "The tide turned."),
    )));
    GrantFacts {
        owner_user_id: "user-1",
        project_id: "proj-1",
        grant_scope: Some("project_writing_assistance_v1"),
        grant_identity: Some("dest-1"),
        binding_identity: Some("dest-1"),
        request: Some(request),
    }
}

fn route(
    revision: &'static str,
    qualification: RuntimeQualification,
    budget_bounds: BudgetBounds,
) -> RouteFacts<'static> {
    RouteFacts {
        model_registration_revision: revision,
        binding_current: true,
        registration_current: true,
        qualification,
        budget_bounds,
        grant: granted(),
    }
}

#[test]
fn a_qualified_profile_or_an_authorized_qualification_with_known_bounds_admits_create() {
    // An unqualified profile, as of a later provider version before its qualification.
    let unqualified = |qualification, budget_bounds| {
        admit_qualified(
            &UNQUALIFIED_PROFILE,
            AGENT_PLAN_REGISTRATION.requirement,
            &route(
                AGENT_PLAN_REGISTRATION.revision,
                qualification,
                budget_bounds,
            ),
        )
    };

    assert_eq!(
        [
            admit_route(&route(
                HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
                RuntimeQualification::Qualified,
                BudgetBounds::NotRequired,
            )),
            unqualified(
                RuntimeQualification::Pending,
                BudgetBounds::AgentPlanRequestV1
            ),
            unqualified(
                RuntimeQualification::QualificationAuthorized,
                BudgetBounds::AgentPlanRequestV1,
            ),
            unqualified(
                RuntimeQualification::QualificationAuthorized,
                BudgetBounds::Unqualified,
            ),
            admit_route(&RouteFacts {
                grant: GrantFacts {
                    grant_scope: None,
                    ..granted()
                },
                ..route(
                    AGENT_PLAN_REGISTRATION.revision,
                    RuntimeQualification::Qualified,
                    BudgetBounds::AgentPlanRequestV1,
                )
            }),
            admit_route(&route(
                AGENT_PLAN_REGISTRATION.revision,
                RuntimeQualification::Qualified,
                BudgetBounds::AgentPlanRequestV1,
            )),
            admit_route(&route(
                "018f0000-0000-7001-8000-00000000fa02",
                RuntimeQualification::QualificationAuthorized,
                BudgetBounds::AgentPlanRequestV1,
            )),
            admit_route(&RouteFacts {
                binding_current: false,
                ..route(
                    AGENT_PLAN_REGISTRATION.revision,
                    RuntimeQualification::Qualified,
                    BudgetBounds::AgentPlanRequestV1,
                )
            }),
        ],
        [
            Ok(()),
            Err("model_runtime_qualification_pending"),
            Ok(()),
            Err("model_request_bounds_unknown"),
            Err("disclosure_outside_grant"),
            Ok(()),
            Err("model_registration_unknown"),
            Err("model_use_binding_stale"),
        ]
    );
}

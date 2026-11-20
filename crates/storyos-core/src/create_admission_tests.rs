use super::{RouteFacts, admit_registration, admit_route};
use crate::shared_tests::targeted_passage_assembly;
use crate::{
    AGENT_PLAN_CAPABILITY_PROFILE, AGENT_PLAN_REGISTRATION, BudgetBounds, GrantFacts,
    HOST_FAKE_TOOL_LOOP_REGISTRATION, ModelCapabilityProfile, ModelRegistration,
    RuntimeQualification, assemble_current_passage_context,
};

/// A profile of the test with the current entries and no qualified combination.
const PENDING_PROFILE: ModelCapabilityProfile = ModelCapabilityProfile {
    revision: "test.pending",
    combinations: &[],
    ..AGENT_PLAN_CAPABILITY_PROFILE
};

/// A Registration of the test that awaits qualification: a later version before its probe.
const PENDING: ModelRegistration = ModelRegistration {
    revision: "test.pending",
    capability_profile: &PENDING_PROFILE,
    ..AGENT_PLAN_REGISTRATION
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
    let pending = |qualification, budget_bounds| {
        admit_registration(
            &PENDING,
            &route(PENDING.revision, qualification, budget_bounds),
        )
    };

    assert_eq!(
        [
            admit_route(&route(
                HOST_FAKE_TOOL_LOOP_REGISTRATION.revision,
                RuntimeQualification::Qualified,
                BudgetBounds::NotRequired,
            )),
            pending(
                RuntimeQualification::Pending,
                BudgetBounds::AgentPlanRequestV1
            ),
            pending(
                RuntimeQualification::QualificationAuthorized,
                BudgetBounds::AgentPlanRequestV1,
            ),
            pending(
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
                PENDING.revision,
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

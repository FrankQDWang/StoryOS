use super::{RouteFacts, admit_route};
use crate::{AGENT_PLAN_REGISTRATION, BudgetBounds, HOST_FAKE_REGISTRATION, RuntimeQualification};

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
    }
}

#[test]
fn only_an_authorized_qualification_with_known_bounds_admits_a_pending_profile() {
    let agent_plan = AGENT_PLAN_REGISTRATION.revision;

    assert_eq!(
        [
            admit_route(&route(
                HOST_FAKE_REGISTRATION.revision,
                RuntimeQualification::Qualified,
                BudgetBounds::NotRequired,
            )),
            admit_route(&route(
                agent_plan,
                RuntimeQualification::Pending,
                BudgetBounds::AgentPlanRequestV1,
            )),
            admit_route(&route(
                agent_plan,
                RuntimeQualification::QualificationAuthorized,
                BudgetBounds::AgentPlanRequestV1,
            )),
            admit_route(&route(
                agent_plan,
                RuntimeQualification::QualificationAuthorized,
                BudgetBounds::Unqualified,
            )),
            admit_route(&RouteFacts {
                binding_current: false,
                ..route(
                    agent_plan,
                    RuntimeQualification::QualificationAuthorized,
                    BudgetBounds::AgentPlanRequestV1,
                )
            }),
        ],
        [
            Ok(()),
            Err("model_runtime_qualification_pending"),
            Ok(()),
            Err("model_request_bounds_unknown"),
            Err("model_use_binding_stale"),
        ]
    );
}

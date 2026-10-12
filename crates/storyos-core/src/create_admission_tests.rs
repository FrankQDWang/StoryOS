use super::{RouteFacts, admit_route};
use crate::{
    AGENT_PLAN_PENDING_REGISTRATION, AGENT_PLAN_REGISTRATION, BudgetBounds, CurrentPassageAssembly,
    GrantFacts, HOST_FAKE_REGISTRATION, InstructionBindingInput, RuntimeQualification,
    assemble_current_passage_context,
};

/// A current-passage request that the standing grant scope covers.
fn granted() -> GrantFacts<'static> {
    let request = Box::leak(Box::new(assemble_current_passage_context(
        &CurrentPassageAssembly {
            operation_requirement_id: "req-1".to_owned(),
            input_snapshot_id: "snap-1".to_owned(),
            run_id: "run-1".to_owned(),
            owner_user_id: "user-1".to_owned(),
            project_id: "proj-1".to_owned(),
            author_message: "Help with this passage.".to_owned(),
            chapter_id: "ch-1".to_owned(),
            chapter_revision_id: Some("rev-1".to_owned()),
            proposal_target_block_ids: Some(vec!["block-1".to_owned()]),
            chapter_body: "The tide turned.".to_owned(),
            instruction: InstructionBindingInput::Absent,
            destination_identity: "dest-1".to_owned(),
        },
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
    let agent_plan = AGENT_PLAN_PENDING_REGISTRATION.revision;

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
            admit_route(&route(
                AGENT_PLAN_REGISTRATION.revision,
                RuntimeQualification::Qualified,
                BudgetBounds::AgentPlanRequestV1,
            )),
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
            Ok(()),
            Err("disclosure_outside_grant"),
            Err("model_use_binding_stale"),
        ]
    );
}

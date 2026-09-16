use super::{
    AssistanceAvailability, AssistanceBindingPresence, ProjectPresence, UpdateProjectAssistance,
    UpdateProjectAssistanceConflict, UpdateProjectAssistanceNoEffect,
    UpdateProjectAssistanceRefusal, UpdateProjectAssistanceResult, update_project_assistance,
};

fn initialize() -> UpdateProjectAssistance {
    UpdateProjectAssistance {
        presence: ProjectPresence::Present,
        binding: AssistanceBindingPresence::Uninitialized,
        expected_revision: 0,
        requested: AssistanceAvailability::Available,
    }
}

fn initialized(availability: AssistanceAvailability, revision: u64) -> UpdateProjectAssistance {
    UpdateProjectAssistance {
        presence: ProjectPresence::Present,
        binding: AssistanceBindingPresence::Initialized {
            availability,
            revision,
        },
        expected_revision: revision,
        requested: AssistanceAvailability::Unavailable,
    }
}

#[test]
fn a_first_prepare_classifies_as_initialized() {
    assert_eq!(
        update_project_assistance(&initialize()),
        UpdateProjectAssistanceResult::Initialized {
            availability: AssistanceAvailability::Available,
            revision: 1,
        }
    );
}

#[test]
fn a_repeated_prepare_with_the_same_availability_is_no_effect() {
    let mut repeat = initialized(AssistanceAvailability::Available, 1);
    repeat.requested = AssistanceAvailability::Available;
    assert_eq!(
        update_project_assistance(&repeat),
        UpdateProjectAssistanceResult::NoEffect {
            reason: UpdateProjectAssistanceNoEffect::AvailabilityUnchanged,
        }
    );
}

#[test]
fn a_matching_revision_and_new_availability_classifies_as_applied() {
    assert_eq!(
        update_project_assistance(&initialized(AssistanceAvailability::Available, 1)),
        UpdateProjectAssistanceResult::Applied {
            availability: AssistanceAvailability::Unavailable,
            revision: 2,
        }
    );
}

#[test]
fn a_stale_revision_classifies_as_conflicted() {
    let mut stale = initialized(AssistanceAvailability::Available, 2);
    stale.expected_revision = 1;
    assert_eq!(
        update_project_assistance(&stale),
        UpdateProjectAssistanceResult::Conflicted {
            reason: UpdateProjectAssistanceConflict::StaleAssistanceRevision,
        }
    );
}

#[test]
fn a_nonzero_expected_revision_before_initialization_is_stale() {
    let mut stale = initialize();
    stale.expected_revision = 1;
    assert_eq!(
        update_project_assistance(&stale),
        UpdateProjectAssistanceResult::Conflicted {
            reason: UpdateProjectAssistanceConflict::StaleAssistanceRevision,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused() {
    let mut missing = initialize();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        update_project_assistance(&missing),
        UpdateProjectAssistanceResult::Refused {
            reason: UpdateProjectAssistanceRefusal::MissingProject,
        }
    );
}

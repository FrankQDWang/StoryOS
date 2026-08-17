use storyos_application::{
    ApplyAuthorEditOutcome as ApplicationOutcome,
    ApplyAuthorEditUnknownObservation as ApplicationUnknownObservation, ProjectId, ProjectScope,
    UserId,
};

use super::*;

#[test]
fn admitted_unsettled_outcome_requires_visible_reconciliation() {
    let scope = ProjectScope::new(
        UserId::new("018f0000-0000-7001-8000-000000000001"),
        ProjectId::new("018f0000-0000-7001-8000-000000000002"),
    );
    let outcome = contract_outcome(
        &scope,
        ApplicationOutcome::StillUnknown {
            observation: ApplicationUnknownObservation::AdmissionCommitted {
                command_id: "018f0000-0000-7001-8000-000000000031".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-000000000032".to_owned(),
            },
        },
    );
    let Ok(outcome) = outcome else {
        panic!("an admitted unsettled command must remain inspectably unknown")
    };

    assert_eq!(
        outcome,
        contracts::ApplyAuthorEditOutcome::StillUnknown {
            observation: contracts::ApplyAuthorEditUnknownObservation::AdmissionCommitted {
                command_id: "018f0000-0000-7001-8000-000000000031".to_owned(),
                author_command_admission_id: "018f0000-0000-7001-8000-000000000032".to_owned(),
                reconciliation_required: contracts::ReconciliationRequired,
            },
        }
    );
}

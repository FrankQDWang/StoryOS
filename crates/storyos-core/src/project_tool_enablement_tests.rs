use super::{PinnedEnablement, ProjectToolState};
use crate::ToolRegistrationLifecycle;

#[test]
fn an_offered_tool_state_derives_from_the_enablement_and_the_pinned_lifecycle() {
    let cases = [
        None,
        Some(PinnedEnablement::Disabled),
        Some(PinnedEnablement::Enabled(ToolRegistrationLifecycle::Active)),
        Some(PinnedEnablement::Enabled(
            ToolRegistrationLifecycle::Quarantined,
        )),
        Some(PinnedEnablement::Enabled(
            ToolRegistrationLifecycle::Retired,
        )),
    ];
    assert_eq!(
        cases.map(|current| ProjectToolState::offered(ToolRegistrationLifecycle::Active, current)),
        [
            ProjectToolState::Disabled,
            ProjectToolState::Disabled,
            ProjectToolState::Enabled,
            ProjectToolState::Quarantined,
            ProjectToolState::Quarantined,
        ]
    );
}

#[test]
fn a_quarantined_or_retired_offered_revision_is_quarantined_for_each_enablement() {
    for offered in [
        ToolRegistrationLifecycle::Quarantined,
        ToolRegistrationLifecycle::Retired,
    ] {
        for current in [
            None,
            Some(PinnedEnablement::Disabled),
            Some(PinnedEnablement::Enabled(ToolRegistrationLifecycle::Active)),
        ] {
            assert_eq!(
                ProjectToolState::offered(offered, current),
                ProjectToolState::Quarantined
            );
        }
    }
}

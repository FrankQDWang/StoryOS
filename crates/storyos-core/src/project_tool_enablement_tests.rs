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
        cases.map(ProjectToolState::offered),
        [
            ProjectToolState::Disabled,
            ProjectToolState::Disabled,
            ProjectToolState::Enabled,
            ProjectToolState::Quarantined,
            ProjectToolState::Quarantined,
        ]
    );
}

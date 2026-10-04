use super::{
    CreateVolume, CreateVolumeApplied, CreateVolumeConflict, CreateVolumeRefusal,
    CreateVolumeResult, ProjectLifecycle, create_volume,
};

fn command() -> CreateVolume {
    CreateVolume {
        expected_tree_revision: 1,
        current_tree_revision: 1,
        current_lifecycle: ProjectLifecycle::Active,
        title: "Volume A".to_owned(),
    }
}

#[test]
fn a_matching_tree_revision_and_active_project_classifies_as_applied() {
    assert_eq!(
        create_volume(&command()),
        CreateVolumeResult::Applied(CreateVolumeApplied { tree_revision: 2 })
    );
}

#[test]
fn a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_tree_revision = 1;
    stale.current_tree_revision = 2;
    assert_eq!(
        create_volume(&stale),
        CreateVolumeResult::Conflicted(CreateVolumeConflict::StaleTreeRevision)
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        create_volume(&archived),
        CreateVolumeResult::Refused(CreateVolumeRefusal::ArchivedProject)
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        create_volume(&empty),
        CreateVolumeResult::Refused(CreateVolumeRefusal::InvalidTitle)
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        create_volume(&too_long),
        CreateVolumeResult::Refused(CreateVolumeRefusal::InvalidTitle)
    );
}

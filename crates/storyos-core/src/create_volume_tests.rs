use super::{
    CreateVolume, CreateVolumeConflict, CreateVolumeRefusal, CreateVolumeResult, ProjectLifecycle,
    ProjectPresence, create_volume,
};

fn command() -> CreateVolume {
    CreateVolume {
        presence: ProjectPresence::Present,
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
        CreateVolumeResult::Applied { tree_revision: 2 }
    );
}

#[test]
fn a_stale_tree_revision_classifies_as_conflicted_with_zero_authority_effect() {
    let mut stale = command();
    stale.expected_tree_revision = 1;
    stale.current_tree_revision = 2;
    assert_eq!(
        create_volume(&stale),
        CreateVolumeResult::Conflicted {
            reason: CreateVolumeConflict::StaleTreeRevision,
        }
    );
}

#[test]
fn an_archived_project_classifies_as_refused_with_zero_authority_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        create_volume(&archived),
        CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::ArchivedProject,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused_with_zero_authority_effect() {
    let mut missing = command();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        create_volume(&missing),
        CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::MissingProject,
        }
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_authority_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        create_volume(&empty),
        CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::InvalidTitle,
        }
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        create_volume(&too_long),
        CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::InvalidTitle,
        }
    );
}

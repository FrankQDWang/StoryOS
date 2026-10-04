use super::{
    ArchiveProject, ArchiveProjectApplied, ArchiveProjectConflict, ArchiveProjectNoEffect,
    ArchiveProjectResult, ProjectLifecycle, archive_project,
};

fn command() -> ArchiveProject {
    ArchiveProject {
        expected_revision: 1,
        current_revision: 1,
        current_lifecycle: ProjectLifecycle::Active,
    }
}

#[test]
fn a_matching_revision_and_active_lifecycle_classifies_as_applied() {
    assert_eq!(
        archive_project(&command()),
        ArchiveProjectResult::Applied(ArchiveProjectApplied { revision: 2 })
    );
}

#[test]
fn a_stale_revision_classifies_as_conflicted_with_zero_lifecycle_effect() {
    let mut stale = command();
    stale.expected_revision = 1;
    stale.current_revision = 2;
    assert_eq!(
        archive_project(&stale),
        ArchiveProjectResult::Conflicted(ArchiveProjectConflict::StaleProjectRevision)
    );
}

#[test]
fn an_already_archived_project_classifies_as_no_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        archive_project(&archived),
        ArchiveProjectResult::NoEffect(ArchiveProjectNoEffect::AlreadyArchived)
    );
}

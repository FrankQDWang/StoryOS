use super::{
    ArchiveProject, ArchiveProjectConflict, ArchiveProjectNoEffect, ArchiveProjectRefusal,
    ArchiveProjectResult, ProjectLifecycle, ProjectPresence, archive_project,
};

fn command() -> ArchiveProject {
    ArchiveProject {
        presence: ProjectPresence::Present,
        expected_revision: 1,
        current_revision: 1,
        current_lifecycle: ProjectLifecycle::Active,
    }
}

#[test]
fn a_matching_revision_and_active_lifecycle_classifies_as_applied() {
    assert_eq!(
        archive_project(&command()),
        ArchiveProjectResult::Applied { revision: 2 }
    );
}

#[test]
fn a_stale_revision_classifies_as_conflicted_with_zero_lifecycle_effect() {
    let mut stale = command();
    stale.expected_revision = 1;
    stale.current_revision = 2;
    assert_eq!(
        archive_project(&stale),
        ArchiveProjectResult::Conflicted {
            reason: ArchiveProjectConflict::StaleProjectRevision,
        }
    );
}

#[test]
fn an_already_archived_project_classifies_as_no_effect() {
    let mut archived = command();
    archived.current_lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        archive_project(&archived),
        ArchiveProjectResult::NoEffect {
            reason: ArchiveProjectNoEffect::AlreadyArchived,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused_with_zero_lifecycle_effect() {
    let mut missing = command();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        archive_project(&missing),
        ArchiveProjectResult::Refused {
            reason: ArchiveProjectRefusal::MissingProject,
        }
    );
}

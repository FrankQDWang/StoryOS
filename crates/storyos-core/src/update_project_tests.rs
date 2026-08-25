use super::{
    ProjectPresence, UpdateProject, UpdateProjectConflict, UpdateProjectNoEffect,
    UpdateProjectRefusal, UpdateProjectResult, update_project,
};

fn command() -> UpdateProject {
    UpdateProject {
        presence: ProjectPresence::Present,
        expected_revision: 1,
        current_revision: 1,
        title: "Renamed Novel".to_owned(),
        current_title: "Empty Novel".to_owned(),
    }
}

#[test]
fn a_matching_revision_and_new_title_classifies_as_applied() {
    assert_eq!(
        update_project(&command()),
        UpdateProjectResult::Applied {
            title: "Renamed Novel".to_owned(),
            revision: 2,
        }
    );
}

#[test]
fn a_stale_revision_classifies_as_conflicted_with_zero_title_effect() {
    let mut stale = command();
    stale.expected_revision = 1;
    stale.current_revision = 2;
    assert_eq!(
        update_project(&stale),
        UpdateProjectResult::Conflicted {
            reason: UpdateProjectConflict::StaleProjectRevision,
        }
    );
}

#[test]
fn an_unchanged_title_classifies_as_no_effect() {
    let mut unchanged = command();
    unchanged.title = unchanged.current_title.clone();
    assert_eq!(
        update_project(&unchanged),
        UpdateProjectResult::NoEffect {
            reason: UpdateProjectNoEffect::TitleUnchanged,
        }
    );
}

#[test]
fn an_invalid_title_classifies_as_refused_with_zero_title_effect() {
    let mut empty = command();
    empty.title.clear();
    assert_eq!(
        update_project(&empty),
        UpdateProjectResult::Refused {
            reason: UpdateProjectRefusal::InvalidTitle,
        }
    );
    let mut too_long = command();
    too_long.title = "n".repeat(1025);
    assert_eq!(
        update_project(&too_long),
        UpdateProjectResult::Refused {
            reason: UpdateProjectRefusal::InvalidTitle,
        }
    );
}

#[test]
fn a_missing_project_classifies_as_refused_with_zero_title_effect() {
    let mut missing = command();
    missing.presence = ProjectPresence::Absent;
    assert_eq!(
        update_project(&missing),
        UpdateProjectResult::Refused {
            reason: UpdateProjectRefusal::MissingProject,
        }
    );
}

use super::{
    UpdateProject, UpdateProjectApplied, UpdateProjectConflict, UpdateProjectNoEffect,
    UpdateProjectResult, update_project,
};

fn command() -> UpdateProject {
    UpdateProject {
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
        UpdateProjectResult::Applied(UpdateProjectApplied {
            title: "Renamed Novel".to_owned(),
            revision: 2,
        })
    );
}

#[test]
fn a_stale_revision_classifies_as_conflicted_with_zero_title_effect() {
    let mut stale = command();
    stale.expected_revision = 1;
    stale.current_revision = 2;
    assert_eq!(
        update_project(&stale),
        UpdateProjectResult::Conflicted(UpdateProjectConflict::StaleProjectRevision)
    );
}

#[test]
fn an_unchanged_title_classifies_as_no_effect() {
    let mut unchanged = command();
    unchanged.title = unchanged.current_title.clone();
    assert_eq!(
        update_project(&unchanged),
        UpdateProjectResult::NoEffect(UpdateProjectNoEffect::TitleUnchanged)
    );
}

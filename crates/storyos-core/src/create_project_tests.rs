use super::*;

#[test]
fn an_absent_project_classifies_as_the_closed_empty_open_state() {
    assert_eq!(
        create_project(/*project_exists*/ false),
        CreateProjectResult::Empty
    );
}

#[test]
fn an_existing_project_classifies_as_a_closed_refusal() {
    assert_eq!(
        create_project(/*project_exists*/ true),
        CreateProjectResult::ExistingProject
    );
}

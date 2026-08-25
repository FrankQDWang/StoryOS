//! Pure Core classification for Create Project.

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateProjectResult {
    Empty,
    ExistingProject,
}

/// Classify one Create Project against the expected-absent Project precondition.
pub fn create_project(project_exists: bool) -> CreateProjectResult {
    if project_exists {
        CreateProjectResult::ExistingProject
    } else {
        CreateProjectResult::Empty
    }
}

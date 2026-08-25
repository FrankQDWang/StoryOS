//! Pure Core classification for Create Project.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectPresence {
    Absent,
    Present,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateProjectResult {
    Empty,
    ExistingProject,
}

/// Classify one Create Project against the expected-absent Project precondition.
pub fn create_project(presence: ProjectPresence) -> CreateProjectResult {
    match presence {
        ProjectPresence::Present => CreateProjectResult::ExistingProject,
        ProjectPresence::Absent => CreateProjectResult::Empty,
    }
}

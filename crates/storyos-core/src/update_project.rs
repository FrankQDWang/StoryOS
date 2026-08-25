//! Pure Core classification for Update Project (rename).

use super::ProjectPresence;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProject {
    pub presence: ProjectPresence,
    pub expected_revision: u64,
    pub current_revision: u64,
    pub title: String,
    pub current_title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectResult {
    Applied { title: String, revision: u64 },
    NoEffect { reason: UpdateProjectNoEffect },
    Conflicted { reason: UpdateProjectConflict },
    Refused { reason: UpdateProjectRefusal },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectNoEffect {
    TitleUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectConflict {
    StaleProjectRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectRefusal {
    MissingProject,
    InvalidTitle,
}

/// Classify one Update Project against exact Scope presence, expected revision, and title.
pub fn update_project(command: &UpdateProject) -> UpdateProjectResult {
    if command.presence == ProjectPresence::Absent {
        return UpdateProjectResult::Refused {
            reason: UpdateProjectRefusal::MissingProject,
        };
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return UpdateProjectResult::Refused {
            reason: UpdateProjectRefusal::InvalidTitle,
        };
    }
    if command.expected_revision != command.current_revision {
        return UpdateProjectResult::Conflicted {
            reason: UpdateProjectConflict::StaleProjectRevision,
        };
    }
    if command.title == command.current_title {
        return UpdateProjectResult::NoEffect {
            reason: UpdateProjectNoEffect::TitleUnchanged,
        };
    }
    UpdateProjectResult::Applied {
        title: command.title.clone(),
        revision: command.current_revision + 1,
    }
}

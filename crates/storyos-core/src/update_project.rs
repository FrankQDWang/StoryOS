//! Pure Core classification for Update Project (rename).

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProject {
    pub expected_revision: u64,
    pub current_revision: u64,
    pub title: String,
    pub current_title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateProjectApplied {
    pub title: String,
    pub revision: u64,
}

pub type UpdateProjectResult = TransitionOutcome<
    UpdateProjectApplied,
    UpdateProjectNoEffect,
    UpdateProjectConflict,
    UpdateProjectRefusal,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectNoEffect {
    TitleUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectConflict {
    StaleProjectRevision,
}

/// A refusal that the adapter returns before Admission; no Receipt records it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateProjectRefusal {
    InvalidTitle,
}

reason_codes!(UpdateProjectNoEffect { TitleUnchanged => "title_unchanged" });
reason_codes!(UpdateProjectConflict { StaleProjectRevision => "stale_project_revision" });

/// Classify one Update Project against expected revision and title.
pub fn update_project(command: &UpdateProject) -> UpdateProjectResult {
    if command.title.is_empty() || command.title.len() > 1024 {
        return UpdateProjectResult::Refused(UpdateProjectRefusal::InvalidTitle);
    }
    if command.expected_revision != command.current_revision {
        return UpdateProjectResult::Conflicted(UpdateProjectConflict::StaleProjectRevision);
    }
    if command.title == command.current_title {
        return UpdateProjectResult::NoEffect(UpdateProjectNoEffect::TitleUnchanged);
    }
    UpdateProjectResult::Applied(UpdateProjectApplied {
        title: command.title.clone(),
        revision: command.current_revision + 1,
    })
}

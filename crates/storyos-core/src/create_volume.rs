//! Pure Core classification for Create Volume.

use super::{ProjectLifecycle, TransitionOutcome};
use crate::transition_outcome::reason_codes;
use std::convert::Infallible;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateVolume {
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateVolumeApplied {
    pub tree_revision: u64,
}

pub type CreateVolumeResult =
    TransitionOutcome<CreateVolumeApplied, Infallible, CreateVolumeConflict, CreateVolumeRefusal>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateVolumeConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateVolumeRefusal {
    ArchivedProject,
    InvalidTitle,
}

reason_codes!(CreateVolumeConflict {
    StaleTreeRevision => "stale_tree_revision",
});

reason_codes!(CreateVolumeRefusal {
    ArchivedProject => "archived_project",
    InvalidTitle => "invalid_title",
});

/// Classify one Create Volume against lifecycle, expected tree revision, and title.
pub fn create_volume(command: &CreateVolume) -> CreateVolumeResult {
    if command.title.is_empty() || command.title.len() > 1024 {
        return CreateVolumeResult::Refused(CreateVolumeRefusal::InvalidTitle);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return CreateVolumeResult::Refused(CreateVolumeRefusal::ArchivedProject);
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return CreateVolumeResult::Conflicted(CreateVolumeConflict::StaleTreeRevision);
    }
    CreateVolumeResult::Applied(CreateVolumeApplied {
        tree_revision: command.current_tree_revision + 1,
    })
}

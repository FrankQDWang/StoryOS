//! Pure Core classification for Create Volume.

use super::{ProjectLifecycle, ProjectPresence};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateVolume {
    pub presence: ProjectPresence,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateVolumeResult {
    Applied { tree_revision: u64 },
    Conflicted { reason: CreateVolumeConflict },
    Refused { reason: CreateVolumeRefusal },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateVolumeConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateVolumeRefusal {
    MissingProject,
    ArchivedProject,
    InvalidTitle,
}

/// Classify one Create Volume against exact Scope presence, lifecycle, expected tree revision, and title.
pub fn create_volume(command: &CreateVolume) -> CreateVolumeResult {
    if command.presence == ProjectPresence::Absent {
        return CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::MissingProject,
        };
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::InvalidTitle,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return CreateVolumeResult::Refused {
            reason: CreateVolumeRefusal::ArchivedProject,
        };
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return CreateVolumeResult::Conflicted {
            reason: CreateVolumeConflict::StaleTreeRevision,
        };
    }
    CreateVolumeResult::Applied {
        tree_revision: command.current_tree_revision + 1,
    }
}

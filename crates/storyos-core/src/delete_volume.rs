//! Pure Core classification for author-initiated Volume removal.

use super::{ProjectLifecycle, ProjectPresence, VolumeJoin};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VolumeRemovalLifecycle {
    Active,
    Removed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VolumeChildPolicy {
    Empty,
    Nonempty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteVolume {
    pub presence: ProjectPresence,
    pub volume_join: VolumeJoin,
    pub volume_lifecycle: VolumeRemovalLifecycle,
    pub child_chapters: VolumeChildPolicy,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteVolumeResult {
    Applied { tree_revision: u64 },
    NoEffect { reason: DeleteVolumeNoEffect },
    Conflicted { reason: DeleteVolumeConflict },
    Refused { reason: DeleteVolumeRefusal },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteVolumeNoEffect {
    AlreadyRemoved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteVolumeConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteVolumeRefusal {
    MissingProject,
    InvalidVolumeJoin,
    ArchivedProject,
    NonemptyVolume,
}

/// Classify one Volume removal against exact Scope, join, revision, and active child Chapters.
pub fn delete_volume(command: &DeleteVolume) -> DeleteVolumeResult {
    if command.presence == ProjectPresence::Absent {
        return DeleteVolumeResult::Refused {
            reason: DeleteVolumeRefusal::MissingProject,
        };
    }
    if command.volume_join == VolumeJoin::Invalid {
        return DeleteVolumeResult::Refused {
            reason: DeleteVolumeRefusal::InvalidVolumeJoin,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return DeleteVolumeResult::Refused {
            reason: DeleteVolumeRefusal::ArchivedProject,
        };
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return DeleteVolumeResult::Conflicted {
            reason: DeleteVolumeConflict::StaleTreeRevision,
        };
    }
    if command.volume_lifecycle == VolumeRemovalLifecycle::Removed {
        return DeleteVolumeResult::NoEffect {
            reason: DeleteVolumeNoEffect::AlreadyRemoved,
        };
    }
    if command.child_chapters == VolumeChildPolicy::Nonempty {
        return DeleteVolumeResult::Refused {
            reason: DeleteVolumeRefusal::NonemptyVolume,
        };
    }
    DeleteVolumeResult::Applied {
        tree_revision: command.current_tree_revision + 1,
    }
}

#[cfg(test)]
#[path = "delete_volume_tests.rs"]
mod tests;

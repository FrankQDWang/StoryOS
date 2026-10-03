//! Pure Core classification for author-initiated Volume removal.

use super::{ProjectLifecycle, TransitionOutcome, VolumeJoin};
use crate::transition_outcome::reason_codes;

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
    pub volume_join: VolumeJoin,
    pub volume_lifecycle: VolumeRemovalLifecycle,
    pub child_chapters: VolumeChildPolicy,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteVolumeApplied {
    pub tree_revision: u64,
}

pub type DeleteVolumeResult = TransitionOutcome<
    DeleteVolumeApplied,
    DeleteVolumeNoEffect,
    DeleteVolumeConflict,
    DeleteVolumeRefusal,
>;

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
    InvalidVolumeJoin,
    ArchivedProject,
    NonemptyVolume,
}

reason_codes!(DeleteVolumeNoEffect {
    AlreadyRemoved => "already_removed",
});

reason_codes!(DeleteVolumeConflict {
    StaleTreeRevision => "stale_tree_revision",
});

reason_codes!(DeleteVolumeRefusal {
    InvalidVolumeJoin => "invalid_volume_join",
    ArchivedProject => "archived_project",
    NonemptyVolume => "nonempty_volume",
});

/// Classify one Volume removal against exact Scope, join, revision, and active child Chapters.
pub fn delete_volume(command: &DeleteVolume) -> DeleteVolumeResult {
    if command.volume_join == VolumeJoin::Invalid {
        return DeleteVolumeResult::Refused(DeleteVolumeRefusal::InvalidVolumeJoin);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return DeleteVolumeResult::Refused(DeleteVolumeRefusal::ArchivedProject);
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return DeleteVolumeResult::Conflicted(DeleteVolumeConflict::StaleTreeRevision);
    }
    if command.volume_lifecycle == VolumeRemovalLifecycle::Removed {
        return DeleteVolumeResult::NoEffect(DeleteVolumeNoEffect::AlreadyRemoved);
    }
    if command.child_chapters == VolumeChildPolicy::Nonempty {
        return DeleteVolumeResult::Refused(DeleteVolumeRefusal::NonemptyVolume);
    }
    DeleteVolumeResult::Applied(DeleteVolumeApplied {
        tree_revision: command.current_tree_revision + 1,
    })
}

#[cfg(test)]
#[path = "delete_volume_tests.rs"]
mod tests;

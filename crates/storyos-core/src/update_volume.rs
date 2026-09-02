//! Pure Core classification for Update Volume (rename and reorder).

use super::{ProjectLifecycle, ProjectPresence, VolumeJoin};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateVolume {
    pub presence: ProjectPresence,
    pub volume_join: VolumeJoin,
    pub expected_tree_revision: u64,
    pub current_tree_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
    pub title: String,
    pub current_title: String,
    pub order: u64,
    pub current_order: u64,
    pub volume_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateVolumeResult {
    Applied {
        title: String,
        order: u64,
        tree_revision: u64,
    },
    NoEffect {
        reason: UpdateVolumeNoEffect,
    },
    Conflicted {
        reason: UpdateVolumeConflict,
    },
    Refused {
        reason: UpdateVolumeRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateVolumeNoEffect {
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateVolumeConflict {
    StaleTreeRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateVolumeRefusal {
    MissingProject,
    InvalidVolumeJoin,
    ArchivedProject,
    InvalidTitle,
    InvalidOrder,
}

/// Classify one Update Volume against exact Scope, Volume join, expected revision, title, and order.
pub fn update_volume(command: &UpdateVolume) -> UpdateVolumeResult {
    if command.presence == ProjectPresence::Absent {
        return UpdateVolumeResult::Refused {
            reason: UpdateVolumeRefusal::MissingProject,
        };
    }
    if command.volume_join == VolumeJoin::Invalid {
        return UpdateVolumeResult::Refused {
            reason: UpdateVolumeRefusal::InvalidVolumeJoin,
        };
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return UpdateVolumeResult::Refused {
            reason: UpdateVolumeRefusal::InvalidTitle,
        };
    }
    if command.order < 1 || command.order > command.volume_count {
        return UpdateVolumeResult::Refused {
            reason: UpdateVolumeRefusal::InvalidOrder,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return UpdateVolumeResult::Refused {
            reason: UpdateVolumeRefusal::ArchivedProject,
        };
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return UpdateVolumeResult::Conflicted {
            reason: UpdateVolumeConflict::StaleTreeRevision,
        };
    }
    if command.title == command.current_title && command.order == command.current_order {
        return UpdateVolumeResult::NoEffect {
            reason: UpdateVolumeNoEffect::Unchanged,
        };
    }
    UpdateVolumeResult::Applied {
        title: command.title.clone(),
        order: command.order,
        tree_revision: command.current_tree_revision + 1,
    }
}

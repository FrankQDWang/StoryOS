//! Pure Core classification for Update Volume (rename and reorder).

use super::{ProjectLifecycle, TransitionOutcome, VolumeJoin};
use crate::transition_outcome::reason_codes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateVolume {
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
pub struct UpdateVolumeApplied {
    pub title: String,
    pub order: u64,
    pub tree_revision: u64,
}

pub type UpdateVolumeResult = TransitionOutcome<
    UpdateVolumeApplied,
    UpdateVolumeNoEffect,
    UpdateVolumeConflict,
    UpdateVolumeRefusal,
>;

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
    InvalidVolumeJoin,
    ArchivedProject,
    InvalidTitle,
    InvalidOrder,
}

reason_codes!(UpdateVolumeNoEffect { Unchanged => "unchanged" });
reason_codes!(UpdateVolumeConflict { StaleTreeRevision => "stale_tree_revision" });
reason_codes!(UpdateVolumeRefusal {
    InvalidVolumeJoin => "invalid_volume_join",
    ArchivedProject => "archived_project",
    InvalidTitle => "invalid_title",
    InvalidOrder => "invalid_order",
});

/// Classify one Update Volume against exact Scope, Volume join, expected revision, title, and order.
pub fn update_volume(command: &UpdateVolume) -> UpdateVolumeResult {
    if command.volume_join == VolumeJoin::Invalid {
        return UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidVolumeJoin);
    }
    if command.title.is_empty() || command.title.len() > 1024 {
        return UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidTitle);
    }
    if command.order < 1 || command.order > command.volume_count {
        return UpdateVolumeResult::Refused(UpdateVolumeRefusal::InvalidOrder);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return UpdateVolumeResult::Refused(UpdateVolumeRefusal::ArchivedProject);
    }
    if command.expected_tree_revision != command.current_tree_revision {
        return UpdateVolumeResult::Conflicted(UpdateVolumeConflict::StaleTreeRevision);
    }
    if command.title == command.current_title && command.order == command.current_order {
        return UpdateVolumeResult::NoEffect(UpdateVolumeNoEffect::Unchanged);
    }
    UpdateVolumeResult::Applied(UpdateVolumeApplied {
        title: command.title.clone(),
        order: command.order,
        tree_revision: command.current_tree_revision + 1,
    })
}

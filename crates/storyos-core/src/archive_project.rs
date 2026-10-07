//! Pure Core classification for Archive Project.

use std::convert::Infallible;

use super::TransitionOutcome;
use crate::transition_outcome::reason_codes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectLifecycle {
    Active,
    Archived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProject {
    pub expected_revision: u64,
    pub current_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProjectApplied {
    pub revision: u64,
}

pub type ArchiveProjectResult = TransitionOutcome<
    ArchiveProjectApplied,
    ArchiveProjectNoEffect,
    ArchiveProjectConflict,
    Infallible,
>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectNoEffect {
    AlreadyArchived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectConflict {
    StaleProjectRevision,
}

reason_codes!(ArchiveProjectNoEffect { AlreadyArchived => "already_archived" });
reason_codes!(ArchiveProjectConflict { StaleProjectRevision => "stale_project_revision" });

/// Classify one Archive Project against expected revision and lifecycle.
pub fn archive_project(command: &ArchiveProject) -> ArchiveProjectResult {
    if command.expected_revision != command.current_revision {
        return ArchiveProjectResult::Conflicted(ArchiveProjectConflict::StaleProjectRevision);
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return ArchiveProjectResult::NoEffect(ArchiveProjectNoEffect::AlreadyArchived);
    }
    ArchiveProjectResult::Applied(ArchiveProjectApplied {
        revision: command.current_revision + 1,
    })
}

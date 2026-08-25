//! Pure Core classification for Archive Project.

use super::ProjectPresence;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectLifecycle {
    Active,
    Archived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveProject {
    pub presence: ProjectPresence,
    pub expected_revision: u64,
    pub current_revision: u64,
    pub current_lifecycle: ProjectLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectResult {
    Applied { revision: u64 },
    NoEffect { reason: ArchiveProjectNoEffect },
    Conflicted { reason: ArchiveProjectConflict },
    Refused { reason: ArchiveProjectRefusal },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectNoEffect {
    AlreadyArchived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectConflict {
    StaleProjectRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveProjectRefusal {
    MissingProject,
}

/// Classify one Archive Project against exact Scope presence, expected revision, and lifecycle.
pub fn archive_project(command: &ArchiveProject) -> ArchiveProjectResult {
    if command.presence == ProjectPresence::Absent {
        return ArchiveProjectResult::Refused {
            reason: ArchiveProjectRefusal::MissingProject,
        };
    }
    if command.expected_revision != command.current_revision {
        return ArchiveProjectResult::Conflicted {
            reason: ArchiveProjectConflict::StaleProjectRevision,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return ArchiveProjectResult::NoEffect {
            reason: ArchiveProjectNoEffect::AlreadyArchived,
        };
    }
    ArchiveProjectResult::Applied {
        revision: command.current_revision + 1,
    }
}

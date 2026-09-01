//! Pure Core classification for Project Export Archive admission.

use super::{ProjectLifecycle, ProjectPresence};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportProjectArchive {
    pub presence: ProjectPresence,
    pub current_lifecycle: ProjectLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportProjectArchiveResult {
    Admitted,
    Refused { reason: ExportProjectArchiveRefusal },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportProjectArchiveRefusal {
    MissingProject,
    ArchivedProject,
}

/// Classify one Project Export Archive admission against exact Scope presence and lifecycle.
pub fn export_project_archive(command: &ExportProjectArchive) -> ExportProjectArchiveResult {
    if command.presence == ProjectPresence::Absent {
        return ExportProjectArchiveResult::Refused {
            reason: ExportProjectArchiveRefusal::MissingProject,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return ExportProjectArchiveResult::Refused {
            reason: ExportProjectArchiveRefusal::ArchivedProject,
        };
    }
    ExportProjectArchiveResult::Admitted
}

#[cfg(test)]
#[path = "project_export_tests.rs"]
mod tests;

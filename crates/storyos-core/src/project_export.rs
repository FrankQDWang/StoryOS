//! Pure Core classification for Project Export Archive admission.

use super::ProjectPresence;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportProjectArchive {
    pub presence: ProjectPresence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportProjectArchiveResult {
    Admitted,
    Refused { reason: ExportProjectArchiveRefusal },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportProjectArchiveRefusal {
    MissingProject,
}

/// Classify one Project Export Archive admission against exact Scope presence.
pub fn export_project_archive(command: &ExportProjectArchive) -> ExportProjectArchiveResult {
    if command.presence == ProjectPresence::Absent {
        return ExportProjectArchiveResult::Refused {
            reason: ExportProjectArchiveRefusal::MissingProject,
        };
    }
    ExportProjectArchiveResult::Admitted
}

#[cfg(test)]
#[path = "project_export_tests.rs"]
mod tests;

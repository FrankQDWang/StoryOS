//! Pure Core classification for human-readable manuscript export.

use super::{ProjectLifecycle, ProjectPresence};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportHumanReadableManuscript {
    pub presence: ProjectPresence,
    pub current_lifecycle: ProjectLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportHumanReadableManuscriptResult {
    Applied,
    Refused {
        reason: ExportHumanReadableManuscriptRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportHumanReadableManuscriptRefusal {
    MissingProject,
    ArchivedProject,
}

/// Classify one human-readable export against exact Scope presence and lifecycle.
pub fn export_human_readable_manuscript(
    command: &ExportHumanReadableManuscript,
) -> ExportHumanReadableManuscriptResult {
    if command.presence == ProjectPresence::Absent {
        return ExportHumanReadableManuscriptResult::Refused {
            reason: ExportHumanReadableManuscriptRefusal::MissingProject,
        };
    }
    if command.current_lifecycle == ProjectLifecycle::Archived {
        return ExportHumanReadableManuscriptResult::Refused {
            reason: ExportHumanReadableManuscriptRefusal::ArchivedProject,
        };
    }
    ExportHumanReadableManuscriptResult::Applied
}

#[cfg(test)]
#[path = "readable_export_command_tests.rs"]
mod command_tests;

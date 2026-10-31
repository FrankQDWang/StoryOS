//! Pure Core classification for human-readable manuscript export admission.

use super::ProjectPresence;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportHumanReadableManuscript {
    pub presence: ProjectPresence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportHumanReadableManuscriptResult {
    Admitted,
    Refused {
        reason: ExportHumanReadableManuscriptRefusal,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportHumanReadableManuscriptRefusal {
    MissingProject,
}

/// Classify one human-readable export admission against exact Scope presence.
pub fn export_human_readable_manuscript(
    command: &ExportHumanReadableManuscript,
) -> ExportHumanReadableManuscriptResult {
    if command.presence == ProjectPresence::Absent {
        return ExportHumanReadableManuscriptResult::Refused {
            reason: ExportHumanReadableManuscriptRefusal::MissingProject,
        };
    }
    ExportHumanReadableManuscriptResult::Admitted
}

#[cfg(test)]
#[path = "readable_export_command_tests.rs"]
mod command_tests;

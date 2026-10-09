//! The settlement error of a command and the contention refusal of its first use (ADR 0043).

use storyos_application::{
    AcceptanceRefusalReason, AgentRunControlRefusal, ArchiveExportRefusal, DiagnosticField as _,
    ProjectCommandError, RefusableCommandError,
};
use storyos_core::{CreateAgentRunRefusal, ExportHumanReadableManuscriptRefusal};
use tokio_postgres::error::SqlState;

/// The settlement error of one project command.
///
/// The sequence reads the sequence error inside it to find a contention failure.
pub(crate) trait CommandError: From<ProjectCommandError> + Send {
    /// The sequence error, or `None` for a refusal of the command.
    fn sequence_error(&self) -> Option<&ProjectCommandError>;

    /// The static reason code that the command span records (ADR 0047).
    fn reason_code(&self) -> &'static str;
}

impl CommandError for ProjectCommandError {
    fn sequence_error(&self) -> Option<&ProjectCommandError> {
        Some(self)
    }

    fn reason_code(&self) -> &'static str {
        self.diagnostic()
    }
}

impl<R: RefusalCode + Send> CommandError for RefusableCommandError<R> {
    fn sequence_error(&self) -> Option<&ProjectCommandError> {
        match self {
            Self::Command(error) => Some(error),
            Self::RefusedBeforeAdmission(_) => None,
        }
    }

    fn reason_code(&self) -> &'static str {
        match self {
            Self::Command(error) => error.diagnostic(),
            Self::RefusedBeforeAdmission(refusal) => refusal.refusal_code(),
        }
    }
}

/// The static code of a refusal before Admission.
pub(crate) trait RefusalCode {
    fn refusal_code(&self) -> &'static str;
}

impl RefusalCode for CreateAgentRunRefusal {
    fn refusal_code(&self) -> &'static str {
        match self {
            Self::MissingProject => "missing_project",
            Self::ArchivedProject => "archived_project",
            Self::AssistanceUnavailable => "assistance_unavailable",
            Self::InaccessibleConversation => "inaccessible_conversation",
            Self::ConversationBusy => "conversation_busy",
            Self::InvalidChapterJoin => "invalid_chapter_join",
        }
    }
}

impl RefusalCode for AgentRunControlRefusal {
    fn refusal_code(&self) -> &'static str {
        match self {
            Self::MissingRun => "missing_run",
            Self::InputLimit => "input_limit",
        }
    }
}

impl RefusalCode for ArchiveExportRefusal {
    fn refusal_code(&self) -> &'static str {
        match self {
            Self::Lifecycle(_) => "archive_lifecycle",
            Self::ArchiveBuild(_) => "archive_build",
        }
    }
}

impl RefusalCode for ExportHumanReadableManuscriptRefusal {
    fn refusal_code(&self) -> &'static str {
        match self {
            Self::MissingProject => "missing_project",
            Self::ArchivedProject => "archived_project",
        }
    }
}

impl RefusalCode for AcceptanceRefusalReason {
    fn refusal_code(&self) -> &'static str {
        match self {
            Self::StaleWriter => "stale_writer",
            Self::SessionChanged => "session_changed",
            Self::InvalidChallenge => "invalid_challenge",
        }
    }
}

/// Whether a store fault comes from a serialization failure, a unique violation, or a deadlock.
pub(super) fn contended(error: &ProjectCommandError) -> bool {
    let ProjectCommandError::Unavailable(source) = error else {
        return false;
    };
    let mut next: Option<&(dyn std::error::Error + 'static)> = Some(source.as_ref());
    while let Some(error) = next {
        let code = error
            .downcast_ref::<tokio_postgres::Error>()
            .and_then(tokio_postgres::Error::code);
        if let Some(
            &SqlState::T_R_SERIALIZATION_FAILURE
            | &SqlState::UNIQUE_VIOLATION
            | &SqlState::T_R_DEADLOCK_DETECTED,
        ) = code
        {
            return true;
        }
        next = error.source();
    }
    false
}

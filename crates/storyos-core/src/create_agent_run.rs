//! Pure Core classification for one bounded createAgentRun admission.

use super::{ProjectLifecycle, ProjectPresence};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssistanceAdmission {
    Missing,
    Unavailable,
    Available,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationAdmission {
    New,
    Existing { found: bool, busy: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChapterAdmission {
    Current,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateAgentRun {
    pub presence: ProjectPresence,
    pub lifecycle: ProjectLifecycle,
    pub assistance: AssistanceAdmission,
    pub conversation: ConversationAdmission,
    pub chapter: ChapterAdmission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateAgentRunRefusal {
    MissingProject,
    ArchivedProject,
    AssistanceUnavailable,
    InaccessibleConversation,
    ConversationBusy,
    InvalidChapterJoin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateAgentRunResult {
    Admitted,
    Refused { reason: CreateAgentRunRefusal },
}

/// Classify one createAgentRun against Scope, assistance, conversation, and Working Target.
pub fn create_agent_run(command: &CreateAgentRun) -> CreateAgentRunResult {
    if command.presence == ProjectPresence::Absent {
        return CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::MissingProject,
        };
    }
    if command.lifecycle == ProjectLifecycle::Archived {
        return CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::ArchivedProject,
        };
    }
    if command.assistance != AssistanceAdmission::Available {
        return CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::AssistanceUnavailable,
        };
    }
    if command.chapter != ChapterAdmission::Current {
        return CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::InvalidChapterJoin,
        };
    }
    match command.conversation {
        ConversationAdmission::New => CreateAgentRunResult::Admitted,
        ConversationAdmission::Existing { found: false, .. } => CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::InaccessibleConversation,
        },
        ConversationAdmission::Existing { busy: true, .. } => CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::ConversationBusy,
        },
        ConversationAdmission::Existing {
            found: true,
            busy: false,
        } => CreateAgentRunResult::Admitted,
    }
}

#[cfg(test)]
#[path = "create_agent_run_tests.rs"]
mod tests;

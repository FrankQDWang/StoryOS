//! Pure Core classification for one bounded createAgentRun admission.

use std::convert::Infallible;

use super::{ProjectLifecycle, ProjectPresence, TransitionOutcome};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssistanceAdmission {
    Missing,
    Unavailable,
    Available,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationAdmission {
    New,
    ExistingIdle,
    ExistingBusy,
    ExistingMissing,
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

/// The Core Transition Outcome of a createAgentRun. It has no zero-authority outcome.
pub type CreateAgentRunOutcome = TransitionOutcome<(), Infallible, Infallible, Infallible>;

/// Classify one createAgentRun against Scope, assistance, conversation, and Working Target.
///
/// Each refusal comes before the Admission of the command.
pub fn create_agent_run(
    command: &CreateAgentRun,
) -> Result<CreateAgentRunOutcome, CreateAgentRunRefusal> {
    if command.presence == ProjectPresence::Absent {
        return Err(CreateAgentRunRefusal::MissingProject);
    }
    if command.lifecycle == ProjectLifecycle::Archived {
        return Err(CreateAgentRunRefusal::ArchivedProject);
    }
    if command.assistance != AssistanceAdmission::Available {
        return Err(CreateAgentRunRefusal::AssistanceUnavailable);
    }
    if command.chapter != ChapterAdmission::Current {
        return Err(CreateAgentRunRefusal::InvalidChapterJoin);
    }
    match command.conversation {
        ConversationAdmission::New | ConversationAdmission::ExistingIdle => {
            Ok(TransitionOutcome::Applied(()))
        }
        ConversationAdmission::ExistingMissing => {
            Err(CreateAgentRunRefusal::InaccessibleConversation)
        }
        ConversationAdmission::ExistingBusy => Err(CreateAgentRunRefusal::ConversationBusy),
    }
}

#[cfg(test)]
#[path = "create_agent_run_tests.rs"]
mod tests;

use super::{
    AssistanceAdmission, ChapterAdmission, ConversationAdmission, CreateAgentRun,
    CreateAgentRunRefusal, CreateAgentRunResult, create_agent_run,
};
use crate::{ProjectLifecycle, ProjectPresence};

#[test]
fn admits_a_new_conversation_when_assistance_and_target_are_current() {
    assert_eq!(
        create_agent_run(&CreateAgentRun {
            presence: ProjectPresence::Present,
            lifecycle: ProjectLifecycle::Active,
            assistance: AssistanceAdmission::Available,
            conversation: ConversationAdmission::New,
            chapter: ChapterAdmission::Current,
        }),
        CreateAgentRunResult::Admitted
    );
}

#[test]
fn refuses_an_inaccessible_existing_conversation() {
    assert_eq!(
        create_agent_run(&CreateAgentRun {
            presence: ProjectPresence::Present,
            lifecycle: ProjectLifecycle::Active,
            assistance: AssistanceAdmission::Available,
            conversation: ConversationAdmission::ExistingMissing,
            chapter: ChapterAdmission::Current,
        }),
        CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::InaccessibleConversation,
        }
    );
}

fn admitted() -> CreateAgentRun {
    CreateAgentRun {
        presence: ProjectPresence::Present,
        lifecycle: ProjectLifecycle::Active,
        assistance: AssistanceAdmission::Available,
        conversation: ConversationAdmission::New,
        chapter: ChapterAdmission::Current,
    }
}

#[test]
fn refuses_an_archived_project() {
    let mut command = admitted();
    command.lifecycle = ProjectLifecycle::Archived;
    assert_eq!(
        create_agent_run(&command),
        CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::ArchivedProject,
        }
    );
}

#[test]
fn refuses_unavailable_assistance() {
    let mut command = admitted();
    command.assistance = AssistanceAdmission::Unavailable;
    assert_eq!(
        create_agent_run(&command),
        CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::AssistanceUnavailable,
        }
    );
}

#[test]
fn refuses_a_busy_existing_conversation() {
    let mut command = admitted();
    command.conversation = ConversationAdmission::ExistingBusy;
    assert_eq!(
        create_agent_run(&command),
        CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::ConversationBusy,
        }
    );
}

#[test]
fn refuses_an_invalid_working_target_chapter() {
    let mut command = admitted();
    command.chapter = ChapterAdmission::Invalid;
    assert_eq!(
        create_agent_run(&command),
        CreateAgentRunResult::Refused {
            reason: CreateAgentRunRefusal::InvalidChapterJoin,
        }
    );
}

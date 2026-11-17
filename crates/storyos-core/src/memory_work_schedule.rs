//! The schedule of background Memory work for a Project Conversation (ADR 0094 section 5, writer
//! 2): the due time after the completion of a Run of the author, the decision for a claimed row
//! from the facts that the store loads (ADR 0039), and the input text of a Conversation capture.
//! Each time rule gets its times as values.

use std::time::{Duration, SystemTime};

/// The quiet time of a Project Conversation after the completion of a Run of the author. This
/// is an initial value. S7-D7 can change it.
pub const CONVERSATION_QUIET_TIME: Duration = Duration::from_secs(/*30 minutes*/ 1800);

/// The due time of the schedule row of a Conversation after a Run of the author that settled at
/// `settled_at` completed.
pub fn conversation_capture_due(settled_at: SystemTime) -> SystemTime {
    settled_at + CONVERSATION_QUIET_TIME
}

/// The facts of one claimed schedule row of a Conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConversationCaptureFacts {
    /// A Run of the author in the Conversation is queued, claimed, waiting, or paused.
    pub author_run_unsettled: bool,
    /// The current Conversation Memory Settings permit contribution.
    pub contribution_permitted: bool,
    /// A completed Run of the Conversation is after the last Run that a capture read.
    pub unread_runs: bool,
    /// A capture of the Conversation is in progress.
    pub capture_in_progress: bool,
}

/// Why a claimed row started no capture. The due time becomes empty in each case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationCaptureSkip {
    /// The completion of that Run sets the due time again.
    AuthorRunUnsettled,
    /// The messages stay not read.
    ContributionNotPermitted,
    NothingUnread,
    /// The settlement of that capture sets the due time again.
    CaptureInProgress,
    /// The start of the capture refused (ADR 0094 section 8).
    StartRefused,
}

impl ConversationCaptureSkip {
    pub fn label(self) -> &'static str {
        match self {
            Self::AuthorRunUnsettled => "author_run_unsettled",
            Self::ContributionNotPermitted => "contribution_not_permitted",
            Self::NothingUnread => "nothing_unread",
            Self::CaptureInProgress => "capture_in_progress",
            Self::StartRefused => "start_refused",
        }
    }
}

/// Decides whether a claimed row of a Conversation starts one capture. The checks are in the
/// order of the table of S7-D3e. `Ok`: one capture starts.
pub fn decide_conversation_capture(
    facts: &ConversationCaptureFacts,
) -> Result<(), ConversationCaptureSkip> {
    if facts.author_run_unsettled {
        return Err(ConversationCaptureSkip::AuthorRunUnsettled);
    }
    if !facts.contribution_permitted {
        return Err(ConversationCaptureSkip::ContributionNotPermitted);
    }
    if !facts.unread_runs {
        return Err(ConversationCaptureSkip::NothingUnread);
    }
    if facts.capture_in_progress {
        return Err(ConversationCaptureSkip::CaptureInProgress);
    }
    Ok(())
}

/// The role of one message of a Conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationRole {
    /// The author message or a steering input that a RunStep read.
    Author,
    /// The final answer text of the Run.
    Assistant,
}

impl ConversationRole {
    fn name(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Assistant => "assistant",
        }
    }
}

/// The settled messages of one completed Run of the Conversation, in their order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationRunMessages {
    pub run_id: String,
    pub messages: Vec<(ConversationRole, String)>,
}

/// The input of one Conversation capture, and the first and the last Run that it reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationCaptureInput {
    pub text: String,
    pub first_run_id: String,
    pub last_run_id: String,
}

/// The input of a capture from the unread Runs of the Conversation, the oldest first. Each
/// message is one JSON line with its role and the identity of its Run, then its text. `None`:
/// no Run is unread.
pub fn conversation_capture_input(
    runs: &[ConversationRunMessages],
) -> Option<ConversationCaptureInput> {
    let (first, last) = (runs.first()?, runs.last()?);
    let text = runs
        .iter()
        .flat_map(|run| {
            run.messages.iter().map(|(role, text)| {
                let header = serde_json::json!({"role": role.name(), "run_id": run.run_id});
                format!("{header}\n{text}")
            })
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Some(ConversationCaptureInput {
        text,
        first_run_id: first.run_id.clone(),
        last_run_id: last.run_id.clone(),
    })
}

#[cfg(test)]
#[path = "memory_work_schedule_tests.rs"]
mod tests;

//! The schedule of background Memory work for a Project Conversation (ADR 0094 section 5, writer
//! 2): the due time after the completion of a Run of the author, the decision for a claimed row
//! from the facts that the store loads (ADR 0039), and the bounded input text of a Conversation
//! capture. Each time rule gets its times as values.

use std::time::{Duration, SystemTime};

use crate::assemble_context::{ProjectionMode, count_context_item_tokens};

/// The quiet time of a Project Conversation after the completion of a Run of the author. This
/// is an initial value. S7-D7 can change it.
pub const CONVERSATION_QUIET_TIME: Duration = Duration::from_secs(/*30 minutes*/ 1800);

/// The due time of the schedule row of a Conversation after a Run of the author that settled at
/// `settled_at` completed.
pub fn conversation_capture_due(settled_at: SystemTime) -> SystemTime {
    settled_at + CONVERSATION_QUIET_TIME
}

/// The due time that the settlement of a Conversation capture with `outcome` at `settled_at`
/// writes. Only `observations_recorded` and `no_observation` move the last Run that a capture
/// read. When a completed Run is after that Run (`later_run`), the row is due at once. After
/// `failed` or `step_limit`, the Host starts no capture for the same messages by itself, and the
/// next completed Run of the author sets the due time. `None`: the settlement writes no due time.
pub fn capture_settlement_due(
    outcome: crate::CaptureOutcome,
    later_run: bool,
    settled_at: SystemTime,
) -> Option<SystemTime> {
    let moves_last_run = matches!(
        outcome,
        crate::CaptureOutcome::ObservationsRecorded | crate::CaptureOutcome::NoObservation
    );
    (moves_last_run && later_run).then_some(settled_at)
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
    /// When that capture settles with `observations_recorded` or `no_observation`, and a completed
    /// Run is after the last Run that it read, its settlement sets the due time again. After
    /// `failed` or `step_limit`, the next completed Run of the author sets it.
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

/// The input of one Conversation capture, the first and the last Run that it reads, and its
/// projection mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationCaptureInput {
    pub text: String,
    pub first_run_id: String,
    pub last_run_id: String,
    pub projection_mode: ProjectionMode,
}

/// The input of a capture from the unread Runs of the Conversation, the oldest first, at or below
/// `item_token_limit` (ADR 0052). Each message is one JSON line with its role and the identity of
/// its Run, then its text. The input has the oldest Runs that fit, and the last Run that it reads
/// is the last of them. The next capture reads the other Runs.
///
/// When the oldest Run alone is above the limit, the input has only that Run, and its final
/// answer is a `deterministic_excerpt`: a prefix of the answer and an explicit mark with the kept
/// and the total character counts. When the author messages of that Run are above the limit
/// also with no answer text, the input stays above the limit, and the start of the capture
/// refuses it. `None`: no Run is unread.
pub fn conversation_capture_input(
    runs: &[ConversationRunMessages],
    item_token_limit: u64,
) -> Option<ConversationCaptureInput> {
    let first = runs.first()?;
    let mut text = run_text(first, None);
    if count_context_item_tokens(&text) > item_token_limit {
        return Some(ConversationCaptureInput {
            text: answer_excerpt(first, item_token_limit),
            first_run_id: first.run_id.clone(),
            last_run_id: first.run_id.clone(),
            projection_mode: ProjectionMode::DeterministicExcerpt,
        });
    }
    let mut last = first;
    for run in &runs[1..] {
        let longer = format!("{text}\n\n{}", run_text(run, None));
        if count_context_item_tokens(&longer) > item_token_limit {
            break;
        }
        (text, last) = (longer, run);
    }
    Some(ConversationCaptureInput {
        text,
        first_run_id: first.run_id.clone(),
        last_run_id: last.run_id.clone(),
        projection_mode: ProjectionMode::ExactRequired,
    })
}

/// The messages of one Run. With `answer_characters`, the final answer keeps only that many
/// characters and ends with the mark of the cut.
fn run_text(run: &ConversationRunMessages, answer_characters: Option<usize>) -> String {
    run.messages
        .iter()
        .map(|(role, text)| {
            let header = serde_json::json!({"role": role.name(), "run_id": run.run_id});
            match (role, answer_characters) {
                (ConversationRole::Assistant, Some(kept)) => {
                    let prefix: String = text.chars().take(kept).collect();
                    let total = text.chars().count();
                    format!(
                        "{header}\n{prefix} [The Host cut this answer here. It keeps {kept} of its \
                         {total} characters.]"
                    )
                }
                _ => format!("{header}\n{text}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The text of one Run whose final answer keeps the longest prefix that fits the limit. The count
/// grows with the kept characters, so a binary search finds that prefix.
fn answer_excerpt(run: &ConversationRunMessages, item_token_limit: u64) -> String {
    let answer = run
        .messages
        .iter()
        .rfind(|(role, _)| *role == ConversationRole::Assistant)
        .map_or(0, |(_, text)| text.chars().count());
    let fits = |kept| count_context_item_tokens(&run_text(run, Some(kept))) <= item_token_limit;
    let (mut low, mut high) = (0, answer);
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    run_text(run, Some(low))
}

#[cfg(test)]
#[path = "memory_work_schedule_tests.rs"]
mod tests;

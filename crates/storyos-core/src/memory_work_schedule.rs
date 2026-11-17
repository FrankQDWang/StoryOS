//! The schedule of background Memory work (ADR 0094 sections 5 and 6, writer 2). For a Project
//! Conversation: the due time after the completion of a Run of the author, the decision for a
//! claimed row from the facts that the store loads (ADR 0039), and the bounded input text of a
//! Conversation capture. For a Project: the due time of a consolidation pass and the decision for
//! its claimed row. Each time rule gets its times as values.

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
/// `failed` or `step_limit`, the Host starts no capture for the same messages by itself. Then the
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

/// The least time from the start of a consolidation pass of a Project to the start of the next
/// pass. This is an initial value. S7-D7 can change it.
pub const CONSOLIDATION_INTERVAL: Duration = Duration::from_secs(/*30 minutes*/ 1800);

/// The due time of the consolidation row of a Project that a settlement at `now` writes. The
/// settlement of a capture that stored observations and the settlement of each pass use it.
/// When an observation waits, the row is due at `now`. When the last pass of the Project started
/// at `last_pass_start`, the row is due at that start and the interval if that is later. `None`:
/// no observation waits, and the settlement writes no due time.
pub fn consolidation_due(
    observation_waits: bool,
    now: SystemTime,
    last_pass_start: Option<SystemTime>,
) -> Option<SystemTime> {
    let earliest = last_pass_start.map(|start| start + CONSOLIDATION_INTERVAL);
    observation_waits.then(|| earliest.map_or(now, |earliest| now.max(earliest)))
}

/// The facts of one claimed consolidation row of a Project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsolidationFacts {
    /// A Memory Observation of the Project waits.
    pub observation_waits: bool,
    /// A pass of the Project is in progress.
    pub pass_in_progress: bool,
}

/// Why a claimed consolidation row started no pass. The due time becomes empty in each case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsolidationSkip {
    /// The next capture that stores observations sets the due time again.
    NothingWaits,
    /// The settlement of that pass sets the due time again when an observation still waits.
    PassInProgress,
    /// The admission of S7-D3b refused the start (ADR 0094 section 8).
    StartRefused,
}

impl ConsolidationSkip {
    pub fn label(self) -> &'static str {
        match self {
            Self::NothingWaits => "no_observation_waits",
            Self::PassInProgress => "pass_in_progress",
            Self::StartRefused => "start_refused",
        }
    }
}

/// Decides whether a claimed consolidation row starts one pass. `Ok`: one pass starts, when the
/// admission of the start permits it.
pub fn decide_consolidation(facts: &ConsolidationFacts) -> Result<(), ConsolidationSkip> {
    if !facts.observation_waits {
        return Err(ConsolidationSkip::NothingWaits);
    }
    if facts.pass_in_progress {
        return Err(ConsolidationSkip::PassInProgress);
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
/// When the oldest Run alone is above the limit, the input has only that Run as a
/// `deterministic_excerpt`. It has the whole messages of the Run in their order while they fit.
/// The first message that does not fit keeps its longest prefix that fits, and an explicit mark
/// gives the kept and the total character counts and the count of the later messages that it
/// omits. When the author messages of the Run fit, the cut is in the final answer. `None`: no Run
/// is unread.
pub fn conversation_capture_input(
    runs: &[ConversationRunMessages],
    item_token_limit: u64,
) -> Option<ConversationCaptureInput> {
    let first = runs.first()?;
    let mut text = run_text(first, first.messages.len(), None);
    if count_context_item_tokens(&text) > item_token_limit {
        return Some(ConversationCaptureInput {
            text: run_excerpt(first, item_token_limit),
            first_run_id: first.run_id.clone(),
            last_run_id: first.run_id.clone(),
            projection_mode: ProjectionMode::DeterministicExcerpt,
        });
    }
    let mut last = first;
    for run in &runs[1..] {
        let longer = format!("{text}\n\n{}", run_text(run, run.messages.len(), None));
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

/// The first `whole` messages of one Run. With `Some(kept)`, the next message follows with only
/// its first `kept` characters and the mark of the cut, and the later messages are omitted.
fn run_text(run: &ConversationRunMessages, whole: usize, kept: Option<usize>) -> String {
    let mut parts = Vec::new();
    for (index, (role, text)) in run.messages.iter().enumerate() {
        let header = serde_json::json!({"role": role.name(), "run_id": run.run_id});
        if index < whole {
            parts.push(format!("{header}\n{text}"));
            continue;
        }
        if let Some(kept) = kept {
            let prefix: String = text.chars().take(kept).collect();
            let total = text.chars().count();
            let omitted = run.messages.len() - index - 1;
            parts.push(format!(
                "{header}\n{prefix} [The Host cut this message here. It keeps {kept} of its {total} \
                 characters. Later messages of the Run that the Host omits: {omitted}.]"
            ));
        }
        break;
    }
    parts.join("\n\n")
}

/// The excerpt of one Run above the limit. The count of the text grows with each whole message
/// and with each kept character, so the excerpt takes the most whole messages that leave room
/// for the mark, and then a binary search finds the longest prefix of the next message.
fn run_excerpt(run: &ConversationRunMessages, item_token_limit: u64) -> String {
    let fits = |whole, kept| {
        count_context_item_tokens(&run_text(run, whole, Some(kept))) <= item_token_limit
    };
    let mut whole = 0;
    while whole + 1 < run.messages.len() && fits(whole + 1, 0) {
        whole += 1;
    }
    let total = run.messages[whole].1.chars().count();
    let (mut low, mut high) = (0, total);
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(whole, middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    run_text(run, whole, Some(low))
}

#[cfg(test)]
#[path = "memory_work_schedule_tests.rs"]
mod tests;

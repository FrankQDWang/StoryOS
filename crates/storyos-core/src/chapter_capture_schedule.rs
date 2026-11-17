//! The schedule of a Chapter capture (ADR 0094 section 5, writer 3): the due time after an
//! Authoritative Commit that changes the text of a Chapter, the rate and the order of the Chapter
//! captures of a Project, the decision for a claimed row from the facts that the store loads (ADR
//! 0039), and the bounded input text of a Chapter capture. Each time rule gets its times as values.

use std::time::{Duration, SystemTime};

use crate::assemble_context::{ProjectionMode, count_context_item_tokens};

/// The quiet time of a Chapter after a commit that changes its text. This is an initial value.
/// S7-D7 can change it.
pub const CHAPTER_QUIET_TIME: Duration = Duration::from_secs(/*1 hour*/ 3600);

/// The least time from the start of a capture of a Chapter to the start of the next capture of
/// the same Chapter. This is an initial value. S7-D7 can change it.
pub const CHAPTER_CAPTURE_INTERVAL: Duration = Duration::from_secs(/*6 hours*/ 21_600);

/// The least time from the start of a Chapter capture of a Project to the start of the next
/// Chapter capture of the same Project. This is an initial value. S7-D7 can change it.
pub const CHAPTER_CAPTURE_RATE: Duration = Duration::from_secs(/*2 minutes*/ 120);

/// The due time of the row of a Chapter after a commit at `committed_at` that changes its text:
/// the commit time and the quiet time, or the start of the last capture of the Chapter and the
/// interval when that is later.
pub fn chapter_capture_due(
    committed_at: SystemTime,
    last_capture_start: Option<SystemTime>,
) -> SystemTime {
    let quiet = committed_at + CHAPTER_QUIET_TIME;
    last_capture_start.map_or(quiet, |start| quiet.max(start + CHAPTER_CAPTURE_INTERVAL))
}

/// The Chapter that a claim of a Project decides for, and the time that the rate permits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChapterCaptureTurn {
    /// The index of the due row whose Chapter is first in the Chapter order.
    pub first: usize,
    /// The time that the rate of the Project permits the next start, when it is after now.
    pub rate_permits_at: Option<SystemTime>,
}

/// The turn of the Chapter captures of one Project. `due_positions` has the position in the
/// Chapter list of the Chapter of each due row. The first Chapter in Chapter order goes first, so
/// that the other rows stay due. The rate permits one start in `CHAPTER_CAPTURE_RATE` from
/// `last_start`, the start of the last Chapter capture of the Project. `None`: no row is due.
pub fn chapter_capture_turn(
    due_positions: &[usize],
    last_start: Option<SystemTime>,
    now: SystemTime,
) -> Option<ChapterCaptureTurn> {
    let (first, _) = due_positions
        .iter()
        .enumerate()
        .min_by_key(|(_, position)| **position)?;
    let rate_permits_at = last_start
        .map(|start| start + CHAPTER_CAPTURE_RATE)
        .filter(|permitted| *permitted > now);
    Some(ChapterCaptureTurn {
        first,
        rate_permits_at,
    })
}

/// The facts of one claimed row of a Chapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChapterCaptureFacts {
    /// The Chapter is a live Chapter of the manuscript.
    pub live: bool,
    /// The authoritative text of the Chapter is empty.
    pub text_empty: bool,
    /// The Authoritative Revision of the Chapter is the Revision that the last capture of the
    /// Chapter with the outcome `observations_recorded` or `no_observation` read.
    pub revision_read: bool,
    /// A capture of the Chapter is in progress.
    pub capture_in_progress: bool,
    /// The time that the rate of the Project permits the next start, when it is after now.
    pub rate_permits_at: Option<SystemTime>,
    /// A Run of the author in the Project completed.
    pub author_run_completed: bool,
}

/// Why a claimed row of a Chapter started no capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChapterCaptureSkip {
    NotLive,
    TextEmpty,
    RevisionRead,
    /// The settlement of that capture sets the due time again when the Revision is not the one
    /// that it read.
    CaptureInProgress,
    /// The row is due at the time that the rate permits.
    RateWait(SystemTime),
    /// The row becomes `held`.
    NoCompletedRun,
    /// The start of S7-D3b refused (ADR 0094 section 8). The row becomes `held`.
    StartRefused,
}

/// The change of a row of a Chapter after a skip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChapterRowChange {
    /// The due time stays empty.
    DueEmpty,
    DueAt(SystemTime),
    /// The row becomes `held`, with no due time. S7-D5b releases it.
    Held,
}

impl ChapterCaptureSkip {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotLive => "chapter_not_live",
            Self::TextEmpty => "text_empty",
            Self::RevisionRead => "revision_read",
            Self::CaptureInProgress => "capture_in_progress",
            Self::RateWait(_) => "rate_wait",
            Self::NoCompletedRun => "no_completed_run",
            Self::StartRefused => "start_refused",
        }
    }

    pub fn row_change(self) -> ChapterRowChange {
        match self {
            Self::NotLive | Self::TextEmpty | Self::RevisionRead | Self::CaptureInProgress => {
                ChapterRowChange::DueEmpty
            }
            Self::RateWait(permitted) => ChapterRowChange::DueAt(permitted),
            Self::NoCompletedRun | Self::StartRefused => ChapterRowChange::Held,
        }
    }
}

/// Decides whether a claimed row of a Chapter starts one capture. The checks are in the order of
/// the table of S7-D5. `Ok`: one capture starts, when the start of S7-D3b permits it.
pub fn decide_chapter_capture(facts: &ChapterCaptureFacts) -> Result<(), ChapterCaptureSkip> {
    if !facts.live {
        return Err(ChapterCaptureSkip::NotLive);
    }
    if facts.text_empty {
        return Err(ChapterCaptureSkip::TextEmpty);
    }
    if facts.revision_read {
        return Err(ChapterCaptureSkip::RevisionRead);
    }
    if facts.capture_in_progress {
        return Err(ChapterCaptureSkip::CaptureInProgress);
    }
    if let Some(permitted) = facts.rate_permits_at {
        return Err(ChapterCaptureSkip::RateWait(permitted));
    }
    if !facts.author_run_completed {
        return Err(ChapterCaptureSkip::NoCompletedRun);
    }
    Ok(())
}

/// The input of one Chapter capture and its projection mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterCaptureInput {
    pub text: String,
    pub projection_mode: ProjectionMode,
}

/// The input of a capture of the Chapter `title` with the `number` in the Chapter list and the
/// text of its `blocks` at the pinned Revision, in their order, at or below `item_token_limit`
/// (ADR 0052). One JSON line gives the number and the title, then each Block follows.
///
/// When the text is above the limit, the input is a `deterministic_excerpt`: the most whole Blocks
/// that fit, then the longest prefix of the next Block that fits. An explicit mark gives the
/// number of the last whole Block, the count of the Blocks, and the kept and the total character
/// counts of the cut Block. The capture can read the other Blocks with `read_chapter`.
pub fn chapter_capture_input(
    title: &str,
    number: usize,
    blocks: &[String],
    item_token_limit: u64,
) -> ChapterCaptureInput {
    let header = serde_json::json!({"chapter_number": number, "chapter_title": title});
    let text = chapter_text(&header, blocks, blocks.len(), None);
    if count_context_item_tokens(&text) <= item_token_limit || blocks.is_empty() {
        return ChapterCaptureInput {
            text,
            projection_mode: ProjectionMode::ExactRequired,
        };
    }
    let fits = |whole, kept| {
        count_context_item_tokens(&chapter_text(&header, blocks, whole, Some(kept)))
            <= item_token_limit
    };
    let mut whole = 0;
    while whole + 1 < blocks.len() && fits(whole + 1, 0) {
        whole += 1;
    }
    let (mut low, mut high) = (0, blocks[whole].chars().count());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(whole, middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    ChapterCaptureInput {
        text: chapter_text(&header, blocks, whole, Some(low)),
        projection_mode: ProjectionMode::DeterministicExcerpt,
    }
}

/// The header and the first `whole` Blocks. With `Some(kept)`, the next Block follows with only
/// its first `kept` characters and the mark of the cut.
fn chapter_text(
    header: &serde_json::Value,
    blocks: &[String],
    whole: usize,
    kept: Option<usize>,
) -> String {
    let mut parts = vec![header.to_string()];
    parts.extend(blocks[..whole].iter().cloned());
    if let Some(kept) = kept {
        let cut = &blocks[whole];
        let prefix: String = cut.chars().take(kept).collect();
        parts.push(format!(
            "{prefix} [The Host cut the Chapter here. Last whole Block: {whole} of {count}. It \
             keeps {kept} of the {total} characters of Block {next}. read_chapter gives the other \
             Blocks.]",
            count = blocks.len(),
            total = cut.chars().count(),
            next = whole + 1,
        ));
    }
    parts.join("\n\n")
}

#[cfg(test)]
#[path = "chapter_capture_schedule_tests.rs"]
mod tests;

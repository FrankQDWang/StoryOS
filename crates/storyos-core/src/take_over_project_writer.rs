//! Pure Core classification for a writer takeover.

use std::convert::Infallible;

use super::{ProjectLifecycle, TransitionOutcome};
use crate::transition_outcome::reason_codes;

/// The current writer of the Project, as the takeover observes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentWriter {
    pub writer_generation: u64,
    /// Whether the requesting Editor Session is already the writer.
    pub is_requesting_session: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeOverProjectWriter {
    pub observed_writer_generation: u64,
    /// `None` when the Project has no writer generation.
    pub current_writer: Option<CurrentWriter>,
    pub lifecycle: ProjectLifecycle,
    /// Whether the Current Chapter exists and has an Authoritative Revision head.
    pub current_chapter_has_head: bool,
}

/// A takeover moves only Operational Records, so it settles as a zero-authority outcome.
pub type TakeOverProjectWriterResult =
    TransitionOutcome<Infallible, TakeOverProjectWriterNoEffect, Infallible, Infallible>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TakeOverProjectWriterNoEffect {
    WriterTakeoverApplied,
}

reason_codes!(TakeOverProjectWriterNoEffect { WriterTakeoverApplied => "writer_takeover_applied" });

/// A takeover whose observed writer is not the current writer; it records no Receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaleWriterObservation;

/// Classify one writer takeover against the current writer, lifecycle, and Current Chapter.
pub fn take_over_project_writer(
    command: &TakeOverProjectWriter,
) -> Result<TakeOverProjectWriterResult, StaleWriterObservation> {
    match command.current_writer {
        Some(CurrentWriter {
            writer_generation,
            is_requesting_session: false,
        }) if writer_generation == command.observed_writer_generation
            && command.lifecycle == ProjectLifecycle::Active
            && command.current_chapter_has_head =>
        {
            Ok(TransitionOutcome::NoEffect(
                TakeOverProjectWriterNoEffect::WriterTakeoverApplied,
            ))
        }
        Some(_) | None => Err(StaleWriterObservation),
    }
}

#[cfg(test)]
#[path = "take_over_project_writer_tests.rs"]
mod tests;

//! The claim port of the schedule of background Memory work (ADR 0094 sections 5 and 6, writers 2
//! and 3).
//! The Worker claims one due schedule row when it has no export work and no Run to claim. The
//! store decides and starts the capture or the pass in the transaction of the claim, so that a
//! second delivery starts no second capture for the same messages and no second pass.

use std::future::Future;

use storyos_core::{
    ChapterCaptureSkip, ConsolidationSkip, ConversationCaptureSkip, IntentPassSkip,
};

use crate::{
    DiagnosticField, ProjectReadError, ProjectScope, StartedIntentPass, StartedMemoryCapture,
    StartedMemoryConsolidation,
};

/// What the decision for one claimed schedule row did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryWorkOutcome {
    Started(StartedMemoryCapture),
    Skipped(ConversationCaptureSkip),
    PassStarted(StartedMemoryConsolidation),
    PassSkipped(ConsolidationSkip),
    /// A claimed row of a Chapter started no capture. A Chapter capture that starts is `Started`.
    ChapterSkipped(ChapterCaptureSkip),
    IntentPassStarted(StartedIntentPass),
    IntentPassSkipped(IntentPassSkip),
}

/// The source of one schedule row: a Project Conversation or a Chapter for a capture, or the
/// Project for a consolidation pass or for an intent pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryWorkSource {
    Conversation(String),
    Project,
    Chapter(String),
    /// The Project for an intent pass (ADR 0101 section 4).
    ProjectIntents,
}

impl MemoryWorkSource {
    /// The kind of the schedule row.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Conversation(_) => "conversation_capture",
            Self::Project => "consolidation",
            Self::Chapter(_) => "chapter_capture",
            Self::ProjectIntents => "intent_pass",
        }
    }
}

/// One claimed schedule row and its outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedMemoryWork {
    pub project_scope: ProjectScope,
    pub source: MemoryWorkSource,
    pub outcome: MemoryWorkOutcome,
}

impl DiagnosticField for MemoryWorkOutcome {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::Started(_) => "capture_started",
            Self::Skipped(skip) => skip.label(),
            Self::PassStarted(_) => "pass_started",
            Self::PassSkipped(skip) => skip.label(),
            Self::ChapterSkipped(skip) => skip.label(),
            Self::IntentPassStarted(_) => "intent_pass_started",
            Self::IntentPassSkipped(skip) => skip.label(),
        }
    }
}

/// The store port of the schedule.
pub trait MemoryWorkScheduleStore: Sync {
    /// Whether one row is due now. The Worker asks it first, so that an idle step makes no claim.
    fn memory_work_due(&self) -> impl Future<Output = Result<bool, ProjectReadError>> + Send;

    /// Claims one row whose due time is in the past, with a row lock. The store decides from the
    /// facts of its source and starts one capture, one pass, or neither. Then it makes the due
    /// time empty. One transaction does these steps. `None`: no row is due.
    fn claim_due_memory_work(
        &self,
    ) -> impl Future<Output = Result<Option<ClaimedMemoryWork>, ProjectReadError>> + Send;
}

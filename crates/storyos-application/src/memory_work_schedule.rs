//! The claim port of the schedule of background Memory work (ADR 0094 section 5, writer 2). The
//! Worker claims one due schedule row when it has no export work and no Run to claim. The store
//! decides and starts the capture in the transaction of the claim, so that a second delivery
//! starts no second capture for the same messages.

use std::future::Future;

use storyos_core::ConversationCaptureSkip;

use crate::{DiagnosticField, ProjectReadError, ProjectScope, StartedMemoryCapture};

/// What the decision for one claimed schedule row did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryWorkOutcome {
    Started(StartedMemoryCapture),
    Skipped(ConversationCaptureSkip),
}

/// One claimed schedule row of a Project Conversation and its outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedMemoryWork {
    pub project_scope: ProjectScope,
    pub conversation_id: String,
    pub outcome: MemoryWorkOutcome,
}

impl DiagnosticField for MemoryWorkOutcome {
    type Value<'a> = &'static str;

    fn diagnostic(&self) -> Self::Value<'_> {
        match self {
            Self::Started(_) => "capture_started",
            Self::Skipped(skip) => skip.label(),
        }
    }
}

/// The store port of the schedule.
pub trait MemoryWorkScheduleStore: Sync {
    /// Claims one row whose due time is in the past with a row lock, decides from the facts of
    /// its Conversation, starts one capture or none, and makes the due time empty, in one
    /// transaction. `None`: no row is due.
    fn claim_due_memory_work(
        &self,
    ) -> impl Future<Output = Result<Option<ClaimedMemoryWork>, ProjectReadError>> + Send;
}

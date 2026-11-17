//! The start port of a capture (ADR 0094 section 5). The triggers of S7-D3e and S7-D5 call it
//! after a Run of the author in the Project completed. The store pins the current Model Use
//! Binding, the grant, and the Model Registration of the Project, and Core decides the admission.

use std::future::Future;

use crate::{ProjectReadError, ProjectScope};

/// The source of one capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryCaptureSource {
    /// A Project Conversation, with the first and the last Run whose messages the capture reads.
    Conversation {
        conversation_id: String,
        first_run_id: String,
        last_run_id: String,
    },
    /// One Chapter at one Revision.
    Chapter {
        chapter_id: String,
        revision_id: String,
    },
}

/// The capture that a start queued.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedMemoryCapture {
    pub capture_id: String,
    pub run_id: String,
}

/// Why the store did not start a capture. A refused start writes no row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryCaptureStartRefusal {
    /// The Core admission refused the facts of the Project.
    Admission(storyos_core::CaptureRefusal),
    /// The source is not in the Project Scope.
    SourceNotFound,
}

/// The store port of the start of a capture (ADR 0094 section 5). A trigger calls it after a
/// Run of the author in the Project completed.
pub trait MemoryCaptureStore: Sync {
    /// Starts one capture of `source` with the input text `input` in one transaction.
    fn start_memory_capture(
        &self,
        scope: &ProjectScope,
        source: &MemoryCaptureSource,
        input: &str,
    ) -> impl Future<
        Output = Result<Result<StartedMemoryCapture, MemoryCaptureStartRefusal>, ProjectReadError>,
    > + Send;
}

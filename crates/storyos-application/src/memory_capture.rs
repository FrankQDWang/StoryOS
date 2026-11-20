//! The types of the start of a capture (ADR 0094 section 5). The store pins the current Model Use
//! Binding, the grant, and the Model Registration of the Project, and Core decides the admission.

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

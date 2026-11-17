//! The start port of a consolidation pass (ADR 0094 section 6). The schedule of S7-D4a3 calls it
//! when observations of the Project wait. The store pins the current Model Use Binding, the grant,
//! the Model Registration, and the Memory revision of the Project, and Core decides the admission
//! with the rule of a capture.

use std::future::Future;

use crate::{ProjectReadError, ProjectScope};

/// The pass that a start queued.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedMemoryConsolidation {
    pub pass_id: String,
    pub run_id: String,
}

/// The store port of the start of a pass. A refused start writes no row.
pub trait MemoryConsolidationStore: Sync {
    /// Starts one pass of the observations that wait in the Project, in one transaction.
    fn start_memory_consolidation(
        &self,
        scope: &ProjectScope,
    ) -> impl Future<
        Output = Result<
            Result<StartedMemoryConsolidation, storyos_core::CaptureRefusal>,
            ProjectReadError,
        >,
    > + Send;
}

//! The type of the start of a consolidation pass (ADR 0094 section 6). The store pins the current
//! Model Use Binding, the grant, the Model Registration, and the Memory revision of the Project,
//! and Core decides the admission with the rule of a capture.

/// The pass that a start queued.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedMemoryConsolidation {
    pub pass_id: String,
    pub run_id: String,
}

/// The intent pass that a start queued (ADR 0101 section 4).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedIntentPass {
    pub pass_id: String,
    pub run_id: String,
}

//! The input, effect, and settlement of the writer takeover command.

use std::convert::Infallible;

use storyos_core::TakeOverProjectWriterNoEffect;

use crate::{ActivityApplied, EditorSessionId, ProjectCommandSettlement};

/// One request of an Editor Session to become the Project writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeOverProjectWriterInput {
    pub editor_session_id: EditorSessionId,
    pub observed_writer_generation: u64,
    pub editor_contract_revision: String,
}

/// The new writer generation and base Snapshot that a writer takeover records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriterTakeover {
    pub prior_editor_session_id: String,
    pub prior_writer_generation: u64,
    pub resulting_editor_session_id: String,
    pub resulting_writer_generation: u64,
    pub resulting_snapshot_id: String,
    pub resulting_snapshot_activity_position: u64,
    /// The current Authoritative Revision of the Current Chapter at the takeover.
    pub resulting_head: String,
}

/// A writer takeover settles as a zero-authority outcome that carries its writer effect.
pub type TakeOverProjectWriterSettlement = ProjectCommandSettlement<
    ActivityApplied<Infallible>,
    TakeOverProjectWriterNoEffect,
    Infallible,
    Infallible,
    (),
    WriterTakeover,
>;

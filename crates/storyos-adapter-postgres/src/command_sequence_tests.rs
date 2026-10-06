#[path = "command_sequence_tests/support.rs"]
mod support;

#[path = "command_sequence_tests/structure_tests.rs"]
mod structure;

#[path = "command_sequence_tests/project_session_tests.rs"]
mod project_session;

#[path = "command_sequence_tests/proposal_decision_tests.rs"]
mod proposal_decision;

#[path = "command_sequence_tests/proposal_generation_tests.rs"]
mod proposal_generation;

#[path = "command_sequence_tests/draft_tests.rs"]
mod draft;

#[path = "command_sequence_tests/replay_tests.rs"]
mod replay;

#[path = "command_sequence_tests/retry_tests.rs"]
mod retry;

#[path = "command_sequence_tests/rollback_tests.rs"]
mod rollback;

#[path = "command_sequence_tests/damaged_evidence_tests.rs"]
mod damaged_evidence;

pub(crate) use structure::{
    create_chapter, create_volume, delete_chapter, delete_volume, update_chapter, update_volume,
};
pub(crate) use support::{CommandCall, applied, command_call};

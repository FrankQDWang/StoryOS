#[path = "command_sequence_tests/support.rs"]
mod support;

#[path = "command_sequence_tests/structure_tests.rs"]
mod structure;

#[path = "command_sequence_tests/acceptance_tests.rs"]
mod acceptance;

#[path = "command_sequence_tests/project_session_tests.rs"]
mod project_session;

#[path = "command_sequence_tests/proposal_decision_tests.rs"]
mod proposal_decision;

#[path = "command_sequence_tests/proposal_generation_tests.rs"]
mod proposal_generation;

#[path = "command_sequence_tests/agent_run_tests.rs"]
mod agent_run;

#[path = "command_sequence_tests/admit_tests.rs"]
mod admit;

#[path = "command_sequence_tests/steer_agent_run_tests.rs"]
mod steer_agent_run;

#[path = "command_sequence_tests/replay_tests.rs"]
mod replay;

#[path = "command_sequence_tests/retry_tests.rs"]
mod retry;

#[path = "command_sequence_tests/rollback_tests.rs"]
mod rollback;

#[path = "command_sequence_tests/damaged_evidence_tests.rs"]
mod damaged_evidence;

#[path = "command_sequence_tests/declared_error_tests.rs"]
mod declared_error;

#[path = "command_sequence_tests/undo_tests.rs"]
mod undo;

#[path = "command_sequence_tests/undo_chain_tests.rs"]
mod undo_chain;

#[path = "command_sequence_tests/proposal_undo_chain_tests.rs"]
mod proposal_undo_chain;

#[path = "command_sequence_tests/proposal_decision_undo_chain_tests.rs"]
mod proposal_decision_undo_chain;

#[path = "command_sequence_tests/proposal_acceptance_undo_chain_tests.rs"]
mod proposal_acceptance_undo_chain;

#[path = "command_sequence_tests/producer_withdrawal_undo_tests.rs"]
mod producer_withdrawal_undo;

#[path = "command_sequence_tests/author_edit_tests.rs"]
mod author_edit;

#[path = "command_sequence_tests/secondary_operation_tests.rs"]
mod secondary_operation;

pub(crate) use agent_run::CREATE_AGENT_RUN;
pub(crate) use project_session::{ARCHIVE_PROJECT, UPDATE_PROJECT, UPDATE_PROJECT_ASSISTANCE};
pub(crate) use structure::{
    CREATE_CHAPTER, CREATE_VOLUME, DELETE_CHAPTER, DELETE_VOLUME, SET_CURRENT_CHAPTER,
    UPDATE_CHAPTER, UPDATE_VOLUME, create_chapter, create_volume, delete_chapter, delete_volume,
    update_chapter, update_volume,
};
pub(crate) use support::{
    CREATE_EDITOR_SESSION, CommandCall, Route, applied, command_call, command_issue,
    create_project_command, create_project_issue, issued, seed_owned_project, seed_project, stores,
    undo_named,
};
pub(crate) use undo::UNDO_LATEST_AUTHOR_ACTION;

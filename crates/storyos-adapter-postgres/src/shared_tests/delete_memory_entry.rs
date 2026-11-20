//! The shared test support from `delete_memory_entry_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_memory_entry::DELETE_MEMORY_ENTRY;
use crate::shared_tests::command_sequence_support::issued;
use std::convert::Infallible;
use storyos_application::{
    DeleteMemoryEntryInput, MemoryEntryDeleted, ProjectScope, RefusableCommandError,
};
use storyos_core::{
    DeleteMemoryEntryConflict, DeleteMemoryEntryNoEffect, DeleteMemoryEntryRefusal,
    TransitionOutcome,
};

pub(crate) type Outcome = Result<
    TransitionOutcome<
        MemoryEntryDeleted,
        DeleteMemoryEntryNoEffect,
        DeleteMemoryEntryConflict,
        Infallible,
    >,
    DeleteMemoryEntryRefusal,
>;

/// Deletes `memory_entry_id` in the Project of `scope` with the Command Challenge `suffix`.
pub(crate) async fn delete(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    memory_entry_id: &str,
    expected_version: u64,
) -> Outcome {
    let input = DeleteMemoryEntryInput {
        memory_entry_id: memory_entry_id.to_owned(),
        expected_version,
    };
    let call = issued(store, scope, suffix, &DELETE_MEMORY_ENTRY, input).await;
    match store.delete_memory_entry(&call.envelope, &call.input).await {
        Ok(settlement) => Ok(settlement.outcome.map_applied(|applied| applied.effect)),
        Err(RefusableCommandError::RefusedBeforeAdmission(refusal)) => Err(refusal),
        Err(RefusableCommandError::Command(error)) => panic!("the delete must settle: {error:?}"),
    }
}

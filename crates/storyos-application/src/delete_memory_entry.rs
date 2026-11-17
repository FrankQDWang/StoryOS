//! The input and the settlement of the Delete Memory Entry command (ADR 0094 sections 6 and 9).

use std::convert::Infallible;

use storyos_core::{
    DeleteMemoryEntryConflict, DeleteMemoryEntryNoEffect, DeleteMemoryEntryRefusal,
};

use crate::{ActivityApplied, ProjectCommandSettlement, RefusableCommandError};

/// One delete of a Memory Entry by the author on the settings page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteMemoryEntryInput {
    pub memory_entry_id: String,
    /// The version of the entry that the page showed.
    pub expected_version: u64,
}

/// The `deleted` version that an applied delete wrote, and the Memory revision of the Project
/// after it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntryDeleted {
    pub memory_entry_id: String,
    pub version: u64,
    pub memory_revision: u64,
}

pub type DeleteMemoryEntrySettlement = ProjectCommandSettlement<
    ActivityApplied<MemoryEntryDeleted>,
    DeleteMemoryEntryNoEffect,
    DeleteMemoryEntryConflict,
    Infallible,
>;

pub type DeleteMemoryEntryError = RefusableCommandError<DeleteMemoryEntryRefusal>;
